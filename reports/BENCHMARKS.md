# Rust HFT benchmark report

Local synthetic microbenchmarks on Apple Silicon (`aarch64`, macos, rustc 1.97.1 (8bab26f4f 2026-07-14)). Release profile: fat LTO, one codegen unit. The Rust runner uses 2,000 warm-up calls and 20,000 measured calls per CPU case. Each sample includes its own wall-clock timer; CPU affinity and scheduler isolation are not controlled.

| Case | p50 ns | p95 ns | p99 ns | Samples |
|---|---:|---:|---:|---:|
| timer_floor | 0 | 42 | 42 | 20000 |
| book_snapshot | 417 | 500 | 500 | 20000 |
| book_update | 41 | 42 | 42 | 20000 |
| book_top | 41 | 42 | 42 | 20000 |
| book_top_full_scan_reference | 500 | 625 | 833 | 20000 |
| depth_sweep | 41 | 42 | 42 | 20000 |
| weather_high | 42 | 125 | 125 | 20000 |
| weather_low | 42 | 42 | 84 | 20000 |
| json_decode | 334 | 459 | 584 | 20000 |
| engine_setup_and_fill | 1250 | 1667 | 2667 | 20000 |
| risk_stale_reject | 42 | 42 | 83 | 20000 |
| dense_f32_dot_1024 | 500 | 542 | 875 | 20000 |
| cpu_q2_dot_1024 | 1000 | 1250 | 1250 | 20000 |
| cpu_q4_dot_1024 | 1000 | 1250 | 4625 | 20000 |
| cpu_q8_dot_1024 | 1000 | 1209 | 4708 | 20000 |

Cached best-price access is compared with a full-array scan reference in the same executable. Values near the timer floor cannot support precise speed ratios. `engine_setup_and_fill` includes engine creation, allocations, market registration, snapshot construction and JSON decoding; it is not live tick-to-trade latency.

| CPU quantization | Storage bytes | Dense bytes | Absolute dot error |
|---|---:|---:|---:|
| 2-bit | 384 | 4096 | 0.056878 |
| 4-bit | 640 | 4096 | 0.149744 |
| 8-bit | 1152 | 4096 | 0.042338 |

Quantization is measured on a synthetic 1,024-weight vector and reports one dot-product error. This is not a trained model evaluation. The CPU packed reference kernels reduce storage but must be compared with dense inference on both accuracy and latency. Lowest precision is not automatically the best choice. MLX results are absent: the native dependency's upstream source download was blocked by DNS failure. No MLX acceleration claim is established.

## Reproduce

```sh
make check
make bench
make replay
make codegen
# On a provisioned Apple Silicon host with upstream MLX source access:
make mlx-check
make mlx-bench
```

See `rust/src/bin/bench.rs` for fixtures and timing boundaries, `Cargo.lock` for pinned Rust dependencies, and `reports/benchmark-after.json` for raw results. Offline builds require a populated Cargo cache. The older `benchmark-before.json` records the initial full-array implementation from this session; its source revision was not separately committed, so use the included full-scan reference for reproducible comparisons.

## Scope

There is no HRT reference dataset or proprietary benchmark here. These measurements establish neither HRT competitiveness, state-of-the-art performance, profitable alpha nor the best available execution rate. Network latency, authenticated orders, live fills, fee schedules, gas, historical forecasts, held-out outcomes and portfolio drawdown were not benchmarked. Base-only policy blocks current live Polymarket settlement because the published exchange contracts use Polygon.

Sources: [Polymarket contracts](https://docs.polymarket.com/resources/contracts), [Base network](https://docs.base.org/get-started/connect-to-base), [MLX quantized matrix multiplication](https://ml-explore.github.io/mlx/build/html/python/_autosummary/mlx.core.quantized_matmul.html).
