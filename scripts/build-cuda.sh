#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
command -v nvcc >/dev/null || { echo 'CUDA unmeasured: nvcc is unavailable' >&2; exit 1; }
mkdir -p .local/bin
nvcc -O3 --fmad=false -arch="${HFT_CUDA_ARCH:-native}" -std=c++17 cuda/packed.cu -o .local/bin/hft-cuda
.local/bin/hft-cuda --probe
