# Rust streaming and accelerator validation

Input JSONL reuses its allocation and retries Interrupted reads. Oversize or other input errors make the reader terminal: later reads reject rather than interpreting a truncated tail as another record.

JsonLines now defaults to a 1 MiB serialized-record limit, including newline. with_limit supports explicit limits from 1 byte to 16 MiB. Serialization completes within a bounded staging writer before any bytes are committed. Oversize/serialization errors leave the output healthy for a subsequent valid record. Underlying write/flush errors poison the writer, discard its unflushed buffer without retry and prevent later record/finish calls. Already written partial bytes cannot be rolled back; terminate that stream. Buffering is not fsync or crash durability.

## Equal-output benchmark

Run make stream-bench. The actual workspace verified identical output and SHA-256 across immediate and batch emission. For 128 records (14,098 bytes), immediate output made **128 write calls and 129 flush calls**; batch output made **one write and one flush call**. This is an in-memory sink, so these are method calls, not measured kernel syscalls or network round trips. It does not establish a speedup for exchange trading.

| Reader capacity (bytes) | p50 batch ns | p95 batch ns |
|---|---:|---:|
| 2 | 158042 | 217542 |
| 4096 | 11750 | 11833 |
| 65536 | 11667 | 11792 |

Each sample reads the same 128-record batch, with five warmup batches and 100 measured samples. Timing includes reader setup and line extraction, excludes JSON parsing/inference and venue I/O. Output equality, raw sorted timings and environment are in [stream-benchmark-compact.json](../reports/stream-benchmark-compact.json). Compact is a correctness profile; these values are diagnostics, not optimized latency or competitor claims.

## Validation and acceleration

The actual workspace passed 56 CPU tests, Clippy, the integrated run-through, stream benchmark and installation. The publishing checkout passed 59 tests with native MLX CPU enabled, plus native Clippy and the native CPU compact benchmark. One NVIDIA parity test is ignored in both test runs; Metal GPU/CUDA remain unvalidated.

Native MLX CPU correctness now has current evidence, replacing the previous disk-blocked gate for that backend. The first fresh build passed the default 1 GiB preflight. A later guarded invocation stopped at 796 MiB; the completed native cache allowed the final warm tests and Clippy to run safely. HFT_NATIVE_MIN_FREE_MB defaults to 1024. For an already-built native cache, HFT_NATIVE_MIN_FREE_MB=256 make mlx-cpu-check mlx-cpu-bench permits a smaller explicit guard. Do not use that as a promise that fresh C++/MLX builds fit.

[Native tests](../reports/streaming-mlx-warm-tests.txt), [native Clippy](../reports/streaming-mlx-warm-clippy.txt), [native benchmark](../reports/benchmark-current-mlx-cpu-compact.json), [actual workspace gates](../reports/streaming-workspace-checks.txt). Compact native numbers remain separate from historical release results. This does not demonstrate GPU throughput, trained-model accuracy, profitable trading or live execution.

## Workspace recovery

The workspace audit found no differences among readable tracked files. 130 source/report files timed out during reads. Their originals were moved intact to .cache/workspace-recovery/20261010-012224 before readable copies were restored from the publishing checkout. The recovery folder is ignored and preserved locally. Changes within unreadable originals could not be inspected; none were silently discarded or published as verified changes.

## Run

Use make check, make run-through, make stream-bench and make install. Installed CLIs can run without Cargo. For native CPU validation use make mlx-cpu-check; for optimized measurements use HFT_PROFILE=release when storage permits. Journal durability, venue normalization and fail-closed live execution remain separate gates.
