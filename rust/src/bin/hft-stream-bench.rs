//! Equal-output in-memory stream benchmark; not socket or exchange latency.
use polymarket_hft::buffer::{JsonLines, Lines};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    hint::black_box,
    io::{self, BufReader, Cursor, Write},
    time::Instant,
};
#[derive(Default)]
struct Sink {
    bytes: Vec<u8>,
    writes: u64,
    flushes: u64,
}
impl Write for Sink {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.writes += 1;
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        self.flushes += 1;
        Ok(())
    }
}
fn emit(flush_each: bool) -> Sink {
    let mut sink = Sink::default();
    {
        let mut out = JsonLines::new(&mut sink, flush_each);
        for i in 0..128 {
            out.record(&json!({"sequence":i,"chain_id":137,"token":"synthetic-stream-token","price_micro":450000,"shares":20,"paper":true})).unwrap();
        }
        out.finish().unwrap();
    }
    sink
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let immediate = emit(true);
    let batch = emit(false);
    assert_eq!(immediate.bytes, batch.bytes);
    let mut timings = Vec::new();
    for capacity in [2usize, 4096, 65536] {
        let mut samples = Vec::new();
        for iteration in 0..105 {
            let start = Instant::now();
            let mut lines = Lines::new(
                BufReader::with_capacity(capacity, Cursor::new(&batch.bytes)),
                65536,
            )?;
            let mut count = 0;
            let mut bytes = 0;
            while let Some((_, line)) = lines.next_line()? {
                count += 1;
                bytes += black_box(line).len();
            }
            assert_eq!(count, 128);
            assert_eq!(bytes, batch.bytes.len());
            if iteration >= 5 {
                samples.push(start.elapsed().as_nanos() as u64);
            }
        }
        samples.sort_unstable();
        timings.push(json!({"reader_capacity_bytes":capacity,"records_per_batch":128,"batch_bytes":batch.bytes.len(),"warmup_batches":5,"samples":100,"p50_batch_ns":samples[50],"p95_batch_ns":samples[95],"raw_sorted_batch_ns":samples}));
    }
    println!(
        "{}",
        serde_json::to_string_pretty(
            &json!({"schema_version":1,"package_version":env!("CARGO_PKG_VERSION"),"release":!cfg!(debug_assertions),"arch":std::env::consts::ARCH,"os":std::env::consts::OS,"fixture":"equal-output-stream-v1","output_equality_verified":true,"output_sha256":format!("{:x}",Sha256::digest(&batch.bytes)),"immediate":{"writes":immediate.writes,"flushes":immediate.flushes},"batch":{"writes":batch.writes,"flushes":batch.flushes},"reader_batches":timings,"limitations":["In-memory sink: flushes are method calls, not fsync or socket syscalls","Batch latency includes reader setup; excludes JSON parsing, venue transport and inference","Compact results are diagnostics; no competitor or end-to-end HFT speed claim"]})
        )?
    );
    Ok(())
}
