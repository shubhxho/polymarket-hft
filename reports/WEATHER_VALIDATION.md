# Validation for weather and disk recovery

48 CPU tests passed, one NVIDIA-only test ignored. Format and Clippy passed. The actual workspace passed check, run-through, replay, fair, polygon, bench, compare, report and install. The installed hft-weather reproduced weather-10000.json byte-for-byte. Codegen passed in the publishing checkout and, after additional project-only cleanup, in the actual workspace. Generated LLVM/assembly files were removed afterward to recover disk space. The first workspace codegen attempt ran out of space; the retry passed.

The first combined run stopped at install with 126 MiB free. After deleting project inspection artifacts, install and GIF generation passed. No unrelated user files were deleted. Native MLX preflight requires 1 GiB and remains blocked; CUDA is not compiled or tested. The GIF decodes as seven 640x300 frames, 14 seconds. Current compact timing results are diagnostic and stored separately from historical release results.

Evidence: [workspace commands](workspace-disk-recovery.txt), [first run](disk-recovery-run.txt), [install and GIF](disk-recovery-install.txt), [native gate](weather-mlx-check.txt).

Workspace codegen: [first attempt](workspace-codegen.txt), [successful retry](workspace-codegen-retry.txt).
