# Polygon Rust migration — version 0.4.0

The active engine, signals, generated constants, fixtures and CLI workflows now use Polygon 137. Non-Polygon inputs reject. This is a breaking change: archive Base journals and retain source version 0.3.0 to inspect them. New code rejects a Base journal without changing its bytes; start a fresh Polygon journal. Synthetic fixture tags do not move real assets or funds.

44 compact-profile tests passed; one NVIDIA GPU parity test was ignored explicitly. Formatting and Clippy passed with warnings denied. The full compiled workflow passed, then all seven installed runtime CLIs were exercised again from the user workspace. See polygon-check.txt, polygon-workflow.txt and polygon-installed-workflow.txt. The installed binaries use the compact CPU profile; build with mlx-cpu or mlx to enable MLX in the current version. Previous native MLX release logs describe version 0.3.0, not a native rerun of this revision.

The CPU loaded-model path validates each batch once, avoiding repeated finite-value scans. Journal recovery reuses its bounded frame allocation. CUDA models retain the encoded HQT1 payload and helper path, avoiding repeated weight serialization and hashing. The CUDA kernel specializes bit widths and reduces with warp shuffles and one block barrier. These changes do not establish a measured speedup. CUDA still launches a process and transfers files/weights per batch; it is not a resident low-latency service.

make cuda-check was attempted, but nvcc is absent and this host has no NVIDIA GPU. CUDA compilation, numerical parity and latency remain unverified. The GPU gate covers q2/q4/q8, arbitrary small group widths, partial packed bytes and batches up to 256 rows. A CPU-host helper fixture passed checks for valid output, wrong byte count, nonfinite output and process failure; that fixture does not execute CUDA.

Fresh synthetic CPU timings: benchmark-polygon-compact.json, 29 operation rows, package version 0.4.0, explicitly labeled build_profile=compact and chain_id=137. These timings include timer overhead and are not comparable to the historical release artifacts. No HRT/FPT superiority, live latency or trading-profit claim is made.

Unsigned venue planning remains available. Signing, authenticated orders, account/fill reconciliation, operating limits and a continuous live feed remain incomplete. No live trading or fund transfers were performed.
