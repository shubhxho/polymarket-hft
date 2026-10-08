# Readiness

Version 0.3.0 delivers a Rust paper/replay engine and benchmark suite. Live fund readiness is not established.

| Gate | Status |
|---|---|
| Base-only chain enforcement | Tested; non-8453 configuration and markets reject |
| Rust correctness suite | 23 default tests; 24 native CPU tests including scalar/batch q2/q4/q8 |
| Formatting and Clippy | Passed with warnings denied |
| CPU kernel and engine benchmarks | Completed; synthetic fixture and raw JSON included |
| Native MLX CPU | Tests and Clippy passed; optimized release and debug benchmarks recorded |
| Metal GPU | Unmeasured; host lacks the Metal compiler |
| News/Jev | Typed fixture gates, time/source rejection, dedup and halt recovery tested |
| Live Jev | Unverified; no API credentials, accuracy or service latency measurement |
| Probability scoring | Brier/log loss/calibration arithmetic tested; no held-out evaluation |
| Base Polymarket exchange | No verified deployment; live command rejects |
| Authenticated feed/order/fill adapter | Not implemented in Rust |
| Durable journal | Write-ahead replay, lock/config gates, prefix idempotence, tail recovery and corruption refusal tested |
| Drawdown/reversal | Conservative liquidation loss gate and inventory exits tested |
| Documented CLI workflows | Evidence in run-through.txt |
| Batched local comparisons | Three book depths, equal outputs, five rounds, raw samples and chart |
| Historical out-of-sample strategy evaluation | Missing historical observations, books, outcomes and venue fees |
| HRT comparison | No accessible proprietary benchmark/reference supplied |
| Public benchmark publication | [Published Gist](https://gist.github.com/shubhxho/1b15d2cd4c0dc4568bb311f627e6db0b); raw results and reproducible source included |
| Source delivery | Private repository created at `shubhxho/polymarket-hft`; push verification is recorded in the delivery response |

Remaining live gates include observed latency/reconnect tests, fill/account reconciliation, resolution audits, held-out calibration and settlement costs. Storage uses fsync, not F_FULLFSYNC; hardware power-loss durability, compaction and distributed leases are unverified. News checks declared URLs without authenticating content. Untrained weather probabilities and synthetic kernel accuracy are not evidence of alpha. Paper fees approximate flat bps and do not establish live best execution.
