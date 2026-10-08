.PHONY: check bench compare package signals evaluate replay fair base mlx-check mlx-bench mlx-cpu-check mlx-cpu-bench reverse durable news codegen install run-through report

check:
	cargo fmt --all -- --check
	cargo test --locked --offline
	cargo clippy --locked --offline --all-targets -- -D warnings

bench:
	cargo run --release --locked --offline --bin hft-bench > reports/benchmark-after.json

compare:
	cargo run --release --locked --offline --bin hft-compare

package:
	sh scripts/package-benchmarks.sh

signals:
	cargo run --release --locked --offline --bin hft-signals -- fixtures/signals.jsonl > reports/signals-smoke.jsonl

evaluate:
	cargo run --release --locked --offline --bin hft-evaluate -- fixtures/paired-forecasts.json > reports/paired-evaluation-synthetic.json

report:
	cargo run --release --locked --offline --bin hft-report

run-through:
	sh scripts/run-through.sh

replay:
	cargo run --release --locked --offline --bin hft -- replay fixtures/smoke.jsonl

fair:
	cargo run --release --locked --offline --bin hft -- fair fixtures/forecast.json

base:
	cargo run --release --locked --offline --bin hft -- base

mlx-check:
	xcrun -f metal
	sh scripts/provision-mlx.sh
	CMAKE_TOOLCHAIN_FILE=$(CURDIR)/.cache/mlx/toolchain.cmake cargo test --locked --features mlx
	CMAKE_TOOLCHAIN_FILE=$(CURDIR)/.cache/mlx/toolchain.cmake cargo clippy --locked --all-targets --features mlx -- -D warnings

mlx-bench:
	xcrun -f metal
	sh scripts/provision-mlx.sh
	CMAKE_TOOLCHAIN_FILE=$(CURDIR)/.cache/mlx/toolchain.cmake cargo run --release --locked --features mlx --bin hft-bench > reports/benchmark-mlx.json

mlx-cpu-check:
	sh scripts/provision-mlx.sh
	CMAKE_TOOLCHAIN_FILE=$(CURDIR)/.cache/mlx/toolchain.cmake cargo test --locked --offline --features mlx-cpu
	CMAKE_TOOLCHAIN_FILE=$(CURDIR)/.cache/mlx/toolchain.cmake cargo clippy --locked --offline --all-targets --features mlx-cpu -- -D warnings

mlx-cpu-bench:
	sh scripts/provision-mlx.sh
	CMAKE_TOOLCHAIN_FILE=$(CURDIR)/.cache/mlx/toolchain.cmake cargo run --release --locked --offline --features mlx-cpu --bin hft-bench > reports/benchmark-mlx-cpu.json

reverse:
	cargo run --release --locked --offline --bin hft -- replay fixtures/reverse.jsonl

durable:
	cargo run --release --locked --offline --bin hft -- durable-replay fixtures/smoke.jsonl state.journal

news:
	cargo run --release --locked --offline --bin hft -- news fixtures/news.jsonl news.journal

# Inspect LLVM output. Fat LTO and one codegen unit are in Cargo.toml.
# Local CPU tuning: RUSTFLAGS='-C target-cpu=native' make codegen.
codegen:
	cargo rustc -p polymarket-hft --release --locked --offline --lib -- --emit=llvm-ir,asm

install:
	cargo install --path rust --locked --offline --root .local
