#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
destination=${1:-/private/tmp/hft-hub-publication}
mkdir -p "$destination/data"
cp publishing/DATASET_CARD.md "$destination/README.md"
cp -R docs reports "$destination/"
cp reports/lookup-samples.csv "$destination/data/"
tar -czf "$destination/source.tar.gz" README.md Cargo.toml Cargo.lock Makefile .gitignore .gitattributes docs rust schema fixtures scripts vendor reports publishing
archive_hash=$(shasum -a 256 "$destination/source.tar.gz" | cut -d ' ' -f 1)
base64 < "$destination/source.tar.gz" > "$destination/source.tar.gz.base64"
printf '# Reproduce version 0.3.0\n\nSource archive SHA-256: %s.\n\n' "$archive_hash" > "$destination/REPRODUCE.md"
cat >> "$destination/REPRODUCE.md" <<'EOF'
Download source.tar.gz.base64. On macOS decode with
`base64 -D -i source.tar.gz.base64 -o source.tar.gz`.
On Linux use `base64 --decode source.tar.gz.base64 > source.tar.gz`.
Verify with `shasum -a 256 source.tar.gz` before extraction.

Extract into an empty directory and run:

```sh
cargo fetch --locked
make check
make run-through
make signals evaluate
make compare bench
make mlx-cpu-check mlx-cpu-bench
make report codegen install
```

Raw CSV is exported by the Rust hft-report binary. Native MLX needs CMake
and a C++ compiler; Metal validation needs full Xcode with the Metal compiler.
All fixtures are synthetic. Read docs/README.md and reports/READINESS.md.
EOF
echo "Packaged benchmark publication at $destination"
