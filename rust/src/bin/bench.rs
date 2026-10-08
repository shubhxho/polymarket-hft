use polymarket_hft::{
    book::{Book, Level, Side},
    engine::{Config, Engine, Event},
    model::{Bucket, Metric, probabilities},
    news::{JevResponse, NewsItem, NewsPipeline, evaluate, request},
    quant::Quantized,
    storage::Journal,
};
use serde_json::{Value, json};
use std::{hint::black_box, time::Instant};

fn measure(name: &str, n: usize, mut f: impl FnMut()) -> Value {
    let warmup = n.min(2000);
    for _ in 0..warmup {
        f();
    }
    let mut samples = Vec::with_capacity(n);
    let wall = Instant::now();
    for _ in 0..n {
        let start = Instant::now();
        f();
        samples.push(start.elapsed().as_nanos() as u64);
    }
    let elapsed = wall.elapsed().as_secs_f64();
    samples.sort_unstable();
    json!({"name": name, "samples": n, "warmup_calls":warmup, "p50_ns": samples[n/2], "p95_ns": samples[n*95/100], "p99_ns": samples[n*99/100], "max_ns": samples[n-1], "samples_per_second_with_timer": n as f64 / elapsed})
}
fn setup() -> Engine {
    let mut e = Engine::new(Config::default()).unwrap();
    e.apply(Event::Market {
        token: "t".into(),
        city: "nyc".into(),
        tick: 1000,
        chain_id: 8453,
        fee_bps: 20,
    })
    .unwrap();
    e.apply(Event::Snapshot {
        token: "t".into(),
        bids: vec![Level {
            price: 400000,
            shares: 100,
        }],
        asks: vec![Level {
            price: 450000,
            shares: 100,
        }],
        sequence: 1,
        time_ms: 1000,
    })
    .unwrap();
    e
}
fn main() {
    let n = 20000;
    let mut results = Vec::new();
    results.push(measure("timer_floor", n, || {
        black_box(1);
    }));
    let mut b = Book::new(1000).unwrap();
    let bids = [Level {
        price: 400000,
        shares: 100,
    }];
    let asks = [Level {
        price: 450000,
        shares: 100,
    }];
    let mut seq = 1;
    results.push(measure("book_snapshot", n, || {
        b.snapshot(black_box(&bids), black_box(&asks), seq, seq)
            .unwrap();
        seq += 1;
    }));
    results.push(measure("book_update", n, || {
        b.update(
            Side::Buy,
            Level {
                price: 400000,
                shares: black_box(99),
            },
            seq,
            seq,
        )
        .unwrap();
        seq += 1;
    }));
    results.push(measure("book_top", n, || {
        let book = black_box(&b);
        black_box((book.best_bid(), book.best_ask()));
    }));
    let mut scan_bids = [0u32; 1001];
    let mut scan_asks = [0u32; 1001];
    scan_bids[400] = 100;
    scan_asks[450] = 100;
    results.push(measure("book_top_full_scan_reference", n, || {
        black_box((
            black_box(&scan_bids).iter().rposition(|&s| s > 0),
            black_box(&scan_asks).iter().position(|&s| s > 0),
        ));
    }));
    results.push(measure("depth_sweep", n, || {
        black_box(
            b.levels(Side::Buy)
                .map(|l| u64::from(l.price) * u64::from(l.shares))
                .sum::<u64>(),
        );
    }));
    let buckets = [
        Bucket { lo: -100, hi: 69 },
        Bucket { lo: 70, hi: 71 },
        Bucket { lo: 72, hi: 200 },
    ];
    results.push(measure("weather_high", n, || {
        black_box(probabilities(&buckets, Metric::High, black_box(70.0), 72.0, 1.8, 6.0).unwrap());
    }));
    results.push(measure("weather_low", n, || {
        black_box(probabilities(&buckets, Metric::Low, black_box(72.0), 70.0, 1.8, 6.0).unwrap());
    }));
    let wire = r#"{"type":"trade","token":"t","side":"buy","fair_micro":700000,"shares":20,"time_ms":1001}"#;
    results.push(measure("json_decode", n, || {
        black_box(serde_json::from_str::<Event>(black_box(wire)).unwrap());
    }));
    // Includes setup/allocation: explicitly not an isolated tick-to-order latency.
    results.push(measure("engine_setup_and_fill", n, || {
        let mut e = setup();
        black_box(e.apply(serde_json::from_str(wire).unwrap()).unwrap());
    }));
    let mut e = setup();
    results.push(measure("risk_stale_reject", n, || {
        black_box(
            e.apply(Event::Trade {
                token: "t".into(),
                side: Side::Buy,
                fair_micro: 700000,
                shares: 20,
                time_ms: 5000,
            })
            .unwrap_err(),
        );
    }));
    let weights: Vec<f32> = (0..1024).map(|i| (i as f32 * 0.1).sin()).collect();
    let input: Vec<f32> = (0..1024).map(|i| (i as f32 * 0.03).cos()).collect();
    let reference: f32 = weights.iter().zip(&input).map(|(a, b)| a * b).sum();
    results.push(measure("dense_f32_dot_1024", n, || {
        black_box(
            black_box(&weights)
                .iter()
                .zip(black_box(&input))
                .map(|(a, b)| a * b)
                .sum::<f32>(),
        );
    }));
    let mut accuracy = Vec::new();
    let item: serde_json::Value = serde_json::from_str(
        include_str!("../../../fixtures/news.jsonl")
            .lines()
            .next()
            .unwrap(),
    )
    .unwrap();
    let news: NewsItem = serde_json::from_value(item["item"].clone()).unwrap();
    let response: JevResponse = serde_json::from_value(item["response"].clone()).unwrap();
    results.push(measure("news_validate_fingerprint", n, || {
        black_box(NewsPipeline::default().validate(black_box(&news)).unwrap());
    }));
    results.push(measure("jev_request_build_six_questions", n, || {
        black_box(request(black_box(&news), "jev-latest"));
    }));
    results.push(measure("jev_recorded_response_gate", n, || {
        black_box(evaluate(black_box(&response)).unwrap());
    }));
    let journal_path =
        std::env::temp_dir().join(format!("hft-bench-{}.journal", std::process::id()));
    assert!(!journal_path.exists(), "benchmark journal already exists");
    let mut journal = Journal::open(&journal_path, None).unwrap();
    results.push(measure("journal_halt_append_fsync", 100, || {
        black_box(
            journal
                .apply(Event::Halt {
                    reason: "benchmark".into(),
                })
                .unwrap()
                .unwrap(),
        );
    }));
    drop(journal);
    results.push(measure("journal_recover_200_events", 100, || {
        black_box(Journal::open(&journal_path, None).unwrap());
    }));
    std::fs::remove_file(&journal_path).unwrap();
    let ordered_bids = std::collections::BTreeMap::from([(400000u32, 100u32)]);
    let ordered_asks = std::collections::BTreeMap::from([(450000u32, 100u32)]);
    results.push(measure("book_top_btree_reference", n, || {
        black_box((
            black_box(&ordered_bids).last_key_value(),
            black_box(&ordered_asks).first_key_value(),
        ));
    }));
    for bits in [2, 4, 8] {
        let q = Quantized::new(&weights, bits, 64).unwrap();
        results.push(measure(&format!("cpu_q{bits}_dot_1024"), n, || {
            black_box(q.dot(black_box(&input)).unwrap());
        }));
        accuracy.push(json!({"backend":"cpu_reference", "bits":bits, "absolute_dot_error": (q.dot(&input).unwrap()-reference).abs(), "storage_bytes":q.storage_bytes(), "dense_bytes": weights.len()*4}));
        #[cfg(any(feature = "mlx", feature = "mlx-cpu"))]
        {
            let q = polymarket_hft::mlx::MlxLinear::new(&weights, i32::from(bits)).unwrap();
            results.push(measure(
                &format!("mlx_q{bits}_dot_1024_including_transfer_and_eval"),
                2000,
                || {
                    black_box(q.dot(black_box(&input)).unwrap());
                },
            ));
            accuracy.push(json!({"backend":"mlx", "bits":bits, "absolute_dot_error":(q.dot(&input).unwrap()-reference).abs()}));
            let batch = input.repeat(32);
            results.push(measure(
                &format!("mlx_q{bits}_batch32_including_transfer_and_eval"),
                2000,
                || {
                    black_box(q.dot_batch(black_box(&batch)).unwrap());
                },
            ));
        }
    }
    let rustc = std::process::Command::new("rustc")
        .arg("--version")
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned());
    println!("{}", serde_json::to_string_pretty(&json!({"schema_version":2,"package_version":env!("CARGO_PKG_VERSION"),"fixture":"synthetic-v2", "arch":std::env::consts::ARCH,"os":std::env::consts::OS,"rustc":rustc,"release":!cfg!(debug_assertions),"mlx_enabled":cfg!(any(feature="mlx", feature="mlx-cpu")),"mlx_backend":if cfg!(feature="mlx"){"metal"}else if cfg!(feature="mlx-cpu"){"native_cpu"}else{"disabled"},"results":results,"quantization":accuracy,"limitations":["Synthetic single-process wall-clock microbenchmarks with timer overhead; no CPU pinning", "No HRT proprietary reference, network latency, historical alpha, gas or live fills measured", "Quantization accuracy is a synthetic kernel test; no trained model or trading-quality claim", "Jev gates use a recorded synthetic response; network/model quality is not measured", "Journal samples use fsync without macOS F_FULLFSYNC; power-loss guarantees depend on the filesystem"]})).unwrap());
}
