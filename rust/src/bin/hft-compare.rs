//! Equal-output, batched book-top comparisons against explicit local references.
use polymarket_hft::book::{Book, Level};
use serde_json::{Value, json};
use std::{collections::BTreeMap, fmt::Write, fs, hint::black_box, time::Instant};

const MARKETS: usize = 64;
const BATCH: usize = 512;
const SAMPLES: usize = 500;
const WARMUPS: usize = 100;
const ROUNDS: usize = 5;
type Tree = BTreeMap<u32, u32>;

fn timed(mut f: impl FnMut() -> u64) -> Value {
    for _ in 0..WARMUPS {
        black_box(f());
    }
    let mut raw = Vec::with_capacity(SAMPLES);
    for _ in 0..SAMPLES {
        let start = Instant::now();
        black_box(f());
        raw.push(start.elapsed().as_nanos() as u64);
    }
    let mut sorted = raw.clone();
    sorted.sort_unstable();
    json!({"batch_size":BATCH,"warmup_batches":WARMUPS,"samples":SAMPLES,
        "median_batch_ns":sorted[SAMPLES/2],"p95_batch_ns":sorted[SAMPLES*95/100],
        "median_amortized_ns_per_lookup":sorted[SAMPLES/2] as f64/BATCH as f64,
        "raw_batch_ns":raw})
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if cfg!(debug_assertions) {
        return Err("comparison requires --release".into());
    }
    let mut workloads = Vec::new();
    let mut claims = String::new();
    let mut markdown = String::from(
        "# Measured comparisons\n\nWe compare our cached best-price lookup against explicitly implemented full-scan and BTree references. This is an operation comparison on identical synthetic books, not a competitor engine or HRT result.\n\n64 markets, 1,001 possible price slots, 512 lookups per timed batch, 100 warmup batches, 500 samples per backend and five rounds. Backend order rotates each round. Query order is deterministic and identical across backends. Every queried best bid/ask is checked for equality before timing. Raw batch times are retained.\n\n| Levels per side | Cached median range ns/lookup | Full scan / cached median ratio range | BTree / cached median ratio range |\n|---|---:|---:|---:|\n",
    );
    let mut svg = String::from(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1000\" height=\"410\" viewBox=\"0 0 1000 410\" role=\"img\" aria-labelledby=\"title desc\"><title id=\"title\">Cached lookup versus local reference implementations</title><desc id=\"desc\">Median amortized nanoseconds per lookup over five rounds on identical synthetic books. Logarithmic bars. Lower is faster. Not a competitor engine benchmark.</desc><rect width=\"1000\" height=\"410\" fill=\"#0b1220\"/><g font-family=\"monospace\" fill=\"#e5e7eb\"><text x=\"24\" y=\"30\" font-size=\"18\">Book-top lookup: measured local references</text><text x=\"24\" y=\"54\" font-size=\"13\">Log scale | lower is faster | five rounds | identical outputs</text>",
    );
    for (case, depth) in [1usize, 16, 128].into_iter().enumerate() {
        let mut books = Vec::new();
        let mut dense = Vec::new();
        let mut trees: Vec<(Tree, Tree)> = Vec::new();
        for market in 0..MARKETS {
            let bid_top = 350 + market as u32;
            let ask_top = 600 + market as u32;
            let bids: Vec<_> = (0..depth)
                .map(|i| Level {
                    price: (bid_top - i as u32) * 1000,
                    shares: 10 + i as u32,
                })
                .collect();
            let asks: Vec<_> = (0..depth)
                .map(|i| Level {
                    price: (ask_top + i as u32) * 1000,
                    shares: 10 + i as u32,
                })
                .collect();
            let mut book = Book::new(1000)?;
            book.snapshot(&bids, &asks, 1, 1000)?;
            let mut arrays = ([0u32; 1001], [0u32; 1001]);
            for l in &bids {
                arrays.0[l.price as usize / 1000] = l.shares;
            }
            for l in &asks {
                arrays.1[l.price as usize / 1000] = l.shares;
            }
            trees.push((
                bids.iter().map(|l| (l.price, l.shares)).collect(),
                asks.iter().map(|l| (l.price, l.shares)).collect(),
            ));
            dense.push(arrays);
            books.push(book);
        }
        let mut seed = 42u64;
        let queries: Vec<usize> = (0..BATCH)
            .map(|_| {
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                ((seed >> 32) as usize) % MARKETS
            })
            .collect();
        let lookup = |backend: usize, index: usize| -> (Option<u32>, Option<u32>) {
            match backend {
                0 => {
                    let b = black_box(&books[index]);
                    (b.best_bid(), b.best_ask())
                }
                1 => {
                    let (b, a) = black_box(&dense[index]);
                    (
                        b.iter().rposition(|&s| s > 0).map(|i| i as u32 * 1000),
                        a.iter().position(|&s| s > 0).map(|i| i as u32 * 1000),
                    )
                }
                _ => {
                    let (b, a) = black_box(&trees[index]);
                    (
                        b.last_key_value().map(|(&p, _)| p),
                        a.first_key_value().map(|(&p, _)| p),
                    )
                }
            }
        };
        for &index in &queries {
            assert_eq!(lookup(0, index), lookup(1, index));
            assert_eq!(lookup(0, index), lookup(2, index));
        }
        let mut rounds = Vec::new();
        let mut medians = [Vec::new(), Vec::new(), Vec::new()];
        let mut ratios = [Vec::new(), Vec::new()];
        for round in 0..ROUNDS {
            let mut rows = vec![Value::Null; 3];
            let order: Vec<_> = (0..3).map(|i| (i + round) % 3).collect();
            for &backend in &order {
                rows[backend] = timed(|| {
                    let mut sum = 0u64;
                    for &index in black_box(&queries) {
                        let (bid, ask) = lookup(backend, index);
                        sum = sum.wrapping_add(u64::from(bid.unwrap()) + u64::from(ask.unwrap()));
                    }
                    sum
                });
                medians[backend].push(
                    rows[backend]["median_amortized_ns_per_lookup"]
                        .as_f64()
                        .unwrap(),
                );
            }
            ratios[0].push(medians[1][round] / medians[0][round]);
            ratios[1].push(medians[2][round] / medians[0][round]);
            rounds.push(json!({"round":round+1,"backend_order":order,"cached":rows[0],"full_scan":rows[1],"btree":rows[2]}));
        }
        let range = |v: &[f64]| {
            (
                v.iter().copied().fold(f64::INFINITY, f64::min),
                v.iter().copied().fold(0.0, f64::max),
            )
        };
        let (lo, hi) = range(&medians[0]);
        let (scan_lo, scan_hi) = range(&ratios[0]);
        let (tree_lo, tree_hi) = range(&ratios[1]);
        writeln!(
            markdown,
            "| {depth} | {lo:.2}–{hi:.2} | {scan_lo:.2}–{scan_hi:.2}x | {tree_lo:.2}–{tree_hi:.2}x |"
        )?;
        writeln!(
            claims,
            "- At {depth} levels per side, cached lookup was {scan_lo:.2}–{scan_hi:.2}x faster than full scan across five measured rounds."
        )?;
        if tree_lo > 1.0 {
            writeln!(
                claims,
                "  Against BTree top lookup it was {tree_lo:.2}–{tree_hi:.2}x faster in every measured round."
            )?;
        } else {
            writeln!(
                claims,
                "  Against BTree top lookup results were mixed ({tree_lo:.2}–{tree_hi:.2}x); no consistent win is established."
            )?;
        }
        for (backend, label) in ["cached", "full_scan", "btree"].into_iter().enumerate() {
            let mut values = medians[backend].clone();
            values.sort_by(f64::total_cmp);
            let value = values[ROUNDS / 2];
            let y = 88 + case * 98 + backend * 26;
            let width = (value.max(1.0).log10() / 3.0 * 480.0).max(1.0);
            let color = if backend == 0 { "#34d399" } else { "#38bdf8" };
            write!(
                svg,
                "<text x=\"24\" y=\"{y}\" font-size=\"13\">{depth} levels: {label}</text><rect x=\"250\" y=\"{}\" width=\"{width:.1}\" height=\"14\" fill=\"{color}\"/><text x=\"{}\" y=\"{y}\" font-size=\"13\">{value:.2} ns/lookup</text>",
                y - 11,
                260.0 + width
            )?;
        }
        workloads
            .push(json!({"levels_per_side":depth,"output_equality_verified":true,"rounds":rounds}));
    }
    svg.push_str("</g></svg>\n");
    markdown.push_str("\n## What we measured faster\n\n");
    markdown.push_str(&claims);
    markdown.push_str("\n![Measured reference comparison](comparison.svg)\n\nRatios greater than one mean cached lookup was faster in that round. Ranges are observed round-to-round variation, not statistical confidence intervals. These are amortized batch values, not individual lookup latency percentiles. Setup, snapshots, updates, matching, feed/network latency and durable writes are excluded. Cached metadata avoids searching; maintaining that cache has a separate cost. No CPU pinning or hardware performance counters were used.\n\nNo claim is made that we beat hftbacktest, Barter, the Polymarket client or HRT: no comparable whole-system implementation/data was run. Packed quantization storage reductions are reported separately; smaller weights do not imply faster or more accurate trading.\n\nReproduce: make compare. Raw samples: [comparison.json](comparison.json). References: [COMPARISONS.md](COMPARISONS.md).\n");
    let metadata = json!({"schema_version":1,"fixture":"equal-output-book-top-v1","release":true,"arch":std::env::consts::ARCH,"os":std::env::consts::OS,"markets":MARKETS,"workloads":workloads,"backends":["cached","full_scan","btree"],"scope":"Synthetic batched top lookup only; no competitor engine or HRT comparison"});
    fs::write(
        "reports/comparison.json",
        serde_json::to_string_pretty(&metadata)?,
    )?;
    fs::write("reports/MEASURED_COMPARISONS.md", markdown)?;
    fs::write("reports/comparison.svg", svg)?;
    println!("Generated equal-output comparison report, chart and raw samples");
    Ok(())
}
