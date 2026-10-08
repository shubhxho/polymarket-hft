use crate::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub struct BinaryScores {
    pub samples: usize,
    pub brier: f64,
    pub log_loss: f64,
    pub calibration_error: f64,
}
/// Proper scores for caller-supplied held-out forecasts. Does not establish model quality.
pub fn binary_scores(forecasts: &[f64], outcomes: &[bool]) -> Result<BinaryScores> {
    if forecasts.is_empty()
        || forecasts.len() != outcomes.len()
        || forecasts
            .iter()
            .any(|p| !p.is_finite() || !(0.0..=1.0).contains(p))
    {
        return Err("invalid_evaluation_data".into());
    }
    let (mut brier, mut loss) = (0.0, 0.0);
    let mut bins = [(0usize, 0.0f64, 0.0f64); 10];
    for (&p, &y) in forecasts.iter().zip(outcomes) {
        let y = f64::from(y);
        brier += (p - y).powi(2);
        let clipped = p.clamp(1e-15, 1.0 - 1e-15);
        loss -= y * clipped.ln() + (1.0 - y) * (1.0 - clipped).ln();
        let i = ((p * 10.0) as usize).min(9);
        bins[i].0 += 1;
        bins[i].1 += p;
        bins[i].2 += y;
    }
    let n = forecasts.len() as f64;
    let ece = bins
        .iter()
        .filter(|b| b.0 > 0)
        .map(|b| (b.1 - b.2).abs() / n)
        .sum();
    Ok(BinaryScores {
        samples: forecasts.len(),
        brier: brier / n,
        log_loss: loss / n,
        calibration_error: ece,
    })
}

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Metric {
    High,
    Low,
}

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Bucket {
    pub lo: i32,
    pub hi: i32,
}

// Abramowitz-Stegun normal CDF approximation, absolute error ~7.5e-8.
fn cdf(x: f64) -> f64 {
    let t = 1.0 / (1.0 + 0.2316419 * x.abs());
    let p = 0.3989422804014327
        * (-0.5 * x * x).exp()
        * t
        * (0.319381530
            + t * (-0.356563782 + t * (1.781477937 + t * (-1.821255978 + t * 1.330274429))));
    if x >= 0.0 { 1.0 - p } else { p }
}

pub fn probabilities(
    buckets: &[Bucket],
    metric: Metric,
    observed: f64,
    mu: f64,
    sigma: f64,
    hours: f64,
) -> Result<Vec<f64>> {
    if buckets.is_empty()
        || ![observed, mu, sigma, hours].iter().all(|v| v.is_finite())
        || sigma <= 0.0
        || !(0.0..=24.0).contains(&hours)
    {
        return Err("invalid_forecast".into());
    }
    for (i, b) in buckets.iter().enumerate() {
        if b.lo > b.hi || (i > 0 && i64::from(b.lo) != i64::from(buckets[i - 1].hi) + 1) {
            return Err("buckets_must_partition_in_order".into());
        }
    }
    if observed < buckets[0].lo as f64 - 0.5 || observed >= buckets.last().unwrap().hi as f64 + 0.5
    {
        return Err("observation_outside_partition".into());
    }
    let w = (hours / 14.0).min(1.0);
    let sig = (sigma * (0.18 + 0.82 * w)).max(0.15);
    let mean = (1.0 - w) * observed + w * mu;
    let mean = match metric {
        Metric::High => mean.max(observed),
        Metric::Low => mean.min(observed),
    };
    let mut out = Vec::with_capacity(buckets.len());
    for b in buckets {
        let lo = b.lo as f64 - 0.5;
        let hi = b.hi as f64 + 0.5;
        let p = if hours == 0.0 {
            f64::from(lo <= observed && observed < hi)
        } else {
            match metric {
                Metric::High if hi <= observed => 0.0,
                Metric::High if lo <= observed => cdf((hi - mean) / sig),
                Metric::Low if lo > observed => 0.0,
                Metric::Low if hi > observed => 1.0 - cdf((lo - mean) / sig),
                _ => cdf((hi - mean) / sig) - cdf((lo - mean) / sig),
            }
        };
        out.push(p.max(0.0));
    }
    // Never normalize an incomplete partition into false certainty.
    if out.iter().sum::<f64>() < 0.99999 {
        return Err("partition_missing_probability_mass".into());
    }
    Ok(out)
}
