#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
profile=${HFT_PROFILE:-compact}
case "$profile" in compact|release) ;; *) echo 'Unsupported HFT_PROFILE' >&2; exit 1;; esac
sh scripts/disk-preflight.sh
cargo build --profile "$profile" --locked --offline --bins
target_dir=$(cargo metadata --no-deps --format-version 1 --locked --offline | sed -n 's/.*"target_directory":"\([^"]*\)".*/\1/p')
mkdir -p .local/bin
for binary in "$target_dir/$profile"/hft*; do
  [ -f "$binary" ] && [ -x "$binary" ] || continue
  name=$(basename "$binary")
  cp "$binary" ".local/bin/$name.tmp.$$"
  chmod 755 ".local/bin/$name.tmp.$$"
  mv ".local/bin/$name.tmp.$$" ".local/bin/$name"
done
echo "Installed $profile CPU binaries."
