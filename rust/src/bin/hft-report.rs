use serde_json::Value;
use std::{fmt::Write, fs};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let raw = fs::read_to_string("reports/benchmark-after.json")?;
    let report: Value = serde_json::from_str(&raw)?;
    let rows = report["results"].as_array().ok_or("missing results")?;
    let mut md = String::from(
        "# Benchmarks\n\nRust 0.3.0, Apple ARM64, synthetic-v2. Default backend: native MLX disabled.\n\n![p95 benchmark latency, logarithmic scale](latency.svg)\n\nThe chart uses a log scale; exact values are labeled. Sub-timer operations, including zero medians, are below measurement resolution. These are local synthetic operation timings, not live tick-to-trade latency or fund returns.\n\n| Operation | Warmups | Samples | p50 ns | p95 ns | p99 ns |\n|---|---:|---:|---:|---:|---:|\n",
    );
    let height = 100 + rows.len() * 30;
    let mut svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1000\" height=\"{height}\" viewBox=\"0 0 1000 {height}\" role=\"img\" aria-labelledby=\"title desc\"><title id=\"title\">Rust paper engine benchmark p95 latency</title><desc id=\"desc\">Logarithmic bars; nanoseconds are labeled. Synthetic local measurements. Durable filesystem sync dominates latency.</desc><rect width=\"1000\" height=\"{height}\" fill=\"#0b1220\"/><g font-family=\"monospace\" fill=\"#e5e7eb\" font-size=\"13\"><text x=\"24\" y=\"30\" font-size=\"18\">Rust paper engine: p95 operation latency</text><text x=\"24\" y=\"54\">Log scale, exact ns labels | synthetic fixture | no live venue</text>"
    );
    for (i, row) in rows.iter().enumerate() {
        let name = row["name"].as_str().ok_or("missing name")?;
        if !name.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_') {
            return Err("unsafe operation label".into());
        }
        let p95 = row["p95_ns"].as_u64().ok_or("missing p95")?;
        writeln!(
            md,
            "| {name} | {} | {} | {} | {p95} | {} |",
            row["warmup_calls"], row["samples"], row["p50_ns"], row["p99_ns"]
        )?;
        let y = 85 + i * 30;
        let width = ((p95.max(1) as f64).log10() / 8.0 * 480.0).max(1.0);
        let color = if name.starts_with("journal_") {
            "#fbbf24"
        } else {
            "#38bdf8"
        };
        write!(
            svg,
            "<text x=\"24\" y=\"{y}\">{name}</text><rect x=\"350\" y=\"{}\" width=\"{width:.1}\" height=\"15\" rx=\"3\" fill=\"{color}\"/><text x=\"{}\" y=\"{y}\">{p95} ns</text>",
            y - 12,
            360.0 + width
        )?;
    }
    svg.push_str("</g></svg>\n");
    md.push_str("\n## Storage versus dense f32\n\nPacked affine reference weights include scales, biases and packing overhead. This is storage for the synthetic vector, not model-memory or accuracy superiority.\n\n| Bits | Packed bytes | Dense bytes | Storage reduction | Absolute dot error |\n|---|---:|---:|---:|---:|\n");
    for q in report["quantization"]
        .as_array()
        .ok_or("missing quantization rows")?
    {
        let packed = q["storage_bytes"].as_u64().ok_or("missing packed bytes")?;
        let dense = q["dense_bytes"].as_u64().ok_or("missing dense bytes")?;
        if dense == 0 {
            return Err("zero dense bytes".into());
        }
        let reduction = 100.0 * (1.0 - packed as f64 / dense as f64);
        writeln!(
            md,
            "| {} | {packed} | {dense} | {reduction:.2}% | {} |",
            q["bits"], q["absolute_dot_error"]
        )?;
    }
    md.push_str("\n[Equal-output batched comparisons](MEASURED_COMPARISONS.md) report matched-round full-scan/BTree lookup results, including mixed cases.\n");
    if let Ok(raw) = fs::read_to_string("reports/benchmark-mlx-cpu.json") {
        let native: Value = serde_json::from_str(&raw)?;
        if native["release"] != true || native["mlx_backend"] != "native_cpu" {
            return Err("native report backend/profile mismatch".into());
        }
        md.push_str("\n## Native MLX CPU, optimized release\n\nThese operations include input creation, evaluation and readback. Batch rows process 32 vectors per call. Results measure kernels, not trained-model quality.\n\n| Operation | Warmups | Samples | p50 ns | p95 ns | p99 ns |\n|---|---:|---:|---:|---:|---:|\n");
        for row in native["results"].as_array().ok_or("missing native rows")? {
            let name = row["name"].as_str().ok_or("missing native name")?;
            if name.starts_with("mlx_") {
                writeln!(
                    md,
                    "| {name} | {} | {} | {} | {} | {} |",
                    row["warmup_calls"],
                    row["samples"],
                    row["p50_ns"],
                    row["p95_ns"],
                    row["p99_ns"]
                )?;
            }
        }
        md.push_str("\n[Native MLX CPU release raw results](benchmark-mlx-cpu.json).\n");
    }
    md.push_str("\nQuantization accuracy and storage are in [raw JSON](benchmark-after.json). Journal rows include fsync and are measured separately from in-memory operations. Packed kernels save storage but are slower than dense f32 on this fixture. BTree lookup and cached top both reach the timer floor; no credible speed ratio can be inferred.\n\nNative MLX CPU release results are separately measured above. [Debug results](benchmark-mlx-cpu-debug.json) are retained as correctness-oriented evidence and are not comparable to release timings. Metal GPU is unmeasured because the host lacks the Metal compiler. Jev routing uses recorded synthetic responses; no real API latency, accuracy or cost was measured.\n\nSee [methodology](../docs/RESEARCH.md), [reference systems](COMPARISONS.md), [readiness](READINESS.md), and [check output](checks.txt). Reproduce with make bench and make report. CPU pinning, historical queue/latency modeling, real fills and HRT comparisons are absent.\n");
    fs::write("reports/BENCHMARKS.md", md)?;
    fs::write("reports/latency.svg", svg)?;
    if let Ok(raw) = fs::read_to_string("reports/comparison.json") {
        let comparison: Value = serde_json::from_str(&raw)?;
        let mut csv = String::from(
            "levels_per_side,round,backend,sample_index,batch_size,batch_ns,amortized_ns_per_lookup\n",
        );
        for work in comparison["workloads"]
            .as_array()
            .ok_or("missing workloads")?
        {
            for round in work["rounds"].as_array().ok_or("missing rounds")? {
                for backend in ["cached", "full_scan", "btree"] {
                    let row = &round[backend];
                    let batch = row["batch_size"]
                        .as_u64()
                        .filter(|&n| n > 0)
                        .ok_or("invalid batch size")?;
                    for (i, ns) in row["raw_batch_ns"]
                        .as_array()
                        .ok_or("missing samples")?
                        .iter()
                        .enumerate()
                    {
                        let ns = ns.as_u64().ok_or("invalid time")?;
                        writeln!(
                            csv,
                            "{},{},{backend},{i},{batch},{ns},{}",
                            work["levels_per_side"],
                            round["round"],
                            ns as f64 / batch as f64
                        )?;
                    }
                }
            }
        }
        fs::write("reports/lookup-samples.csv", csv)?;
    }
    println!("Generated reports/BENCHMARKS.md and reports/latency.svg");
    Ok(())
}
