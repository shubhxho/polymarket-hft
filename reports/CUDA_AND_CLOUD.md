# CUDA, wallet and cloud gate results

Validated on 2026-10-11 in `/private/tmp/polymarket-hft-publish`, the existing
readable local mirror. Updated source files were copied from/to the user
workspace. Cloud-backed unchanged workspace files stalled formatting, so this
is mirror validation rather than an assertion that every workspace file can
currently be read. No local Git commands were invoked.

| Gate | Observed result |
| --- | --- |
| Default compact Cargo tests | 63 passed, 0 failed, 1 NVIDIA parity test ignored |
| Native MLX CPU compact Cargo tests | 66 passed, 0 failed, 1 NVIDIA parity test ignored |
| Default and native MLX CPU Clippy | Passed with warnings denied |
| Cargo formatting | Passed |
| Connected paper run-through | Passed |
| Shared native CUDA host parser | Passed normally and with address/undefined-behavior sanitizers |
| CI artifact unit tests | 4 passed |
| Workflow syntax | actionlint v1.7.12 passed with explicit Blacksmith labels |
| Native CUDA compile / NVIDIA parity | Not executed: no nvcc/GPU; Docker socket access denied in this environment |
| Release profile | Configured in CI; not run locally during this change |
| Remote Blacksmith and GCP/GCS execution | GitHub registered both workflows, but automatic and manual runs failed at startup before any jobs; no cloud upload ran |
| Live wallet RPC | Not queried; full read sequence covered with injected transport tests |

Evidence: [default gates](cuda-wallet-checks.txt), [native MLX CPU tests](cuda-wallet-mlx.txt),
[native Clippy](cuda-wallet-mlx-clippy.txt), [generated default test inventory](test-inventory.txt).
The inventory includes the ignored NVIDIA test; listing it does not mean it ran.

There is no new claim of reduced exchange latency, profitability, crypto
exclusivity, company accreditation or live execution readiness. The wallet
adapter reads Polygon balances only. See [test and deployment setup](../docs/TESTING_AND_CLOUD.md).

## Remote workflow observation

Source was published as `481d2b0c9cdd6def98c3112f29a35663d761c97d` using
GitHub's `createCommitOnBranch` API. All 23 published file content hashes were
verified against the local tested files without a Git command.

- [CUDA compile dispatch](https://github.com/shubhxho/polymarket-hft/actions/runs/38082239929): `startup_failure`, no jobs.
- [CPU correctness dispatch](https://github.com/shubhxho/polymarket-hft/actions/runs/38082241982): `startup_failure`, no jobs.
- [Initial push](https://github.com/shubhxho/polymarket-hft/actions/runs/38082099389): `startup_failure`, no jobs.

Repository Actions permissions are enabled and allow all actions. The workflow
files pass actionlint, and GitHub accepted both manual dispatch requests. GitHub
returned no diagnostic check runs/annotations identifying the startup cause;
this evidence does not establish that a particular external configuration is
at fault. Resolve the repository's startup condition before claiming Blacksmith
execution, release-profile results, CUDA compilation or cloud publication.
