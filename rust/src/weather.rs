//! Synthetic weather data and cash-settled paper portfolios. No real capital or feeds.
use crate::{
    Result,
    buffer::{JsonLines, Lines},
    math::{binary_quote_with_fee, fraction_budget, fractional_kelly, polymarket_fee_bound},
    model::{Bucket, Metric, probabilities},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{BufReader, Read},
    path::Path,
    sync::mpsc::sync_channel,
};
pub const BANKROLL: u64 = 10_000_000_000;
pub const CITIES: [&str; 8] = [
    "nyc", "london", "chicago", "seoul", "dallas", "atlanta", "miami", "toronto",
];
pub const SCENARIOS: [&str; 6] = [
    "normal",
    "biased_forecast",
    "fee_spike",
    "thin",
    "delayed",
    "adverse_price",
];
fn buckets() -> Vec<Bucket> {
    vec![
        Bucket { lo: -100, hi: 60 },
        Bucket { lo: 61, hi: 65 },
        Bucket { lo: 66, hi: 70 },
        Bucket { lo: 71, hi: 75 },
        Bucket { lo: 76, hi: 200 },
    ]
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ForecastRecord {
    pub chain_id: u64,
    pub id: String,
    pub scenario: String,
    pub seed: u32,
    pub day: u32,
    pub city: String,
    pub metric: Metric,
    pub observed: f64,
    pub mu: f64,
    pub sigma: f64,
    pub hours: f64,
    pub asks: Vec<u32>,
    pub depth: u32,
    pub fee_rate_micro: u32,
    pub delay_ms: u64,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Resolution {
    pub id: String,
    pub bucket: usize,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DataManifest {
    pub synthetic: bool,
    pub records: u64,
    pub days: u32,
    pub seeds: u32,
    pub queue_capacity: usize,
    pub forecasts_sha256: String,
    pub resolutions_sha256: String,
}
fn rng(state: &mut u64) -> u64 {
    *state = state
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    *state
}
fn unit(state: &mut u64) -> f64 {
    ((rng(state) >> 11) as f64 + 0.5) / 9007199254740992.0
}
fn normal(state: &mut u64) -> f64 {
    (-2.0 * unit(state).ln()).sqrt() * (std::f64::consts::TAU * unit(state)).cos()
}
fn generated(
    scenario: &str,
    seed: u32,
    day: u32,
    index: usize,
    state: &mut u64,
) -> Result<(ForecastRecord, Resolution)> {
    let city = CITIES[index / 2];
    let metric = if index.is_multiple_of(2) {
        Metric::High
    } else {
        Metric::Low
    };
    let base = 64.0 + (index / 2) as f64;
    let observed = match metric {
        Metric::High => base - 2.,
        Metric::Low => base + 2.,
    };
    let sigma = 2.5;
    let hours = 6.;
    let w = hours / 14.;
    let mean = observed * (1. - w) + base * w;
    let std = sigma * (0.18 + 0.82 * w);
    let mut actual = None;
    for _ in 0..1024 {
        let x = mean + normal(state) * std;
        if match metric {
            Metric::High => x >= observed,
            Metric::Low => x <= observed,
        } {
            actual = Some(x.round() as i32);
            break;
        }
    }
    let actual = actual.ok_or("weather_generator_tail_sampling_failed")?;
    let bins = buckets();
    let bucket = bins
        .iter()
        .position(|b| actual >= b.lo && actual <= b.hi)
        .ok_or("weather_generator_partition")?;
    let truth = probabilities(&bins, metric, observed, base, sigma, hours)?;
    let asks = truth
        .iter()
        .map(|p| {
            let noise = (rng(state) % 61) as i64 * 1000 - 30_000;
            ((p * 1e6).round() as i64 + noise + 5000).clamp(1000, 999000) as u32 / 1000 * 1000
        })
        .collect();
    let mu = base
        + (unit(state) - 0.5) * 2.
        + if scenario == "biased_forecast" {
            5.
        } else {
            0.
        };
    let id = format!("{scenario}:{seed}:{day}:{city}:{}", index % 2);
    Ok((
        ForecastRecord {
            chain_id: crate::polygon::CHAIN_ID,
            id: id.clone(),
            scenario: scenario.into(),
            seed,
            day,
            city: city.into(),
            metric,
            observed,
            mu,
            sigma,
            hours,
            asks,
            depth: if scenario == "thin" { 1 } else { 200 },
            fee_rate_micro: if scenario == "fee_spike" {
                150_000
            } else {
                50_000
            },
            delay_ms: if scenario == "delayed" { 3500 } else { 5 },
        },
        Resolution { id, bucket },
    ))
}
fn hash(path: &Path) -> Result<String> {
    let mut f = File::open(path).map_err(|e| e.to_string())?;
    let mut h = Sha256::new();
    let mut buf = [0; 65536];
    loop {
        let n = f.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(format!("{:x}", h.finalize()))
}
pub fn generate(path: &Path, days: u32, seeds: u32) -> Result<DataManifest> {
    if !(1..=90).contains(&days) || !(1..=32).contains(&seeds) {
        return Err("invalid_weather_generation_bounds".into());
    }
    // Create an isolated artifact directory; never truncate caller data.
    fs::create_dir(path).map_err(|e| format!("weather_data_directory:{e}"))?;
    let (tx, rx) = sync_channel(64);
    let producer = std::thread::spawn(move || -> Result<()> {
        for scenario in SCENARIOS {
            for seed in 0..seeds {
                let mut state = 10_000 + u64::from(seed);
                for day in 0..days {
                    for offset in 0..16 {
                        let index = (offset + day as usize) % 16;
                        let record = generated(scenario, seed, day, index, &mut state)?;
                        if tx.send(record).is_err() {
                            return Err("weather_consumer_closed".into());
                        }
                    }
                }
            }
        }
        Ok(())
    });
    let forecasts = path.join("forecasts.jsonl");
    let resolutions = path.join("resolutions.jsonl");
    let consume = (|| -> Result<u64> {
        let mut f = JsonLines::new(
            OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&forecasts)
                .map_err(|e| e.to_string())?,
            false,
        );
        let mut r = JsonLines::new(
            OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&resolutions)
                .map_err(|e| e.to_string())?,
            false,
        );
        let mut records = 0;
        for (forecast, resolution) in &rx {
            f.record(&forecast).map_err(|e| e.to_string())?;
            r.record(&resolution).map_err(|e| e.to_string())?;
            records += 1;
        }
        f.finish().map_err(|e| e.to_string())?;
        r.finish().map_err(|e| e.to_string())?;
        Ok(records)
    })();
    drop(rx);
    let production = producer.join().map_err(|_| "weather_producer_panicked")?;
    let records = consume?;
    production?;
    let manifest = DataManifest {
        synthetic: true,
        records,
        days,
        seeds,
        queue_capacity: 64,
        forecasts_sha256: hash(&forecasts)?,
        resolutions_sha256: hash(&resolutions)?,
    };
    fs::write(
        path.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    Ok(manifest)
}
#[derive(Default, Serialize)]
pub struct Portfolio {
    pub scenario: String,
    pub seed: u32,
    pub starting_cash_micro: u64,
    pub ending_cash_micro: u64,
    pub pnl_micro: i128,
    pub max_drawdown_micro: u64,
    pub fees_micro: u64,
    pub orders: u64,
    pub evaluated_buckets: u64,
    pub skipped: BTreeMap<String, u64>,
    pub daily_equity_micro: Vec<u64>,
}
#[derive(Serialize)]
pub struct WeatherReport {
    pub chain_id: u64,
    pub synthetic: bool,
    pub live_ready: bool,
    pub profitability_established: bool,
    pub starting_bankroll_micro: u64,
    pub forecasts_sha256: String,
    pub resolutions_sha256: String,
    pub portfolios: Vec<Portfolio>,
    pub contracts: u64,
    pub evaluated_buckets: u64,
    pub assumptions: Vec<&'static str>,
}
struct Position {
    contract: usize,
    bucket: usize,
    shares: u32,
}
fn decide(
    f: &ForecastRecord,
    cash: &mut u64,
    spent: &mut u64,
    city_spent: &mut BTreeMap<String, u64>,
    out: &mut Portfolio,
    contract: usize,
) -> Result<Vec<Position>> {
    if f.chain_id != crate::polygon::CHAIN_ID
        || !SCENARIOS.contains(&f.scenario.as_str())
        || !CITIES.contains(&f.city.as_str())
        || f.asks.len() != 5
        || f.depth > 10_000
        || f.fee_rate_micro > 1_000_000
    {
        return Err("invalid_weather_record".into());
    }
    let probabilities = probabilities(&buckets(), f.metric, f.observed, f.mu, f.sigma, f.hours)?;
    let mut positions = Vec::new();
    for (bucket, (&price, &p)) in f.asks.iter().zip(&probabilities).enumerate() {
        out.evaluated_buckets += 1;
        if f.delay_ms > 2000 {
            *out.skipped.entry("stale".into()).or_default() += 1;
            continue;
        }
        let execution_price = if f.scenario == "adverse_price" {
            price.saturating_add(50_000).min(999000)
        } else {
            price
        };
        let probability = ((p * 1e6).floor() as u32).saturating_sub(30_000);
        let fee = polymarket_fee_bound(execution_price, f.fee_rate_micro, false)?;
        let quote = binary_quote_with_fee(probability, execution_price, fee)?;
        if quote.expected_edge_micro < 20_000 {
            *out.skipped.entry("edge".into()).or_default() += 1;
            continue;
        }
        let fraction = fractional_kelly(quote.full_kelly_fraction_micro, 250_000, 10_000)?;
        let used = *city_spent.get(&f.city).unwrap_or(&0);
        let budget = fraction_budget(*cash, fraction)?
            .min(100_000_000)
            .min(500_000_000u64.saturating_sub(*spent))
            .min(250_000_000u64.saturating_sub(used));
        let shares = (budget / quote.effective_cost_micro)
            .min(u64::from(f.depth))
            .min(1000) as u32;
        if shares < 5 {
            *out.skipped.entry("size_or_budget".into()).or_default() += 1;
            continue;
        }
        let cost = u64::from(shares) * quote.effective_cost_micro;
        if cost > budget || cost > *cash {
            return Err("weather_budget_invariant".into());
        }
        *cash -= cost;
        *spent += cost;
        *city_spent.entry(f.city.clone()).or_default() += cost;
        out.orders += 1;
        out.fees_micro += u64::from(shares) * fee;
        positions.push(Position {
            contract,
            bucket,
            shares,
        });
    }
    Ok(positions)
}
pub fn simulate(path: &Path) -> Result<WeatherReport> {
    let mut metadata = Vec::new();
    File::open(path.join("manifest.json"))
        .map_err(|e| e.to_string())?
        .take(4097)
        .read_to_end(&mut metadata)
        .map_err(|e| e.to_string())?;
    if metadata.len() > 4096 {
        return Err("weather_manifest_too_large".into());
    }
    let manifest: DataManifest = serde_json::from_slice(&metadata).map_err(|e| e.to_string())?;
    let forecasts_hash = hash(&path.join("forecasts.jsonl"))?;
    let resolutions_hash = hash(&path.join("resolutions.jsonl"))?;
    if !manifest.synthetic
        || !(1..=90).contains(&manifest.days)
        || !(1..=32).contains(&manifest.seeds)
        || manifest.forecasts_sha256 != forecasts_hash
        || manifest.resolutions_sha256 != resolutions_hash
    {
        return Err("weather_manifest_integrity".into());
    }

    let mut forecasts = Lines::new(
        BufReader::new(File::open(path.join("forecasts.jsonl")).map_err(|e| e.to_string())?),
        65536,
    )
    .map_err(|e| e.to_string())?;
    let mut resolutions = Lines::new(
        BufReader::new(File::open(path.join("resolutions.jsonl")).map_err(|e| e.to_string())?),
        65536,
    )
    .map_err(|e| e.to_string())?;
    let mut portfolios = Vec::new();
    let mut current: Option<Portfolio> = None;
    let mut portfolio_keys = std::collections::BTreeSet::new();
    let mut peak = BANKROLL;
    let mut contracts = 0;
    let mut cash = BANKROLL;
    loop {
        let mut day = Vec::with_capacity(16);
        for _ in 0..16 {
            match forecasts.next_line().map_err(|e| e.to_string())? {
                Some((_, bytes)) => day.push(
                    serde_json::from_slice::<ForecastRecord>(bytes).map_err(|e| e.to_string())?,
                ),
                None => break,
            }
        }
        if day.is_empty() {
            break;
        }
        if day.len() != 16 {
            return Err("weather_incomplete_day".into());
        }
        let first = &day[0];
        if first.seed >= manifest.seeds {
            return Err("weather_seed_out_of_range".into());
        }
        if current
            .as_ref()
            .is_none_or(|p| p.scenario != first.scenario || p.seed != first.seed)
        {
            if let Some(mut p) = current.take() {
                p.ending_cash_micro = cash;
                p.pnl_micro = i128::from(cash) - i128::from(BANKROLL);
                portfolios.push(p);
            }
            if !portfolio_keys.insert((first.scenario.clone(), first.seed)) {
                return Err("weather_repeated_portfolio".into());
            }
            cash = BANKROLL;
            peak = BANKROLL;
            current = Some(Portfolio {
                scenario: first.scenario.clone(),
                seed: first.seed,
                starting_cash_micro: BANKROLL,
                ..Portfolio::default()
            });
        }
        let out = current.as_mut().unwrap();
        if first.day as usize != out.daily_equity_micro.len() {
            return Err("weather_day_order".into());
        }
        let mut positions = Vec::new();
        let mut spent = 0;
        let mut city_spent = BTreeMap::new();
        let mut seen = std::collections::BTreeSet::new();
        for (index, f) in day.iter().enumerate() {
            if f.scenario != first.scenario
                || f.seed != first.seed
                || f.day != first.day
                || !seen.insert((f.city.clone(), matches!(f.metric, Metric::High)))
            {
                return Err("weather_day_identity_mismatch".into());
            }
            positions.extend(decide(
                f,
                &mut cash,
                &mut spent,
                &mut city_spent,
                out,
                index,
            )?);
            contracts += 1;
        }
        // Only after all sixteen contracts' decisions, parse any outcomes for this day.
        let mut resolved = Vec::with_capacity(16);
        for f in &day {
            let (_, bytes) = resolutions
                .next_line()
                .map_err(|e| e.to_string())?
                .ok_or("weather_missing_resolution")?;
            let r: Resolution = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
            if r.id != f.id || r.bucket >= 5 {
                return Err("weather_resolution_mismatch".into());
            }
            resolved.push(r);
        }
        for p in positions {
            if resolved[p.contract].bucket == p.bucket {
                cash = cash
                    .checked_add(u64::from(p.shares) * 1_000_000)
                    .ok_or("weather_cash_overflow")?;
            }
        }
        peak = peak.max(cash);
        out.max_drawdown_micro = out.max_drawdown_micro.max(peak.saturating_sub(cash));
        out.daily_equity_micro.push(cash);
        if contracts > 16 * 90 * 32 * 6 {
            return Err("weather_replay_limit".into());
        }
    }
    if resolutions
        .next_line()
        .map_err(|e| e.to_string())?
        .is_some()
    {
        return Err("weather_extra_resolution".into());
    }
    if let Some(mut p) = current {
        p.ending_cash_micro = cash;
        p.pnl_micro = i128::from(cash) - i128::from(BANKROLL);
        portfolios.push(p);
    }
    if portfolios.is_empty() {
        return Err("weather_empty_data".into());
    }
    if contracts != manifest.records
        || portfolios.len() != SCENARIOS.len() * manifest.seeds as usize
        || portfolios
            .iter()
            .any(|p| p.daily_equity_micro.len() != manifest.days as usize)
    {
        return Err("weather_manifest_record_count".into());
    }
    let evaluated_buckets = portfolios.iter().map(|p| p.evaluated_buckets).sum();
    Ok(WeatherReport {
        chain_id: crate::polygon::CHAIN_ID,
        synthetic: true,
        live_ready: false,
        profitability_established: false,
        starting_bankroll_micro: BANKROLL,
        forecasts_sha256: hash(&path.join("forecasts.jsonl"))?,
        resolutions_sha256: hash(&path.join("resolutions.jsonl"))?,
        portfolios,
        contracts,
        evaluated_buckets,
        assumptions: vec![
            "Independent $10000 paper portfolios per scenario/seed; all eight cities, both extrema and five buckets share each portfolio",
            "Separate bounded producer thread and forecast/resolution files; all daily decisions precede reading settlements",
            "Twenty-day default horizon; synthetic temperatures and quotes, no observed weather or historical order books",
            "Quarter Kelly, 1% fraction cap, $100/order, $500/day and $250/city/day; no leverage or reuse of same-day payouts",
            "Weather fee-rate assumption 0.05, conservative upfront cash fee; net-share/live venue rounding and gas not modeled",
            "Forecast is an untrained truncated-normal baseline; correlation exists across buckets and caps do not solve correlated Kelly",
            "Settlement is a synthetic $1 payout, losses included; no live profit, capacity or latency claim",
        ],
    })
}
