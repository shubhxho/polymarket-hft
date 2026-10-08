# Benchmarks

Rust 0.3.0, Apple ARM64, synthetic-v2. Default backend: native MLX disabled.

![p95 benchmark latency, logarithmic scale](latency.svg)

The chart uses a log scale; exact values are labeled. Sub-timer operations, including zero medians, are below measurement resolution. These are local synthetic operation timings, not live tick-to-trade latency or fund returns.

| Operation | Warmups | Samples | p50 ns | p95 ns | p99 ns |
|---|---:|---:|---:|---:|---:|
| timer_floor | 2000 | 20000 | 0 | 42 | 42 |
| book_snapshot | 2000 | 20000 | 208 | 209 | 250 |
| book_update | 2000 | 20000 | 0 | 42 | 42 |
| book_top | 2000 | 20000 | 0 | 42 | 42 |
| book_top_full_scan_reference | 2000 | 20000 | 292 | 334 | 375 |
| depth_sweep | 2000 | 20000 | 0 | 42 | 42 |
| weather_high | 2000 | 20000 | 42 | 42 | 42 |
| weather_low | 2000 | 20000 | 41 | 42 | 42 |
| json_decode | 2000 | 20000 | 209 | 291 | 292 |
| engine_setup_and_fill | 2000 | 20000 | 917 | 1166 | 1250 |
| risk_stale_reject | 2000 | 20000 | 41 | 42 | 42 |
| dense_f32_dot_1024 | 2000 | 20000 | 458 | 583 | 584 |
| news_validate_fingerprint | 2000 | 20000 | 834 | 1083 | 1125 |
| jev_request_build_six_questions | 2000 | 20000 | 1000 | 1083 | 1125 |
| jev_recorded_response_gate | 2000 | 20000 | 167 | 167 | 208 |
| journal_halt_append_fsync | 100 | 100 | 3946584 | 4246583 | 10094583 |
| journal_recover_200_events | 100 | 100 | 260125 | 268875 | 275500 |
| book_top_btree_reference | 2000 | 20000 | 0 | 42 | 42 |
| cpu_q2_dot_1024 | 2000 | 20000 | 1000 | 1000 | 1042 |
| cpu_q4_dot_1024 | 2000 | 20000 | 1000 | 1125 | 1167 |
| cpu_q8_dot_1024 | 2000 | 20000 | 1125 | 1209 | 1250 |

## Native MLX CPU, optimized release

These operations include input creation, evaluation and readback. Batch rows process 32 vectors per call. Results measure kernels, not trained-model quality.

| Operation | Warmups | Samples | p50 ns | p95 ns | p99 ns |
|---|---:|---:|---:|---:|---:|
| mlx_q2_dot_1024_including_transfer_and_eval | 2000 | 2000 | 8417 | 9458 | 9958 |
| mlx_q2_batch32_including_transfer_and_eval | 2000 | 2000 | 42500 | 48458 | 53125 |
| mlx_q4_dot_1024_including_transfer_and_eval | 2000 | 2000 | 6708 | 9000 | 12167 |
| mlx_q4_batch32_including_transfer_and_eval | 2000 | 2000 | 19875 | 22708 | 35375 |
| mlx_q8_dot_1024_including_transfer_and_eval | 2000 | 2000 | 6167 | 8166 | 9333 |
| mlx_q8_batch32_including_transfer_and_eval | 2000 | 2000 | 22458 | 24042 | 27375 |

[Native MLX CPU release raw results](benchmark-mlx-cpu.json).

Quantization accuracy and storage are in [raw JSON](benchmark-after.json). Journal rows include fsync and are measured separately from in-memory operations. Packed kernels save storage but are slower than dense f32 on this fixture. BTree lookup and cached top both reach the timer floor; no credible speed ratio can be inferred.

Native MLX CPU release results are separately measured above. [Debug results](benchmark-mlx-cpu-debug.json) are retained as correctness-oriented evidence and are not comparable to release timings. Metal GPU is unmeasured because the host lacks the Metal compiler. Jev routing uses recorded synthetic responses; no real API latency, accuracy or cost was measured.

See [methodology](../docs/RESEARCH.md), [reference systems](COMPARISONS.md), [readiness](READINESS.md), and [check output](checks.txt). Reproduce with make bench and make report. CPU pinning, historical queue/latency modeling, real fills and HRT comparisons are absent.
