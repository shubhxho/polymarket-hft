# Research and measurement

The untrained normal weather baseline conditions high/low distributions on observed extrema and applies continuity correction to integer bins. Incomplete partitions reject instead of silently renormalizing.

binary_scores computes Brier score, clipped binary log loss and ten-bin calibration error. Tests verify arithmetic, not forecasting skill. Evaluation needs temporally held-out observations, exact station/resolution mappings, books, outcomes, fees and latency. Compare against climatology and market probabilities using identical samples. Separate training, calibration and evaluation splits.

Packed Rust q2/q4/q8 affine kernels save storage but unpacking can be slower than dense f32 at small sizes. Lower bits need not produce lower error on every vector. Native MLX supports scalar and batch quantized matmul. These kernels do not drive orders.

make bench publishes warmups, samples, p50/p95/p99/max nanoseconds and timer-inclusive throughput. Extremely short operations fall at the timer floor, including zero medians. The unpinned synthetic single-process fixture is not a production latency benchmark. BTree/full-scan comparisons concern only top lookup on that fixture. Journal timings include fsync; engine-only times must not be advertised as durable latency.

The benchmark SVG uses log-scaled p95 bars and exact labels. Native CPU and Metal must be separately labeled. Recorded Jev gates exclude network cost, latency and model quality. No comparable HRT proprietary implementation is available.

Live readiness requires a compatible Base venue, authenticated adapters, reconnect/sequence handling, queue and latency modeling, fill/account reconciliation, resolution audits and held-out net-performance evaluation. The live command remains blocked.
