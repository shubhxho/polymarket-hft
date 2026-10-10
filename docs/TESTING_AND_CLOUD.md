# Test gates, CUDA harness, Blacksmith and GCS

This is a Polygon-only paper research engine. A wallet balance is not proof of
ownership, available trading collateral or profitability. No integration reads
private keys, signs transactions, approves tokens or submits orders.

## Run the full default gate

```sh
make check
make test-inventory
make run-through
HFT_HOST_SANITIZERS=1 make cuda-host-check
```

`make check` runs Cargo formatting, every default test, Clippy with warnings as
errors, the shared native CUDA protocol tests, and artifact staging tests.
`make test-inventory` regenerates `reports/test-inventory.txt` from Cargo rather
than maintaining a test list that silently drifts. Default tests and GPU tests
are deliberately separate: a skipped NVIDIA test is not GPU validation.

| Suite | Contract covered |
| --- | --- |
| `core` | Book atomicity and sequences, ticks, fees, inventory/cash/city caps, clocks, risk and randomized reference parity |
| `cuda` | Helper process timeout/exit errors, oversized/truncated/nonfinite outputs, startup parity, canonical HCI1 request framing and pre-launch batch bounds |
| `desk`, `desk_service` | HTTP boundary validation, live signal ranking/time/fee checks, retained model/feed service dispatch |
| `math_signals` | Exact binary/edge/Kelly budgets, normal/weather reference math, signal uncertainty, provenance and evaluation leakage |
| `pipeline` | Journal recovery, locks, configuration drift, deduplication, news halt and loss controls |
| `simulations`, `weather` | Bounded deterministic stress, losses and shared $10,000 synthetic portfolio settlement without future observations |
| `streaming`, `transport` | Bounded records, interrupted input, terminal failures, no destructor retry, exact decimals, binary encoding, quantization and venue planning |
| `wallet` | Address and URL policy, exact JSON-RPC identity, Polygon chain checks, uint256 words, fixed block reads, transport failure and chain drift |
| Native `cuda/protocol_test.cpp` | Shared CUDA input parser: all truncation points, magic/reserved bytes, shape/batch bounds, finite quantization, overflow and packed padding |
| `scripts/test_ci_artifacts.py` | Fresh evidence allowlist, hash/size integrity, provenance, refusal of symlinks, empty/malformed evidence and destination overwrites |

Native MLX adds feature-specific parity tests. Run `make mlx-cpu-check` with
sufficient free disk and provisioned dependencies. Warm native builds can be
validated directly using the existing CMake toolchain; do not lower the fresh
build disk threshold just to force a large native rebuild onto a full disk.

## CUDA device gate

```sh
make cuda-check
```

This builds with nvcc, probes the device and explicitly runs ignored NVIDIA
parity tests. Compare packed CPU and CUDA results across 2/4/8 bits, group sizes,
widths including a partial warp and 8,192 weights, and batches of 1/3/32/256 rows.
The bridge performs startup parity before exposing the selected backend. It
still launches a subprocess and transfers weights for each request: this is a
correctness harness, not evidence of production HFT latency or retained GPU
weights.

`cuda/protocol.hpp` is used by the actual executable and CPU-only tests, so the
host test is not a parallel mock parser. It checks structure and numeric bounds;
HQT1 SHA-256 corruption verification is performed by Rust `Quantized::decode`
before canonical frames are encoded. The native executable is an internal
helper, not a separate untrusted-model ingestion API. A checksum is not model
authentication. Native device execution remains unvalidated until an actual
NVIDIA run passes.

## Blacksmith pipelines

All jobs use Blacksmith runner labels. Source is fetched at the exact Actions
commit through GitHub's tarball API. No job invokes a checkout or a local Git
command. Dependencies come from the locked Cargo registry manifest.

- `blacksmith.yml`: Linux compact and release tests, doc tests, Clippy, sanitized
  native host parser and artifact tests. Passing compact jobs run the connected
  paper workflow and generate fresh kernel/stream benchmark evidence.
