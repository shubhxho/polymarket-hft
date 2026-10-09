# Rust source map

This polygon-shaped source map describes the Rust modules and research workflow. The paper engine remains Base-only. A separate opt-in Polygon planner prepares research order fields; it does not submit orders or migrate funds.

```mermaid
flowchart LR
    M{{"math.rs: exact quote / sizing"}} --> S{{"signals.rs: timestamped candidates"}}
    P{{"model.rs: conditional probabilities"}} -. "caller supplies forecast" .-> S
    S --> E{{"engine.rs + book.rs: paper execution gates"}}
    E --> J{{"storage.rs: write-ahead state"}}
    J -. "caller supplies resolved outcomes" .-> V{{"evaluation.rs: paired forecast scores"}}
    V -. "calibration feedback" .-> P
    P -. "fee / risk assumptions" .-> M
    N["news.rs: Jev routes"] --> J
    S -. "separate caller forecast + metadata" .-> O{{"venue.rs: Polygon order plan"}}
    O --> H["bin/hft-polymarket.rs: unsigned JSON"]
    K["buffer.rs + encoding.rs: bounded codecs"] --> C
    G["hardware.rs + cuda/packed.cu: backend selection"] --> Q
    Q["quant.rs + mlx.rs: kernel research"] --> B["bin/bench.rs + hft-compare.rs"]
    V --> R["bin/hft-evaluate.rs: evidence JSON"]
    S --> C["bin/hft-signals.rs: bounded JSONL"]
    B --> D["bin/hft-report.rs: reports / SVG / CSV"]
```

Solid arrows show supported data flow. Dashed arrows require caller-supplied forecasts/outcomes or an offline calibration step. Journal records do not include resolved outcomes; no outcome-fetching pipeline is implied.

| Source | Responsibility |
|---|---|
| rust/src/buffer.rs, encoding.rs | Bounded reusable JSONL and fixed 40-byte local frames |
| rust/src/hardware.rs, quant.rs, mlx.rs | Backend readiness, HQT1 weights and loaded inference |
| rust/src/venue.rs | Separate Polygon unsigned order plans and read-only books |
| rust/src/math.rs | Checked fee/edge/Kelly and cash-fraction arithmetic |
| rust/src/signals.rs | Base-only candidate schema, provenance declarations, expiry and caps |
| rust/src/model.rs | Truncated-normal baseline and scoring primitives |
| rust/src/evaluation.rs | Equal-record forecast comparison and leakage/ID gates |
| rust/src/engine.rs, book.rs | Authoritative paper fill/risk state |
| rust/src/storage.rs | Single-writer journal and recovery |
| rust/tests/math_signals.rs | Golden math, boundary, integration and leakage tests |

First-party Rust forbids unsafe code. The optional native MLX binding contains upstream FFI and remains separately validated. The archived Python prototype is outside the active runtime.
