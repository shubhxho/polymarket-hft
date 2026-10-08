//! Paired scoring of supplied forecasts, never a claim that model inference was run.
use crate::{
    Result,
    model::{BinaryScores, binary_scores},
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ForecastPair {
    pub id: String,
    pub observed_ms: u64,
    pub predicted_ms: u64,
    pub resolved_ms: u64,
    pub outcome: bool,
    pub candidate_probability: f64,
    pub reference_probability: f64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PairedInput {
    pub candidate_model: String,
    pub reference_model: String,
    pub dataset_id: String,
    pub split: String,
    pub synthetic: bool,
    pub records: Vec<ForecastPair>,
}
#[derive(Serialize)]
pub struct PairedReport {
    pub candidate_model: String,
    pub reference_model: String,
    pub dataset_id: String,
    pub split: String,
    pub synthetic: bool,
    pub candidate: BinaryScores,
    pub reference: BinaryScores,
    pub candidate_minus_reference_brier: f64,
    pub candidate_minus_reference_log_loss: f64,
    pub mean_brier_difference_standard_error: Option<f64>,
    pub candidate_lower_on_both_scores: bool,
    pub model_inference_measured: bool,
    pub provenance_verified: bool,
}
pub fn paired_scores(input: &PairedInput) -> Result<PairedReport> {
    if [
        &input.candidate_model,
        &input.reference_model,
        &input.dataset_id,
    ]
    .iter()
    .any(|s| s.trim().is_empty() || s.len() > 512)
        || input.candidate_model == input.reference_model
        || !["test", "heldout"].contains(&input.split.as_str())
        || input.records.is_empty()
        || input.records.len() > 10_000
    {
        return Err("invalid_evaluation_metadata".into());
    }
    let mut ids = BTreeSet::new();
    let mut p = Vec::new();
    let mut q = Vec::new();
    let mut outcomes = Vec::new();
    for r in &input.records {
        if r.id.trim().is_empty()
            || r.id.len() > 512
            || !ids.insert(&r.id)
            || r.observed_ms > r.predicted_ms
            || r.predicted_ms >= r.resolved_ms
        {
            return Err("duplicate_id_or_forecast_leakage".into());
        }
        p.push(r.candidate_probability);
        q.push(r.reference_probability);
        outcomes.push(r.outcome);
    }
    let candidate = binary_scores(&p, &outcomes)?;
    let reference = binary_scores(&q, &outcomes)?;
    let diff = candidate.brier - reference.brier;
    let log_diff = candidate.log_loss - reference.log_loss;
    let se = if p.len() > 1 {
        let variance = p
            .iter()
            .zip(&q)
            .zip(&outcomes)
            .map(|((&p, &q), &y)| {
                let y = f64::from(y);
                let d = (p - y).powi(2) - (q - y).powi(2);
                (d - diff).powi(2)
            })
            .sum::<f64>()
            / (p.len() - 1) as f64;
        Some((variance / p.len() as f64).sqrt())
    } else {
        None
    };
    Ok(PairedReport {
        candidate_model: input.candidate_model.clone(),
        reference_model: input.reference_model.clone(),
        dataset_id: input.dataset_id.clone(),
        split: input.split.clone(),
        synthetic: input.synthetic,
        candidate,
        reference,
        candidate_minus_reference_brier: diff,
        candidate_minus_reference_log_loss: log_diff,
        mean_brier_difference_standard_error: se,
        candidate_lower_on_both_scores: diff < 0.0 && log_diff < 0.0,
        model_inference_measured: false,
        provenance_verified: false,
    })
}
