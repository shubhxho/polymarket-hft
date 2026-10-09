#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
if [ -n "${HFT_FEATURES:-}" ]; then
  case "$HFT_FEATURES" in mlx|mlx-cpu) sh scripts/provision-mlx.sh; export CMAKE_TOOLCHAIN_FILE="$(pwd)/.cache/mlx/toolchain.cmake";; *) echo "Unsupported run-through feature" >&2; exit 1;; esac
  cargo build --release --locked --offline --features "$HFT_FEATURES" --bin hft --bin hft-signals --bin hft-evaluate --bin hft-polymarket --bin hft-hardware --bin hft-pack --bin hft-infer
else
  cargo build --release --locked --offline --bin hft --bin hft-signals --bin hft-evaluate --bin hft-polymarket --bin hft-hardware --bin hft-pack --bin hft-infer
fi
work_dir=$(mktemp -d "${TMPDIR:-/tmp}/hft-run-through.XXXXXX")
target_dir=$(cargo metadata --locked --offline --no-deps --format-version 1 | sed -n 's/.*"target_directory":"\([^"]*\)".*/\1/p')
: "${target_dir:?cargo target directory unavailable}"
cli="$target_dir/release/hft"
{
  "$cli" base
  "$cli" fair fixtures/forecast.json
  "$cli" replay fixtures/smoke.jsonl
  "$cli" replay fixtures/weather.jsonl
  "$cli" replay fixtures/reverse.jsonl
  "$target_dir/release/hft-signals" fixtures/signals.jsonl > "$work_dir/signals.jsonl"
  cat "$work_dir/signals.jsonl"
  rg -q '"action":"buy_candidate"' "$work_dir/signals.jsonl"
  rg -q '"action":"no_edge"' "$work_dir/signals.jsonl"
  "$target_dir/release/hft-evaluate" fixtures/paired-forecasts.json > "$work_dir/evaluation.json"
  cat "$work_dir/evaluation.json"
  rg -q '"synthetic": true' "$work_dir/evaluation.json"
  rg -q '"model_inference_measured": false' "$work_dir/evaluation.json"
  "$target_dir/release/hft-hardware"
  "$target_dir/release/hft-polymarket" plan fixtures/polygon-plan.json > "$work_dir/polygon-plan.json"
  cat "$work_dir/polygon-plan.json"
  rg -q '"submission_enabled": false' "$work_dir/polygon-plan.json"
  "$target_dir/release/hft-pack" fixtures/weights.json "$work_dir/weights.hqt" 2 64
  "$target_dir/release/hft-infer" "$work_dir/weights.hqt" fixtures/inference-batch.json cpu > "$work_dir/inference.json"
  cat "$work_dir/inference.json"
  rg -q '"backend":"cpu"' "$work_dir/inference.json"
  if "$target_dir/release/hft-polymarket" submit > "$work_dir/polygon-submit.txt" 2>&1; then
    echo "Unconfigured Polygon submission unexpectedly allowed" >&2; exit 1
  fi
  cat "$work_dir/polygon-submit.txt"
  "$cli" durable-replay fixtures/smoke.jsonl "$work_dir/state.journal"
  "$cli" durable-replay fixtures/smoke.jsonl "$work_dir/state.journal" > "$work_dir/restart.jsonl"
  cat "$work_dir/restart.jsonl"
  rg -q '"new_events":0' "$work_dir/restart.jsonl"
  "$cli" journal "$work_dir/state.journal"
  "$cli" news fixtures/news.jsonl "$work_dir/news.journal"
  "$cli" news fixtures/news.jsonl "$work_dir/news.journal" > "$work_dir/news-restart.jsonl"
  cat "$work_dir/news-restart.jsonl"
  test "$(rg -c '"rejected":' "$work_dir/news-restart.jsonl")" = 2
  rg -q 'duplicate_news' "$work_dir/news-restart.jsonl"
  if "$cli" live > "$work_dir/live.txt" 2>&1; then
    echo "Live policy unexpectedly allowed execution" >&2
    exit 1
  fi
  cat "$work_dir/live.txt"
  echo "PASS: replay, weather, Base, reversal, signals, paired scoring, Polygon planning/refusal, pack/load/infer, hardware, durable restart, news dedup, live refusal"
} > reports/run-through.txt
cat reports/run-through.txt
echo "Temporary evidence retained at $work_dir"
