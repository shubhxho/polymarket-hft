.PHONY: check bench replay fair base mlx-check mlx-bench codegen install

check:
	cargo fmt --all -- --check
	cargo test --locked --offline
	cargo clippy --locked --offline --all-targets -- -D warnings

bench:
	cargo run --release --locked --offline --bin hft-bench > reports/benchmark-after.json

replay:
	cargo run --release --locked --offline --bin hft -- replay fixtures/smoke.jsonl

fair:
	cargo run --release --locked --offline --bin hft -- fair fixtures/forecast.json

base:
	cargo run --release --locked --offline --bin hft -- base

mlx-check:
	cargo test --locked --features mlx
	cargo clippy --locked --all-targets --features mlx -- -D warnings

mlx-bench:
	cargo run --release --locked --features mlx --bin hft-bench > reports/benchmark-mlx.json

# Inspect LLVM output. Fat LTO and one codegen unit are in Cargo.toml.
# Local CPU tuning: RUSTFLAGS='-C target-cpu=native' make codegen.
codegen:
	cargo rustc -p polymarket-hft --release --locked --offline --lib -- --emit=llvm-ir,asm

install:
	cargo install --path rust --locked --offline --root .local
