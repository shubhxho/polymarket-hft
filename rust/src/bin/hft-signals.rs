use polymarket_hft::signals::{SignalRequest, generate};
use std::{
    fs::File,
    io::{self, BufRead, BufReader, Read},
};
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 1 {
        return Err("usage: hft-signals <jsonl|->".into());
    }
    let mut reader: Box<dyn BufRead> = if args[0] == "-" {
        Box::new(BufReader::new(io::stdin()))
    } else {
        Box::new(BufReader::new(File::open(&args[0])?))
    };
    loop {
        let mut bytes = Vec::new();
        let n = (&mut reader).take(65_537).read_until(b'\n', &mut bytes)?;
        if n == 0 {
            break;
        }
        if n > 65_536 {
            return Err("signal_input_line_too_large".into());
        }
        if bytes.iter().all(|b| b.is_ascii_whitespace()) {
            continue;
        }
        let request: SignalRequest = serde_json::from_slice(&bytes)?;
        match generate(&request) {
            Ok(signal) => println!("{}", serde_json::to_string(&signal)?),
            Err(reason) => println!("{}", serde_json::json!({"id":request.id,"rejected":reason})),
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
