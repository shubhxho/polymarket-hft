use polymarket_hft::{hardware, quant::Quantized};
use std::{fs::File, io::Read};
fn bounded(path: &str, limit: u64) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut b = Vec::new();
    File::open(path)?.take(limit + 1).read_to_end(&mut b)?;
    if b.len() as u64 > limit {
        return Err("inference_file_too_large".into());
    }
    Ok(b)
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<_> = std::env::args().skip(1).collect();
    if a.len() != 3 {
        return Err(
            "usage: hft-infer <weights.hqt> <batch.json> <auto|cpu|mlx-cpu|mlx-metal|cuda>".into(),
        );
    }
    let weights = Quantized::decode(&bounded(&a[0], 160 * 1024 * 1024)?)?;
    let input: Vec<f32> = serde_json::from_slice(&bounded(&a[1], 64 * 1024 * 1024)?)?;
    let model = hardware::LoadedModel::new(weights, &a[2])?;
    let backend = model.backend();
    let values = model.infer_batch(&input)?;
    println!(
        "{}",
        serde_json::json!({"backend":backend,"values":values,"trained_model":false,"includes_setup_and_transfer":true})
    );
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
