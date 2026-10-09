---
language:
- en
tags:
- rust
- mlx
- benchmarking
- paper-trading
- polygon
size_categories:
- 10K<n<100K
configs:
- config_name: lookup_samples
  data_files:
  - split: benchmark
    path: data/lookup-samples.csv
---

# Rust paper-engine benchmarks

Reproducible local measurements for the Rust paper engine and optional Polygon research planner. These are synthetic paper-engine and kernel benchmarks, with no live orders, historical alpha evaluation or fund-return claim.

The dataset contains **22,500 raw timing batches** from five rounds of equal-output top-of-book lookup. Three implementations—our cached lookup, a full-array scan and a BTree reference—query the same 64 synthetic books at 1, 16 and 128 levels per side. Each timed batch contains 512 lookups; each backend gets 100 warmup batches and 500 measured batches per round. Backend order rotates.

## Measured wins and mixed results

At 16 and 128 levels per side, cached lookup was **1.33–1.40x faster than BTree top lookup** across all five rounds. Compared with a full-array scan it was approximately **202–214x faster** at those depths. These ratios concern lookup only: maintaining the cache, updates, matching, network I/O and persistence are excluded.

At one level per side, BTree comparison was mixed (0.35–1.20x); there was no consistent win. See the [full comparison report](reports/MEASURED_COMPARISONS.md), [raw comparison JSON](reports/comparison.json) and [environment](reports/ENVIRONMENT.md). Ranges describe observed round-to-round variation, not confidence intervals.

![Local reference comparison](reports/comparison.svg)

Packed affine q2 weights used 384 bytes versus 4,096 bytes for the synthetic dense f32 vector: **90.63% less storage**, including packing/scales/biases. Packed reference kernels were slower than dense f32 here; memory savings are not a speed or trading-quality claim.

## Current Polygon revision

Version 0.4.0 migration evidence is in [POLYGON_MIGRATION.md](reports/POLYGON_MIGRATION.md). Its 29-row [compact CPU benchmark](reports/benchmark-polygon-compact.json) is labeled separately and cannot be used as a speed comparison against the historical release results below. CUDA remains uncompiled and unmeasured on this host.

## Contents

- data/lookup-samples.csv: viewer-ready raw batch data.
- reports/comparison.json: all rounds and raw samples with output-equality checks.
- reports/benchmark-after.json: default optimized Rust operation measurements.
- reports/benchmark-mlx-cpu.json: native MLX CPU release kernels, input creation/evaluation/readback included.
- reports/BENCHMARKS.md: timings and storage/error tables.
- reports/polygon-check.txt and reports/run-through.txt: current Polygon correctness and CLI evidence. Earlier release logs describe source version 0.3.0.
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

Current Polygon version 0.4.0 passed 46 compact-profile tests; one NVIDIA-only parity test is explicitly ignored. Historical version 0.3.0 passed 42 default and 44 native MLX CPU tests. Formatting, Clippy, replay, reversal, journal restart, news rejection, signal math, paired scoring, code generation and installation were exercised.

The Rust signal interface exports expiring candidates with exact fee/edge math, conservative probabilities and capped sizing. The weather baseline now uses conditional truncated-normal normalization, verified against a half-normal reference. The paired scoring fixture is synthetic and does not establish FPT superiority. See docs/SIGNALS_AND_MATH.md and reports/FPT_STATUS.md.

The host is macOS ARM64 with Rust 1.97.1 / LLVM 22.1.6, fat LTO and one codegen unit. CPU model, RAM and core count could not be captured because sandbox sysctl access was denied. No CPU pinning or thermal isolation was used.

Metal GPU is unmeasured because its compiler is unavailable. Jev timings use synthetic recorded responses; live API accuracy, latency and cost are unmeasured. No whole-system comparison was run against HRT, hftbacktest, Barter or the official venue client.

The current Rust engine, signals and planner enforce Polygon 137; Base inputs and historical Base journals reject. Historical version 0.3.0 release measurements remain Base-tagged synthetic research results, not new Polygon or CUDA measurements. Signing and live execution remain unavailable. Declared news source URLs are not authenticated. This dataset supports reproducible research; it does not establish live fund readiness.

Project: [GitHub](https://github.com/shubhxho/polymarket-hft). Public mirrors: [benchmark Gist](https://gist.github.com/shubhxho/1b15d2cd4c0dc4568bb311f627e6db0b).

The Rust pipeline now includes bounded reusable I/O, fixed 40-byte local updates, corruption-checked HQT1 weights, explicit fractional Kelly and an opt-in Polygon unsigned planner. CUDA source is experimental and unmeasured. Neither Polymarket nor Robinhood authenticated trading is implemented. See docs/TRANSPORT_AND_BACKENDS.md and docs/POLYGON_EXECUTION.md.

The simulation extension adds 336 deterministic synthetic paths and 672,000 engine invariant checks. Losses are retained, including a negative normal/aggressive median; no profitability claim follows. See reports/SIMULATIONS.md and docs/SIMULATIONS.md. Current CPU test gate: 46 passed, one NVIDIA-only test ignored. CUDA waiting/stream changes remain GPU-unverified.
