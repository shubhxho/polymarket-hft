HFT_PROFILE ?= compact
export CARGO_BUILD_JOBS ?= 1

.PHONY: check bench compare package signals evaluate replay fair polygon mlx-check mlx-bench mlx-cpu-check mlx-cpu-bench reverse durable news codegen install run-through report

check:
	cargo fmt --all -- --check
	cargo test --profile $(HFT_PROFILE) --locked --offline
	cargo clippy --profile $(HFT_PROFILE) --locked --offline --all-targets -- -D warnings

bench:
	cargo run --profile $(HFT_PROFILE) --locked --offline --bin hft-bench > reports/benchmark-current-$(HFT_PROFILE).json.tmp
	mv reports/benchmark-current-$(HFT_PROFILE).json.tmp reports/benchmark-current-$(HFT_PROFILE).json

compare:
	mkdir -p reports/current-$(HFT_PROFILE)
	HFT_REPORT_DIR=reports/current-$(HFT_PROFILE) cargo run --profile $(HFT_PROFILE) --locked --offline --bin hft-compare

package:
	sh scripts/package-benchmarks.sh

signals:
	cargo run --profile $(HFT_PROFILE) --locked --offline --bin hft-signals -- fixtures/signals.jsonl > reports/signals-smoke.jsonl.tmp
	mv reports/signals-smoke.jsonl.tmp reports/signals-smoke.jsonl

evaluate:
	cargo run --profile $(HFT_PROFILE) --locked --offline --bin hft-evaluate -- fixtures/paired-forecasts.json > reports/paired-evaluation-synthetic.json.tmp
	mv reports/paired-evaluation-synthetic.json.tmp reports/paired-evaluation-synthetic.json

report:
	cargo run --profile $(HFT_PROFILE) --locked --offline --bin hft-report

run-through:
	HFT_PROFILE=$(HFT_PROFILE) sh scripts/run-through.sh

replay:
	cargo run --profile $(HFT_PROFILE) --locked --offline --bin hft -- replay fixtures/smoke.jsonl

fair:
	cargo run --profile $(HFT_PROFILE) --locked --offline --bin hft -- fair fixtures/forecast.json

polygon:
	cargo run --profile $(HFT_PROFILE) --locked --offline --bin hft -- polygon

mlx-check:
	xcrun -f metal
	sh scripts/provision-mlx.sh
	CMAKE_TOOLCHAIN_FILE=$(CURDIR)/.cache/mlx/toolchain.cmake cargo test --profile $(HFT_PROFILE) --locked --features mlx
	CMAKE_TOOLCHAIN_FILE=$(CURDIR)/.cache/mlx/toolchain.cmake cargo clippy --profile $(HFT_PROFILE) --locked --all-targets --features mlx -- -D warnings

mlx-bench:
	xcrun -f metal
	sh scripts/provision-mlx.sh
	CMAKE_TOOLCHAIN_FILE=$(CURDIR)/.cache/mlx/toolchain.cmake cargo run --profile $(HFT_PROFILE) --locked --features mlx --bin hft-bench > reports/benchmark-current-mlx-$(HFT_PROFILE).json.tmp
	mv reports/benchmark-current-mlx-$(HFT_PROFILE).json.tmp reports/benchmark-current-mlx-$(HFT_PROFILE).json

mlx-cpu-check:
	sh scripts/provision-mlx.sh
	CMAKE_TOOLCHAIN_FILE=$(CURDIR)/.cache/mlx/toolchain.cmake cargo test --profile $(HFT_PROFILE) --locked --offline --features mlx-cpu
	CMAKE_TOOLCHAIN_FILE=$(CURDIR)/.cache/mlx/toolchain.cmake cargo clippy --profile $(HFT_PROFILE) --locked --offline --all-targets --features mlx-cpu -- -D warnings

mlx-cpu-bench:
	sh scripts/provision-mlx.sh
	CMAKE_TOOLCHAIN_FILE=$(CURDIR)/.cache/mlx/toolchain.cmake cargo run --profile $(HFT_PROFILE) --locked --offline --features mlx-cpu --bin hft-bench > reports/benchmark-current-mlx-cpu-$(HFT_PROFILE).json.tmp
	mv reports/benchmark-current-mlx-cpu-$(HFT_PROFILE).json.tmp reports/benchmark-current-mlx-cpu-$(HFT_PROFILE).json

