---
language:
- en
tags:
- rust
- mlx
- benchmarking
- paper-trading
- base
size_categories:
- 10K<n<100K
configs:
- config_name: lookup_samples
  data_files:
  - split: benchmark
    path: data/lookup-samples.csv
---

# Rust paper-engine benchmarks

Reproducible local measurements for the Base-only Polymarket HFT research engine, version 0.3.0. These are synthetic paper-engine and kernel benchmarks, with no live orders, historical alpha evaluation or fund-return claim.

The dataset contains **22,500 raw timing batches** from five rounds of equal-output top-of-book lookup. Three implementations—our cached lookup, a full-array scan and a BTree reference—query the same 64 synthetic books at 1, 16 and 128 levels per side. Each timed batch contains 512 lookups; each backend gets 100 warmup batches and 500 measured batches per round. Backend order rotates.

## Measured wins and mixed results

At 16 and 128 levels per side, cached lookup was **1.33–1.40x faster than BTree top lookup** across all five rounds. Compared with a full-array scan it was approximately **202–214x faster** at those depths. These ratios concern lookup only: maintaining the cache, updates, matching, network I/O and persistence are excluded.

At one level per side, BTree comparison was mixed (0.35–1.20x); there was no consistent win. See the [full comparison report](reports/MEASURED_COMPARISONS.md), [raw comparison JSON](reports/comparison.json) and [environment](reports/ENVIRONMENT.md). Ranges describe observed round-to-round variation, not confidence intervals.

![Local reference comparison](reports/comparison.svg)

Packed affine q2 weights used 384 bytes versus 4,096 bytes for the synthetic dense f32 vector: **90.63% less storage**, including packing/scales/biases. Packed reference kernels were slower than dense f32 here; memory savings are not a speed or trading-quality claim.

## Contents

- data/lookup-samples.csv: viewer-ready raw batch data.
- reports/comparison.json: all rounds and raw samples with output-equality checks.
- reports/benchmark-after.json: default optimized Rust operation measurements.
- reports/benchmark-mlx-cpu.json: native MLX CPU release kernels, input creation/evaluation/readback included.
- reports/BENCHMARKS.md: timings and storage/error tables.
- reports/full-checks.txt and reports/run-through.txt: correctness and CLI evidence.
- docs/: architecture, operations, schemas and research methods.
- source.tar.gz.base64 and REPRODUCE.md: reproducible Rust source bundle and its SHA-256.

## Columns

| Column | Meaning |
|---|---|
| levels_per_side | Number of occupied levels on each side |
| round | One of five independently timed rounds |
| backend | cached, full_scan or btree |
| sample_index | Batch index within the round/backend |
| batch_size | 512 lookups |
| batch_ns | Wall-clock nanoseconds for the entire batch |
| amortized_ns_per_lookup | batch_ns divided by batch_size; not individual-request latency |

## Validation and limits

23 default Rust tests and 24 native MLX CPU tests passed. Formatting, Clippy, replay, reversal, journal restart, news rejection, code generation and installation were exercised.

The host is macOS ARM64 with Rust 1.97.1 / LLVM 22.1.6, fat LTO and one codegen unit. CPU model, RAM and core count could not be captured because sandbox sysctl access was denied. No CPU pinning or thermal isolation was used.

Metal GPU is unmeasured because its compiler is unavailable. Jev timings use synthetic recorded responses; live API accuracy, latency and cost are unmeasured. No whole-system comparison was run against HRT, hftbacktest, Barter or the official venue client.

Base mainnet is the only allowed chain. No compatible Base Polymarket settlement deployment is verified, so live execution refuses to run. Declared news source URLs are not authenticated. This dataset supports reproducible research; it does not establish live fund readiness.

Project: [GitHub](https://github.com/shubhxho/polymarket-hft). Public mirrors: [benchmark Gist](https://gist.github.com/shubhxho/1b15d2cd4c0dc4568bb311f627e6db0b).
