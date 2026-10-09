# Readiness

Version 0.4.0 delivers a Rust paper/replay engine and benchmark suite. Live fund readiness is not established.

| Gate | Status |
|---|---|
| Polygon chain enforcement | Non-137 configurations, markets and signals reject; Base journals reject unchanged |
| Rust correctness suite | Current compact gate: 46 passed, one GPU test ignored; previous 0.3.0 native MLX release gate: 44 passed |
| Formatting and Clippy | Passed with warnings denied in release, native MLX CPU release and compact builds |
| Disk recovery | Failed workspace/duplicate release artifacts cleared; shared local cache and compact build verified in the workspace |
| CPU kernel and engine benchmarks | Completed; synthetic fixture and raw JSON included |
| Native MLX CPU | Historical 0.3.0 tests and benchmarks passed; current 0.4.0 native rerun remains pending |
| Packed I/O and buffering | Bounded input, output errors, fixed-frame and packed-weight corruption checks tested |
| Polygon order planning | Opt-in unsigned BUY/FOK plans; tick/asset/protocol/time/budget gates tested |
| Read-only Polymarket API | Public API fetched with curl and normalized by Rust; Rust HTTPS transport/continuous feed remains unverified |
| CUDA | Experimental kernel/bridge source; nvcc unavailable, no CUDA correctness or latency measured |
| Metal GPU | Unmeasured; host lacks the Metal compiler |
| News/Jev | Typed fixture gates, time/source rejection, dedup and halt recovery tested |
| Live Jev | Unverified; no API credentials, accuracy or service latency measurement |
| Probability scoring | Brier/log loss/calibration arithmetic tested; no held-out evaluation |
| Signal and math interface | Exact fee/edge/budget boundaries, expiry, engine integration and truncated-normal reference checked |
| Paired evaluation | Duplicate/leakage gates tested; supplied synthetic forecasts only, no FPT inference |
| Polygon execution | Unsigned planning only; signing, authenticated execution and reconciliation unavailable |
| Authenticated feed/order/fill adapter | Not implemented in Rust |
| Durable journal | Write-ahead replay, lock/config gates, prefix idempotence, tail recovery and corruption refusal tested |
| Drawdown/reversal | Conservative liquidation loss gate and inventory exits tested |
| Documented CLI workflows | Evidence in run-through.txt |
| Batched local comparisons | Three book depths, equal outputs, five rounds, raw samples and chart |
| Historical out-of-sample strategy evaluation | Missing historical observations, books, outcomes and venue fees |
| HRT comparison | No accessible proprietary benchmark/reference supplied |
| Public benchmark publication | [Published Gist](https://gist.github.com/shubhxho/1b15d2cd4c0dc4568bb311f627e6db0b); raw results and reproducible source included |
| Source delivery | Private repository created at `shubhxho/polymarket-hft`; push verification is recorded in the delivery response |

Current validation: polygon-check.txt, polygon-workflow.txt and polygon-installed-workflow.txt. Historical 0.3.0 logs: transport-release-tests.txt, transport-mlx-release-tests.txt and compact-check.txt. Installed q2/q4/q8 CPU/MLX equivalence is recorded in installed-packed-inference.json. Earlier native debug runs hit local disk capacity; optimized release validation is reported separately.

Remaining live gates include observed latency/reconnect tests, fill/account reconciliation, resolution audits, held-out calibration and settlement costs. Storage uses fsync, not F_FULLFSYNC; hardware power-loss durability, compaction and distributed leases are unverified. News checks declared URLs without authenticating content. Untrained weather probabilities and synthetic kernel accuracy are not evidence of alpha. Paper fees approximate flat bps and do not establish live best execution.

Simulation extension: make simulations passed 672,000 checks across 336 synthetic paths; negative P&L retained. make codegen emitted compact LLVM IR/assembly. See SIMULATIONS.md and simulation-codegen.txt.
