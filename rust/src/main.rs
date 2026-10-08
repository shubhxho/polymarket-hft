use polymarket_hft::{
    Result, base,
    engine::{Config, Engine, Event},
    model::{Bucket, Metric, probabilities},
};
use serde::Deserialize;
use std::{
    fs::File,
    io::{self, BufRead, BufReader},
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Forecast {
    buckets: Vec<Bucket>,
    metric: Metric,
    observed: f64,
    mu: f64,
    sigma: f64,
    hours: f64,
}

fn run() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("fair") if args.len() == 2 => {
            let f: Forecast =
                serde_json::from_reader(File::open(&args[1]).map_err(|e| e.to_string())?)
                    .map_err(|e| e.to_string())?;
            let p = probabilities(&f.buckets, f.metric, f.observed, f.mu, f.sigma, f.hours)?;
            println!(
                "{}",
                serde_json::json!({"probabilities":p,"model":"untrained_truncated_normal","live_ready":false})
            );
        }
        Some("base") if args.len() == 1 => println!(
            "{}",
            serde_json::json!({"chain": base::CHAIN_NAME, "chain_id": base::CHAIN_ID, "rpc": base::RPC, "live": false, "reason": "No verified Base Polymarket exchange deployment; settlement unavailable under Base-only policy"})
        ),
        Some("live") => return Err(
            "Base-only live execution unavailable: no verified Base Polymarket exchange deployment"
                .into(),
        ),
        Some("replay") if (2..=3).contains(&args.len()) => {
            let config = if let Some(path) = args.get(2) {
                serde_json::from_reader(File::open(path).map_err(|e| e.to_string())?)
                    .map_err(|e| e.to_string())?
            } else {
                Config::default()
            };
            let mut engine = Engine::new(config)?;
            let input: Box<dyn BufRead> = if args[1] == "-" {
                Box::new(BufReader::new(io::stdin()))
            } else {
                Box::new(BufReader::new(
                    File::open(&args[1]).map_err(|e| e.to_string())?,
                ))
            };
            let (mut events, mut fills, mut rejects) = (0u64, 0u64, 0u64);
            for (n, line) in input.lines().enumerate() {
                let line = line.map_err(|e| e.to_string())?;
                if line.trim().is_empty() {
                    continue;
                }
                let event: Event =
                    serde_json::from_str(&line).map_err(|e| format!("line {}: {e}", n + 1))?;
                events += 1;
                match engine.apply(event) {
                    Ok(Some(fill)) => {
                        fills += 1;
                        println!("{}", serde_json::to_string(&fill).unwrap());
                    }
                    Ok(None) => (),
                    Err(reason) => {
                        rejects += 1;
                        println!("{}", serde_json::json!({"line": n + 1, "rejected": reason}));
                    }
                }
            }
            println!(
                "{}",
                serde_json::json!({"summary": {"events": events, "fills": fills, "rejects": rejects, "cash_micro": engine.cash, "realized_pnl_micro": engine.realized_pnl.to_string(), "paper": true, "chain_id": base::CHAIN_ID}})
            );
        }
        _ => return Err(
            "usage: hft base | fair <forecast.json> | replay <events.jsonl|-> [config.json] | live"
                .into(),
        ),
    }
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
