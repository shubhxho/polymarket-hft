#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
mkdir -p reports
inventory=$(mktemp "reports/test-inventory.XXXXXX")
trap 'rm -f "$inventory"' EXIT HUP INT TERM
cargo test --profile "${HFT_PROFILE:-compact}" --locked --offline --all-targets -- --list > "$inventory"
mv "$inventory" reports/test-inventory.txt
