# Benchmark environment

Run date: 2026-10-09, local host. All comparisons ran on the same host and release profile.

| Field | Observed value |
|---|---|
| Rust target | aarch64-apple-darwin |
| rustc | 1.97.1, commit 8bab26f4f68e0e26f0bb7960be334d5b520ea452 |
| LLVM | 22.1.6 |
| macOS | 27.0.1, build 26A434 |
| Rust release settings | Fat LTO, one codegen unit, panic abort |
| CPU-specific flags | No target-cpu=native override |
| Native MLX | MLX 0.32.2, mlx-rs 0.32.0 with documented feature patch |
| CPU model / RAM / core count | sysctl query denied by sandbox; not independently captured |
| Process placement | No CPU pinning, isolation or thermal control |
| Input | Synthetic fixtures; no historical or live venue data |

Comparison batches contain 512 lookups across 64 markets. All backends query identical best prices and are checked for equality before timing. Five backend-order-rotated rounds retain raw samples. Median ratios are computed within each round; ranges are empirical variation, not confidence intervals.

The suite covers operation timings, numerical kernel error, storage sizes and replay behavior. It does not measure real feed latency, queue position, network trading, market impact, alpha, or a competitor's entire engine. Results should not be transferred to another machine without rerunning.

Reproduce with make compare, make bench, make mlx-cpu-bench and make report. See [research method](../docs/RESEARCH.md) and [current full check log](full-checks.txt).
