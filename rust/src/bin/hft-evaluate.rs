use polymarket_hft::evaluation::{PairedInput, paired_scores};
use std::{fs::File, io::Read};
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 1 {
        return Err("usage: hft-evaluate <paired-forecasts.json>".into());
    }
    let mut bytes = Vec::new();
    File::open(&args[0])?
        .take(4_194_305)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 4_194_304 {
        return Err("evaluation_input_too_large".into());
    }
    let input: PairedInput = serde_json::from_slice(&bytes)?;
    println!("{}", serde_json::to_string_pretty(&paired_scores(&input)?)?);
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
