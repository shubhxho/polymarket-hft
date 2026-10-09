# Local Rust research desk

Run `make desk`, then open **http://127.0.0.1:8787/**. After `make install`, run `.local/bin/hft-desk` directly without a rebuild. The single Rust binary embeds its HTML, CSS, JavaScript, local Inter font and evidence snapshots. No frontend build, hosted inference service or cloud job is required.

The four views are:

- **Simulation:** start, pause, resume and reset accelerated playback of daily Rust weather settlements. Choose all six scenarios and a seed. A shared $10,000 bankroll, equity curve and bounded event tape show the selected path. This is replay of precomputed synthetic simulations, not intraday live matching. Regenerate the source report with `make weather-simulations`, then rebuild the desk to embed it.
- **Supplied live data:** import JSON/JSONL or post current candidates from a producer. The Rust engine ranks conservative fee-inclusive edge per share, applies caller-selected fractional Kelly and caps (quarter-Kelly in the example), and rejects stale/future timestamps using the server wall clock. “Best” means the strongest eligible supplied candidate; it does not mean best across Polymarket. The analyzer does not enforce aggregate portfolio allocation or cross-market correlation.
- **Evidence:** read build profile, compiler, environment, artifact hashes, sample counts, timer-floor values and every weather scenario. Current compact timings are diagnostic; the historical release comparisons are separate public artifacts.
- **Local inference:** load an HQT1 checkpoint once and run bounded batches using the retained Rust CPU/MLX/CUDA model. Outputs are not automatically treated as calibrated probabilities.

## Incoming candidates

The local service binds only to 127.0.0.1. Example with data you have prepared:

```sh
curl http://127.0.0.1:8787/api/signals \
  -H 'Content-Type: application/json' -H 'X-HFT-Desk: 1' \
  --data-binary @current-candidates.json
```

Supply an array of 1–64 `SignalRequest` objects, maximum 64 KiB. The UI accepts arrays or JSONL and provides a clearly synthetic schema example. Required fields are the existing signal schema plus `fee_rate_micro`; set `fee_bps` to zero. Prices/probabilities/rates use millionths, money uses micro-dollars, and quote/forecast times use UNIX milliseconds. The service overwrites `generated_ms` with its own clock. Do not refresh timestamps unless the corresponding quote/forecast was actually refreshed. Duplicate token candidates reject.

The price-dependent fee is `shares × rate × p × (1-p)`, using the supplied actual market rate and a conservative cash bound. [Polymarket documents the fee curve and per-market fee parameters](https://docs.polymarket.com/trading/fees). Cash-bound sizing does not implement actual venue net-share settlement. The CLI's legacy flat-bps fixture mode remains explicitly labeled as an assumption.

Enable **Follow incoming local feed** to refresh rankings every two seconds. `/api/latest` recalculates age from the retained candidate data; it cannot keep stale candidates valid. This service does not automatically discover or subscribe to Polymarket markets. Feed provenance and probability calibration remain your responsibility. It has no wallet, signing, money movement or order-submission route.

## Local model

```sh
.local/bin/hft-pack fixtures/weights.json .local/demo.hqt 4 64
.local/bin/hft-desk 8787 .local/demo.hqt cpu
# With an appropriately compiled native backend:
HFT_PROFILE=compact cargo run --profile compact --locked --offline \
  --features mlx-cpu --bin hft-desk -- 8787 .local/demo.hqt mlx-cpu
```

CPU is explicit in the example. `auto` chooses an executable detected backend. `mlx-metal` requires the MLX feature, Apple hardware and Metal tools; `cuda` requires a working configured helper. The existing MLX path imports affine packed codes/parameters directly, retains device arrays and a stream, explicitly evaluates lazy results, and uses fallible readbacks. This follows [MLX's lazy evaluation model](https://ml-explore.github.io/mlx/build/html/usage/lazy_evaluation.html). Native MLX and CUDA remain unvalidated for this desk update. Default operation sends no hosted inference requests; local compute still uses your machine's resources.

## Command-line transport and API

`hft-desk --stdio` accepts bounded JSONL envelopes such as:

```json
{"method":"GET","path":"/api/meta"}
{"method":"POST","path":"/api/replay","body":{"scenario":"normal","seed":0,"day":0}}
```

Responses have `ok` and `data` or `error`. It uses the same API dispatch as HTTP, without opening a listening socket. Optional model arguments are supported after `--stdio`.

HTTP routes: GET `/api/meta`, `/api/weather`, `/api/latest`; POST `/api/replay`, `/api/signals`, `/api/infer`. POST requires JSON content type and `X-HFT-Desk: 1`. HTTP/1.1 framing is bounded, duplicate headers/chunked input reject, Host and Origin are checked, socket reads/writes time out, and requests are limited to 32 per second. This is a single-owner local research process, not a public internet server or trading transport. Inference is serial and can delay other requests. Large-model performance and high-frequency throughput are unmeasured.

## Credibility and validation

No TY/YC, investor, partner or customer affiliation is claimed without verification. The desk ties claims to artifacts rather than endorsements. No external HFT-engine win, production readiness, model calibration or profitability has been established.

53 CPU tests pass, with one NVIDIA parity test ignored. They include actual subprocess API dispatch, repeated inference with retained CPU weights, fee-curve arithmetic, ranking, stale data and HTTP framing. JavaScript syntax passes. Browser interaction/layout testing is **unverified**: no browser was connected and this sandbox refused local TCP binding. Normal user-shell startup still requires verification on the target machine. Native MLX remains blocked by the storage preflight; CUDA remains uncompiled/unmeasured here.

Generate the [terminal GIF](../reports/desk-release.gif) with `make desk-gif`. Rust renders real daily settlement values, ffmpeg encodes 21 frames at 640×300 over 42 seconds, and temporary frames are deleted afterward. This is a terminal-style rendering, not a browser capture. [CPU validation](../reports/desk-checks.txt), [GIF validation](../reports/desk-gif-check.txt), and [weather methods](WEATHER_PORTFOLIOS.md) supply the evidence.
