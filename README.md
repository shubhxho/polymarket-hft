# Polymarket HFT research engine — Rust, Base only

The active engine, weather model, risk controls, replay CLI, code generation and benchmark runner are Rust. It is a paper research engine. It does **not** submit orders, sign transactions, manage a real fund, or establish profitable trading performance.

Base mainnet (8453) is the sole accepted chain. Polymarket's [published exchange contracts](https://docs.polymarket.com/resources/contracts) are on Polygon (137). A Base-only policy therefore blocks live Polymarket settlement. Relabeling markets does not move their contracts to Base. The fixtures are synthetic Base-tagged paper markets, not Base Polymarket deployments.

## Run

Rust 1.97.1 was used for validation. Dependencies are pinned in Cargo.lock. Offline commands require those dependencies in the Cargo cache.

```sh
make check
make replay
make fair
make base
make bench
make codegen
make install
.local/bin/hft replay fixtures/weather.jsonl
```

`hft replay <jsonl|-> [config.json]` reads ordered events and emits fills, rejections and a summary as JSONL. Invalid JSON or unknown fields fail with an error. Risk rejections are normal replay output. `hft live` exits with an error explaining the settlement blocker. Configuration comes from an explicit JSON file; the Rust runtime does not load secrets or Python environment variables.

## Library

The `polymarket_hft` crate can be used directly from Rust. `Engine` owns the book and risk state; callers serialize events through that owner. No Python runtime is needed.

```rust
use polymarket_hft::engine::{Config, Engine, Event};

let mut engine = Engine::new(Config::default())?;
let event: Event = serde_json::from_str(json_line).map_err(|e| e.to_string())?;
if let Some(fill) = engine.apply(event)? {
    // Persist or display the confirmed paper fill.
}
# Ok::<(), String>(())
```

## Execution and risk

- Prices and cash use integer millionths of a dollar; sizes are whole shares. Supported tick sizes divide 1,000,000 and fit 1,001 levels. Invalid ticks are rejected rather than rounded or clamped.
- Dense fixed-size level arrays, occupancy bitmaps and cached best prices give constant-time top access and skip empty price levels during sweeps. Best-level removal scans at most 16 bitmap words.
- Paper FAK sweeps consume available levels in price order, stop when fee-adjusted edge fails, and respect cash, order, city cost-basis and inventory caps. Actual executed quantities and fees drive accounting. No naked shorts, minimum-size inflation, or assumed full fills.
- A sequence gap, malformed update or crossed update disables trading until a valid snapshot. Replay sequences are a **normalized input contract**, not an assertion that Polymarket sends these sequence numbers. An eventual feed adapter must supply continuity and reconnect semantics.
- Monotonic replay time, book age, per-second order attempts, a realized session-loss halt and an operator halt constrain execution. Loss state is per replay session; no unattended daily-reset scheduler exists.
- The untrained normal weather model handles highs and lows, continuity-corrected integer bins and observed extrema. Incomplete partitions fail instead of being normalized into false certainty. Weather events choose the eligible buy with the highest fee-adjusted top-level edge, then apply execution risk. Generic trade events support inventory exits.

Paper fees use a conservative flat bps-per-notional model rounded up per share. They are **not** a verified live Polymarket fee schedule. Prices are best within the supplied book, configured edge and risk limits; no global best-rate claim is made. The replay has no latency, queue, market-impact or gas model.

## Code generation and quantization

`schema/base.conf` is compiled by a Rust build script into checked chain and price constants. Unknown/duplicate schema keys fail the build. Serde derive generates typed JSON codecs. The release profile uses LLVM, fat LTO and one codegen unit; `make codegen` emits LLVM IR and assembly for inspection. CPU-specific compilation is opt-in with `RUSTFLAGS='-C target-cpu=native'`. No claim of state-of-the-art superiority is established by these settings.

Rust packed affine 2-, 4- and 8-bit reference kernels measure storage and numerical error. Money and risk calculations remain exact integers. Quantization is an optional research surface, not an unvalidated predictor attached to orders.

`make mlx-check` and `make mlx-bench` enable native MLX quantized matrix multiplication through the pinned Rust binding. Weights use groups of 64; each result is explicitly evaluated before timing completes. Transfers are included in MLX timings. The current native build is blocked because its MLX source download cannot resolve GitHub in this sandbox. CPU results must not be presented as MLX results. A trained model, held-out calibration and trading-quality accuracy gates are still required before inference can influence orders.

## Results and remaining gates

See [benchmark report](reports/BENCHMARKS.md), [raw results](reports/benchmark-after.json), and [readiness status](reports/READINESS.md). These are local synthetic measurements, not HRT comparisons, fund returns or live tick-to-trade latency.

The former Python prototype is preserved under `legacy/python/` for reference. Its live gateway is disabled under the Base-only policy. It is not part of the Rust runtime or current validation surface.
