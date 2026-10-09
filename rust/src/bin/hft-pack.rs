use polymarket_hft::quant::Quantized;
use std::{
    fs::File,
    io::{Read, Write},
    path::Path,
};
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<_> = std::env::args().skip(1).collect();
    if a.len() != 4 {
        return Err("usage: hft-pack <weights.json> <output.hqt> <2|4|8> <group-size>".into());
    }
    let mut bytes = Vec::new();
    File::open(&a[0])?
        .take(64 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 64 * 1024 * 1024 {
        return Err("weights_input_too_large".into());
    }
    let weights: Vec<f32> = serde_json::from_slice(&bytes)?;
    let q = Quantized::new(&weights, a[2].parse()?, a[3].parse()?)?;
    let encoded = q.encode();
    // Refuse overwrite to protect an existing model or unrelated file.
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(Path::new(&a[1]))?;
    file.write_all(&encoded)?;
    file.sync_all()?;
    println!(
        "{}",
        serde_json::json!({"weights":q.len(),"bits":q.bits(),"group":q.group(),"packed_storage_bytes":q.storage_bytes(),"encoded_bytes":encoded.len(),"dense_bytes":weights.len()*4,"format":"HQT1","checksum":"sha256","trained_model":false})
    );
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
