# Buffering, encoding and backends

Rust input paths reuse a bounded line allocation instead of allocating a new vector per record. Replay and durable/news input accept at most 1 MiB per line; signals accept 64 KiB, including newline. Oversized input stops processing. The signal CLI locks stdout once, reuses its serialization buffer and checks write/flush errors. Default output flushes each record for interactive consumers. `--batch-output` flushes on buffer capacity or completion to improve bulk throughput; consumers must allow that delay. Output buffering never replaces journal fsync.

## Local market frames

`encoding::LevelFrame` encodes one update in 40 bytes, little endian: magic HFT1, version, kind, side, flags, session token key, price, quantity, reserved bytes, sequence and timestamp. Reserved bytes must be zero. Quantity zero means deletion. Token keys are session-local integers; callers maintain the mapping to actual asset IDs. This format has no checksum or authentication and is intended for trusted local transport. It is not a Polymarket WebSocket wire format. Book continuity and snapshot recovery remain caller/engine responsibilities.

## Packed model files

`hft-pack weights.json weights.hqt 2 64` produces HQT1 affine weights. The format includes bit width, group size, weight count, packed length, group count, scales/biases, packed codes and SHA-256. Decoder shape checks precede allocation, validate finite positive scales and canonical trailing bits, and limit weights to 16 million. SHA-256 detects accidental corruption; it does not authenticate the model. Packed storage figures exclude the file header and digest, which add 56 bytes.

The CPU dot kernel specializes 2/4/8 bits and hoists group scales/biases out of the inner loop. It preserves accumulation order relative to decoded dense weights. The Rust compiler may vectorize supported operations; this is not a custom compiler or an assembly/SIMD superiority claim.

```sh
make hardware
cargo run --release --locked --offline --bin hft-pack -- fixtures/weights.json /tmp/example-unique.hqt 2 64
cargo run --release --locked --offline --bin hft-infer -- /tmp/example-unique.hqt fixtures/inference-batch.json auto
```

Packing refuses to overwrite a file. Choose a fresh output path. The CLI reports setup-inclusive inference; it is not a latency-sensitive server. In Rust applications, construct `hardware::LoadedModel` once and reuse `infer_batch`. CPU and MLX retain loaded weights; hardware probing stays out of repeated inference. CPU requests skip probing entirely.

## Hardware selection

Detection reports hardware hints separately from usable compiled backends. Auto chooses a successfully probed CUDA helper, then compiled MLX Metal on Apple ARM64, compiled MLX CPU, or packed CPU. Explicit unavailable requests fail. A successful nvidia-smi command alone does not activate CUDA. Apple hardware alone does not activate Metal. Compile `mlx` for Metal or `mlx-cpu` for native CPU. MLX requires group 64 and imports the exact packed uint32 codes, scales and biases from HQT1. It does not requantize the decoded model. Native CPU tests compare that loaded representation against the Rust reference; each deployed model still needs numerical and predictive validation. No loaded GPU fallback is silently substituted after inference fails.

The Metal backend requires a working Metal toolchain/runtime and remains unmeasured here. The observed Apple ARM64 host supports the tested MLX CPU configuration. GPU inference is a cold research path, separate from integer money and execution checks. Selection at startup does not establish the currently loaded weights have predictive quality.

## Experimental CUDA port

`cuda/packed.cu` implements batched affine packed dot products with one CUDA block per row. Build with `make cuda-build`, then set `HFT_CUDA_HELPER` to the absolute `.local/bin/hft-cuda` path. The helper probes cudaGetDeviceCount, creates device buffers, copies packed weights/inputs, launches the kernel and reads results. The Rust side validates weights/input, bounds batches, limits output, and kills a helper exceeding three seconds.

This port is **uncompiled and unmeasured on this host** because nvcc and an NVIDIA GPU are absent. It uses process/file transfer per batch, not a resident CUDA service, so it is unsuitable for an HFT hot path. It does not use fast-math. GPU reduction order differs from CPU and requires toleranced correctness tests on NVIDIA hardware before deployment. MLX/Apple and NVIDIA CUDA are separate backends, following [MLX](https://github.com/ml-explore/mlx) and [CUDA runtime device discovery](https://docs.nvidia.com/cuda/cuda-runtime-api/cuda_runtime_api/group__CUDART__DEVICE.html).

## Disk-constrained development

Use `cargo build --profile compact --locked --offline` or `make compact-check` for correctness work. Compact uses modest optimization, no debug symbols or incremental cache and does not change the release performance profile. Default development builds also disable debug/incremental output and use optimization level 1. These settings reduce artifact growth; they cannot guarantee a build fits an already-full filesystem.

This Mac has a local ignored `.cargo/config.toml` pointing both working copies to `/private/tmp/polymarket-hft-target`, with one build job. It is a machine setting, excluded from the published source. Cargo recreates that directory when needed. `run-through.sh` resolves Cargo's target directory through metadata, so shared caches and `CARGO_TARGET_DIR` overrides work. Other machines can set their own target directory. Avoid simultaneous builds of multiple profiles when free storage is limited. Cleanup removes generated target files, preserving source, fixtures and journals; validated CLIs can run from `.local/bin` without rebuilding.

The MLX toolchain disables C/C++ example executables, tests and framework benchmark targets that are not runtime dependencies. This avoids building large demo binaries in both Cargo panic profiles. `HFT_FEATURES=mlx-cpu make run-through` verifies the native CPU CLI workflow.