- Manual `mlx_cpu` input: native MLX CPU tests on Blacksmith Apple Silicon.
- `cuda.yml`: host parser tests plus a digest-pinned NVIDIA development container
  that compiles CUDA kernels for `sm_80`. Compile success is not device parity.
- Manual `publish_gcs` input on `main`: only after both CPU profiles pass, upload
  the compact artifact bundle using GCP Workload Identity Federation.

Install the Blacksmith GitHub App with access to this repository before enabling
these workflows. Runner registration and remote runs are external setup gates.
The published Blacksmith runner list does not specify a GPU runner; no GPU label
is invented here. Run `make cuda-check` on an approved NVIDIA host, or obtain a
confirmed GPU service from Blacksmith before adding a device job. Optional MLX
and cloud publication are manual to keep resource use explicit.

Action versions are pinned by commit. Validate the workflow syntax without Git:

```sh
actionlint -config-file .github/actionlint.yaml -shellcheck= \
  .github/workflows/blacksmith.yml .github/workflows/cuda.yml
```

## GCP / GCS publication setup

Set these repository or `benchmark-publication` environment variables:

| Variable | Value |
| --- | --- |
| `GCP_WORKLOAD_IDENTITY_PROVIDER` | `projects/PROJECT_NUMBER/locations/global/workloadIdentityPools/POOL/providers/PROVIDER` |
| `GCP_SERVICE_ACCOUNT` | Publication-only service account email |
| `GCS_BUCKET` | Existing bucket name, without `gs://` |

Configure GCP's OIDC provider for GitHub. Bind trust to this repository's immutable
numeric repository ID and `refs/heads/main`; restrict the GitHub environment to
that branch. Grant the federated principal `roles/iam.workloadIdentityUser` on
the publication service account, and grant that account
`roles/storage.objectCreator` only on the evidence bucket. Existing run evidence
uses unique commit/run/attempt paths, so publication needs no overwrite/delete
permission. Do not grant project Owner or export service account keys.

The job gets `id-token: write` only for publication. It stages five fresh files:
checks, run-through, kernel benchmark, stream benchmark and test inventory. `manifest.json`
records commit/run identity, compact profile, byte counts, SHA-256 digests and
`gpu_validated: false`. Credentials, wallets, source directories and historical
reports are excluded. The upload path is:

```text
gs://BUCKET/hft/COMMIT/RUN_ID-RUN_ATTEMPT/
```

No GCP resources have been provisioned and no upload is claimed merely because
workflow YAML validates. Provider, service account, bucket permissions and an
actual successful workflow run must establish cloud readiness.

## Read-only crypto wallet inspection

```sh
HFT_POLYGON_RPC_URL=https://YOUR-RPC \
  cargo run --profile compact --locked --offline --bin hft-wallet -- \
  0xYOUR_40_HEX_DIGIT_ADDRESS [0xTOKEN_CONTRACT]
```

HTTPS, no redirects, bounded responses and network timeouts are enforced. The
adapter checks chain ID 137 before reading, pins native and optional ERC-20
`balanceOf` reads to one block number, then checks the chain again. Balances stay
in exact hexadecimal wei/raw token units without floating point or assumed
decimals. Address normalization does not verify EIP-55 or ownership. The RPC
provider is trusted for correctness; a block number is not a finality guarantee
and a reorganization can change its contents. Supplying a token contract does
not certify that token as Polymarket collateral. RPC URLs may contain provider
credentials; errors do not echo them.

## References

- [Blacksmith runner types](https://docs.blacksmith.sh/blacksmith-runners/overview)
- [GitHub source archive API](https://docs.github.com/en/rest/repos/contents#download-a-repository-archive-tar)
- [GCP identity federation for deployment pipelines](https://docs.cloud.google.com/iam/docs/workload-identity-federation-with-deployment-pipelines)
- [Google authentication action](https://github.com/google-github-actions/auth)
- [Google GCS upload action](https://github.com/google-github-actions/upload-cloud-storage)
