use polymarket_hft::weather;
use std::path::Path;
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<_> = std::env::args().skip(1).collect();
    match a.first().map(String::as_str) {
        Some("generate") if (2..=4).contains(&a.len()) => {
            let days = a.get(2).map(|s| s.parse()).transpose()?.unwrap_or(20);
            let seeds = a.get(3).map(|s| s.parse()).transpose()?.unwrap_or(16);
            println!(
                "{}",
                serde_json::to_string_pretty(&weather::generate(Path::new(&a[1]), days, seeds)?)?
            );
        }
        Some("simulate") if a.len() == 2 => println!(
            "{}",
            serde_json::to_string_pretty(&weather::simulate(Path::new(&a[1]))?)?
        ),
        _ => {
            return Err(
                "usage: hft-weather generate <new-directory> [days] [seeds] | simulate <directory>"
                    .into(),
            );
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
