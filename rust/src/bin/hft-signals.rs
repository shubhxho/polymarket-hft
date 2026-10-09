use polymarket_hft::{
    buffer::{JsonLines, Lines},
    signals::{SignalRequest, generate},
};
use std::{
    fs::File,
    io::{self, BufRead, BufReader},
};
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 1 && !(args.len() == 2 && args[1] == "--batch-output") {
        return Err("usage: hft-signals <jsonl|-> [--batch-output]".into());
    }
    let reader: Box<dyn BufRead> = if args[0] == "-" {
        Box::new(BufReader::with_capacity(64 * 1024, io::stdin()))
    } else {
        Box::new(BufReader::with_capacity(64 * 1024, File::open(&args[0])?))
    };
    let mut lines = Lines::new(reader, 65_536)?;
    let stdout = io::stdout();
    let mut output = JsonLines::new(stdout.lock(), args.len() == 1);
    while let Some((n, bytes)) = lines.next_line()? {
        if bytes.iter().all(|b| b.is_ascii_whitespace()) {
            continue;
        }
        let request: SignalRequest =
            serde_json::from_slice(bytes).map_err(|e| format!("line {n}: {e}"))?;
        match generate(&request) {
            Ok(signal) => output.record(&signal)?,
            Err(reason) => {
                output.record(&serde_json::json!({"id":request.id,"rejected":reason}))?
            }
        }
    }
    output.finish()?;
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
