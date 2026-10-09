# Code generation and simulations

Two local entry points:

```sh
make codegen
make simulations
```

Both default to the compact profile to fit this development machine. `make codegen` emits LLVM IR and assembly in Cargo's target directory. Use `HFT_PROFILE=release make codegen` for release/LTO inspection when adequate disk space is available; compact assembly is not the release performance artifact.

`make simulations` requires formatting, all compact tests and Clippy to pass first. It runs the actual Rust Engine over 336 deterministic paths: seven scenarios × three fixed policies × sixteen seeds, 2,000 steps each, totaling 672,000 accounting/limit checks. A test or invariant failure returns nonzero; the previous successful JSON is not overwritten. Outputs are reports/simulations.json, reports/SIMULATIONS.md and reports/simulations.svg. The CLI can run smaller/larger bounded suites: `hft-simulations 1000 8 --report`.

Each path checks charged fees, cash conservation on buys/sells, inventory changes, liquidity bounds, order/position limits, Polygon IDs and unchanged holdings after rejected trades. Fault scenarios exercise stale data, sequence gaps, thin books, bursts, higher fees and biased forecasts through a crash. Both policy and seed schedule are fixed before reporting; there is no search for the most profitable configuration. The synthetic latent price is visible to the toy forecast, making its information advantage artificial. Separate seeds do not establish historical out-of-sample alpha.

The simulation gate is a correctness gate. Losses are retained. A loss guard can stop new buys after a shock but cannot guarantee a maximum market loss or drawdown. Zero P&L may mean no fills. Marked P&L includes replay fees and available bid-depth valuation, not binary settlement, gas or actual liquidation. There is no calibrated price/latency model, maker queue, market impact, network feed or actual venue fill model. `live_ready` and `profitability_established` remain false, and the live command still refuses execution.

For inspiration, the requested [LinkedIn engineering discussion](https://www.linkedin.com/posts/mehul-gosavi_softwareengineering-hft-lowlatency-activity-7480104569654652928-TtVm) suggests testing sequence gaps, latency and transaction costs. It is context, not validation of this system. The [hftbacktest fill-model documentation](https://hftbacktest.readthedocs.io/en/latest/order_fill.html) explains limitations of replay fills, market impact and queue assumptions. This stress harness does not implement that venue-calibrated model.

## CUDA waiting and coordinates

The Rust bridge captures the helper path once, distinguishes spawn/wait/exit/timeout failures, and polls from 1 ms up to 4 ms rather than a fixed 10 ms. Set `HFT_CUDA_TIMEOUT_MS` to 1–30,000 for inference; default is 3,000. Probes still have a three-second limit. A timeout kills/reaps the direct helper and returns an error; no CPU result is silently substituted. This is a soft OS scheduling deadline, not a hard real-time bound. The trusted native helper does not spawn children.

The CUDA source checks device 0's grid and thread limits, queues copies and kernels on one nonblocking stream, and synchronizes that stream before inspecting or publishing output. This surfaces asynchronous errors at completion, as described by [NVIDIA's stream documentation](https://docs.nvidia.com/cuda/cuda-programming-guide/02-basics/asynchronous-execution.html). Host vectors are pageable: async API calls do not imply transfer overlap. Process and file transfer still make this unsuitable for an HFT hot path.

CUDA model loading also runs a one-row all-ones comparison against the loaded CPU weights and rejects a numerical mismatch before making inference available. This is a startup smoke gate, not a substitute for the full GPU parity suite. CPU-host fixtures verify successful output, startup numerical mismatch, exit failure, malformed/nonfinite output, a killed timeout and invalid timeout settings. They do not run CUDA. `make cuda-check` requires nvcc and NVIDIA hardware and remains blocked on this Mac. CUDA performance and numerical correctness remain unverified.
