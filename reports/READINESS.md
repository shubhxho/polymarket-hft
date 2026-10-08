# Readiness

The deliverable is a Rust paper/replay engine and benchmark suite. Live fund readiness is not established.

| Gate | Status |
|---|---|
| Base-only chain enforcement | Tested; non-8453 configuration and markets reject |
| Rust correctness suite | 14 tests passed, including 10,000 book updates against an ordered reference |
| Formatting and Clippy | Passed with warnings denied |
| CPU kernel and engine benchmarks | Completed; synthetic fixture and raw JSON included |
| Native MLX | Blocked by upstream download DNS failure; not validated |
| Base Polymarket exchange | No verified deployment; live command rejects |
| Authenticated feed/order/fill adapter | Not implemented in Rust |
| Durable journal and restart reconciliation | Replay output available; automatic persistence/recovery not implemented |
| Historical out-of-sample strategy evaluation | Missing historical observations, books, outcomes and venue fees |
| HRT comparison | No accessible proprietary benchmark/reference supplied |
| Public benchmark publication | [Published Gist](https://gist.github.com/shubhxho/1b15d2cd4c0dc4568bb311f627e6db0b); raw results and reproducible source included |
| Source delivery | Private repository created at `shubhxho/polymarket-hft`; push verification is recorded in the delivery response |

Additional live gates include observed latency and reconnect tests, partial-fill reconciliation, account balances, resolution-source validation, model calibration, unrealized drawdown controls, settlement and gas costs. Current limits apply to simulated cost basis and realized session PnL. They do not model a real fund's full portfolio risk.
