# Operations

## Build and run

Rust 1.97.1 is the validated toolchain. A clean machine needs network access to populate Cargo dependencies; subsequent checks are locked and offline.

```sh
cargo fetch --locked
make check
make run-through
make bench
make codegen
make install
.local/bin/hft polygon
```

run-through exercises the CLI with temporary journals and writes reports/run-through.txt. It checks restart idempotence, news deduplication and the live settlement refusal. Existing journals are preserved.

| Command | Behavior |
|---|---|
| hft polygon | Prints Polygon chain 137 and execution status |
| hft fair fixtures/forecast.json | Untrained weather probabilities |
| hft replay fixtures/smoke.jsonl | Fresh in-memory paper state |
| hft durable-replay fixtures/smoke.jsonl state.journal | Write-ahead replay with prefix validation |
| hft journal state.journal | Recover and display state |
| hft replay fixtures/reverse.jsonl | Target buy then sell at an intentional loss |
| hft news fixtures/news.jsonl news.journal | Recorded fixture and missing-model review |
| hft news input.jsonl news.journal --jev | Real Jev request; requires TYPESAFE_API_KEY |
| hft live | Fails until authenticated execution and reconciliation exist |

replay and durable-replay accept an optional final config.json. Configuration and events reject unknown fields. An existing journal retains its configuration; changes reject. Event tags are the public Event enum in rust/src/engine.rs. News input wraps item and response as shown in fixtures/news.jsonl. In --jev mode a supplied recorded response is prohibited. TYPESAFE_DEFAULT_MODEL defaults to jev-latest. Live Jev has not been validated with credentials.

## Recover safely

Stop the old writer, then resume durable-replay using the same full ordered input plus a new suffix. Changed or shortened prefixes fail. News dedup survives restart. Preserve a copy before investigating corruption. An interrupted newline-less tail is truncated on open; complete corruption fails. Disk-full/sync errors require restart and inspection. There is no cross-host lease, compaction or exchange-fill reconciliation.

## MLX

```sh
make mlx-cpu-check
make mlx-cpu-bench
# Requires full Xcode with the Metal compiler:
make mlx-check
make mlx-bench
```

Provisioning needs curl, tar, shasum, CMake and a C++ compiler. It fetches SHA-256-pinned MLX, JSON, fmt and GGUF sources into .cache/mlx and directs CMake to those sources. CPU uses Accelerate; mlx selects Metal. The vendored Rust binding has one documented feature-propagation fix in vendor/README.md. Results evaluate lazy operations and include input/readback. Quantization groups are 64.

| Failure | Action |
|---|---|
| journal_already_locked_or_lock_unavailable | Stop the other writer; verify filesystem locking |
| journal_config_mismatch | Use stored config or a new journal |
| replay_prefix_mismatch_at | Restore original input and investigate divergence |
| journal_integrity_failure_at | Preserve the file and inspect verified frames |
| duplicate_news | Expected for repeated accepted input |
| stale_future_or_reversed_news_time | Correct timestamps without disguising stale reports |
| TYPESAFE_API_KEY_missing | Configure a key or use recorded-response mode |
| No space left on device | Free space; cargo clean removes rebuildable project artifacts |
| xcrun cannot find metal | Install/select full Xcode and Metal tools |

Paper fees approximate flat notional bps. Prices are best within supplied depth and risk limits; no live best-rate guarantee is established.
