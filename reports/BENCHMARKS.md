# Benchmarks

Rust 0.3.0, Apple ARM64, synthetic-v2. Default backend: native MLX disabled.

![p95 benchmark latency, logarithmic scale](latency.svg)

The chart uses a log scale; exact values are labeled. Sub-timer operations, including zero medians, are below measurement resolution. These are local synthetic operation timings, not live tick-to-trade latency or fund returns.

| Operation | Warmups | Samples | p50 ns | p95 ns | p99 ns |
|---|---:|---:|---:|---:|---:|
| timer_floor | 2000 | 20000 | 0 | 42 | 42 |
| book_snapshot | 2000 | 20000 | 417 | 417 | 500 |
| book_update | 2000 | 20000 | 41 | 42 | 42 |
| book_top | 2000 | 20000 | 41 | 42 | 42 |
| book_top_full_scan_reference | 2000 | 20000 | 500 | 792 | 834 |
| depth_sweep | 2000 | 20000 | 42 | 42 | 42 |
| weather_high | 2000 | 20000 | 83 | 84 | 125 |
| weather_low | 2000 | 20000 | 42 | 84 | 84 |
| json_decode | 2000 | 20000 | 417 | 500 | 542 |
| engine_setup_and_fill | 2000 | 20000 | 1250 | 1667 | 1709 |
| risk_stale_reject | 2000 | 20000 | 41 | 42 | 42 |
| dense_f32_dot_1024 | 2000 | 20000 | 500 | 500 | 542 |
| news_validate_fingerprint | 2000 | 20000 | 833 | 875 | 917 |
| jev_request_build_six_questions | 2000 | 20000 | 1000 | 1084 | 1125 |
| jev_recorded_response_gate | 2000 | 20000 | 166 | 167 | 167 |
| journal_halt_append_fsync | 100 | 100 | 3928250 | 4197875 | 5086666 |
| journal_recover_200_events | 100 | 100 | 322833 | 426334 | 452000 |
| book_top_btree_reference | 2000 | 20000 | 0 | 42 | 42 |
| cpu_q2_dot_1024 | 2000 | 20000 | 959 | 1000 | 1042 |
| cpu_q4_dot_1024 | 2000 | 20000 | 959 | 1000 | 1041 |
| cpu_q8_dot_1024 | 2000 | 20000 | 959 | 1000 | 1042 |

## Storage versus dense f32

Packed affine reference weights include scales, biases and packing overhead. This is storage for the synthetic vector, not model-memory or accuracy superiority.

| Bits | Packed bytes | Dense bytes | Storage reduction | Absolute dot error |
|---|---:|---:|---:|---:|
| 2 | 384 | 4096 | 90.62% | 0.05687808990478515 |
| 4 | 640 | 4096 | 84.38% | 0.14974403381347656 |
| 8 | 1152 | 4096 | 71.88% | 0.04233837127685547 |

[Equal-output batched comparisons](MEASURED_COMPARISONS.md) report matched-round full-scan/BTree lookup results, including mixed cases.

## Native MLX CPU, optimized release

These operations include input creation, evaluation and readback. Batch rows process 32 vectors per call. Results measure kernels, not trained-model quality.

| Operation | Warmups | Samples | p50 ns | p95 ns | p99 ns |
|---|---:|---:|---:|---:|---:|
| mlx_q2_dot_1024_including_transfer_and_eval | 2000 | 2000 | 7417 | 9500 | 11708 |
| mlx_q2_batch32_including_transfer_and_eval | 2000 | 2000 | 41375 | 50084 | 54167 |
| mlx_q4_dot_1024_including_transfer_and_eval | 2000 | 2000 | 7500 | 8917 | 11917 |
| mlx_q4_batch32_including_transfer_and_eval | 2000 | 2000 | 21125 | 25167 | 28500 |
| mlx_q8_dot_1024_including_transfer_and_eval | 2000 | 2000 | 7459 | 9084 | 14500 |
| mlx_q8_batch32_including_transfer_and_eval | 2000 | 2000 | 20292 | 68750 | 108958 |

[Native MLX CPU release raw results](benchmark-mlx-cpu.json).

Quantization accuracy and storage are in [raw JSON](benchmark-after.json). Journal rows include fsync and are measured separately from in-memory operations. Packed kernels save storage but are slower than dense f32 on this fixture. BTree lookup and cached top both reach the timer floor; no credible speed ratio can be inferred.

Native MLX CPU release results are separately measured above. [Debug results](benchmark-mlx-cpu-debug.json) are retained as correctness-oriented evidence and are not comparable to release timings. Metal GPU is unmeasured because the host lacks the Metal compiler. Jev routing uses recorded synthetic responses; no real API latency, accuracy or cost was measured.

See [methodology](../docs/RESEARCH.md), [reference systems](COMPARISONS.md), [readiness](READINESS.md), and [check output](checks.txt). Reproduce with make bench and make report. CPU pinning, historical queue/latency modeling, real fills and HRT comparisons are absent.
