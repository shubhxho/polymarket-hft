#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
binary=$(mktemp "${TMPDIR:-/tmp}/hft-cuda-host.XXXXXX")
trap 'rm -f "$binary"' EXIT HUP INT TERM
"${CXX:-c++}" -std=c++17 -O1 -Wall -Wextra -Werror ${HFT_HOST_SANITIZERS:+-fsanitize=address,undefined} cuda/protocol_test.cpp -o "$binary"
"$binary"
