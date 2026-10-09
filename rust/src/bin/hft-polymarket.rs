use polymarket_hft::venue::{Forecast, MarketClient, MarketContext, Policy, normalize_book, plan};
use serde::Deserialize;
use std::{fs::File, io::Read};
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    market: MarketContext,
    policy: Policy,
    forecast: Forecast,
    now_ms: u64,
}
fn bounded(path: &str, limit: u64) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut bytes = Vec::new();
    File::open(path)?.take(limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err("venue_input_too_large".into());
    }
    Ok(bytes)
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("book") if args.len() == 2 => {
            println!("{}", MarketClient::new()?.book(&args[1])?);
        }
        Some("plan") if args.len() == 2 => {
            let input: Input = serde_json::from_slice(&bounded(&args[1], 65_536)?)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&plan(
                    &input.market,
                    &input.policy,
                    &input.forecast,
                    input.now_ms
                )?)?
            );
        }
        Some("normalize") if args.len() == 3 => {
            let book = serde_json::from_slice(&bounded(&args[1], 1_048_576)?)?;
            let market = normalize_book(&book, args[2].parse()?, false)?;
            println!("{}", serde_json::to_string_pretty(&market)?);
        }
        Some("submit") => {
            return Err("submission unavailable: signer, authenticated execution, live reconciliation and approved operating limits are not configured".into());
        }
        _ => {
            return Err("usage: hft-polymarket book <asset-id> | normalize <book.json> <rate-micro> | plan <input.json> | submit".into());
        }
    }
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