reverse:
	cargo run --profile $(HFT_PROFILE) --locked --offline --bin hft -- replay fixtures/reverse.jsonl

durable:
	cargo run --profile $(HFT_PROFILE) --locked --offline --bin hft -- durable-replay fixtures/smoke.jsonl state.journal

news:
	cargo run --profile $(HFT_PROFILE) --locked --offline --bin hft -- news fixtures/news.jsonl news.journal

# Compact is the disk-safe default for inspection/stress. Set HFT_PROFILE=release for optimized measurements.
# Fat LTO and one codegen unit are in the release profile.
# Local CPU tuning: RUSTFLAGS='-C target-cpu=native' make codegen.
codegen:
	cargo rustc -p polymarket-hft --profile $(HFT_PROFILE) --locked --offline --lib -- --emit=llvm-ir,asm

install:
	HFT_PROFILE=$(HFT_PROFILE) sh scripts/install.sh

.PHONY: hardware polygon-plan cuda-build
hardware:
	cargo run --profile $(HFT_PROFILE) --locked --offline --bin hft-hardware

polygon-plan:
	cargo run --profile $(HFT_PROFILE) --locked --offline --bin hft-polymarket -- plan fixtures/polygon-plan.json > reports/polygon-plan.json.tmp
	mv reports/polygon-plan.json.tmp reports/polygon-plan.json

cuda-build:
	sh scripts/build-cuda.sh

.PHONY: mlx-cpu-release-check
mlx-cpu-release-check:
	sh scripts/provision-mlx.sh
	CMAKE_TOOLCHAIN_FILE=$(CURDIR)/.cache/mlx/toolchain.cmake cargo test --profile release --locked --offline --features mlx-cpu
	CMAKE_TOOLCHAIN_FILE=$(CURDIR)/.cache/mlx/toolchain.cmake cargo clippy --profile release --locked --offline --all-targets --features mlx-cpu -- -D warnings

.PHONY: release-check
release-check:
	cargo fmt --all -- --check
	cargo test --profile release --locked --offline
	cargo clippy --profile release --locked --offline --all-targets -- -D warnings

.PHONY: compact-check
compact-check:
	cargo fmt --all -- --check
	cargo test --profile compact --locked --offline
	cargo clippy --profile compact --locked --offline --all-targets -- -D warnings

.PHONY: cuda-check
cuda-check: cuda-build
	HFT_CUDA_HELPER=$(CURDIR)/.local/bin/hft-cuda cargo test --profile compact --locked --offline --test cuda -- --ignored --nocapture

.PHONY: simulations
simulations: compact-check
	mkdir -p reports
	cargo run --profile $(HFT_PROFILE) --locked --offline --bin hft-simulations -- --report > reports/simulations.json.tmp
	mv reports/simulations.json.tmp reports/simulations.json

.PHONY: weather-data weather-simulations
weather-data:
	HFT_PROFILE=$(HFT_PROFILE) sh scripts/weather-simulations.sh generate

weather-simulations: compact-check weather-data
	HFT_PROFILE=$(HFT_PROFILE) sh scripts/weather-simulations.sh simulate

.PHONY: disk-preflight native-preflight
disk-preflight:
	sh scripts/disk-preflight.sh
native-preflight:
	HFT_MIN_FREE_MB=1024 sh scripts/disk-preflight.sh

check compact-check run-through replay fair polygon bench compare report codegen install signals evaluate reverse durable news hardware polygon-plan weather-data: disk-preflight
mlx-check mlx-bench mlx-cpu-check mlx-cpu-bench mlx-cpu-release-check release-check: native-preflight

.PHONY: release-gif
release-gif: disk-preflight
	cargo run --profile $(HFT_PROFILE) --locked --offline --bin hft-release

.PHONY: desk
desk: disk-preflight
	cargo run --profile $(HFT_PROFILE) --locked --offline --bin hft-desk

.PHONY: desk-gif
desk-gif: disk-preflight
	cargo run --profile $(HFT_PROFILE) --locked --offline --bin hft-release -- --desk
