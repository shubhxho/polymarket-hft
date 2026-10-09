# Full workflow rerun — 2026-10-09

The compact profile passed all 42 tests, formatting and Clippy with warnings denied. The focused transport suite passed all 11 tests. Previously installed native MLX CPU release binaries from source commit 9b67b41 passed the complete CLI workflow in both the workspace and publication checkout; see installed-workflow-rerun.txt. This mode explicitly skips rebuilding binaries.

The workflow exercised Base paper replay, weather, reversals, signals, paired scoring, Polygon unsigned planning and submission refusal, quantized pack/load/CPU inference, hardware selection, durable restart, news deduplication and live refusal. The Rust normalizer also processed the previously downloaded public Polymarket book; this is a read-only smoke check, not a latency benchmark.

CUDA build was attempted again and failed because nvcc is absent. No NVIDIA GPU is available on this Apple silicon host. CUDA source is published but CUDA compilation, runtime correctness and latency remain unmeasured. The Rust HTTPS book request returned market_data_network_error in this restricted runtime; its actual network transport remains unverified.

No live orders were submitted. These checks do not establish profitability, competitive superiority or end-to-end exchange latency.
