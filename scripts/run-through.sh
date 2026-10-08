#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
cargo build --release --locked --offline --bin hft --bin hft-signals --bin hft-evaluate
work_dir=$(mktemp -d "${TMPDIR:-/tmp}/hft-run-through.XXXXXX")
cli=target/release/hft
{
  "$cli" base
  "$cli" fair fixtures/forecast.json
  "$cli" replay fixtures/smoke.jsonl
  "$cli" replay fixtures/weather.jsonl
  "$cli" replay fixtures/reverse.jsonl
  target/release/hft-signals fixtures/signals.jsonl > "$work_dir/signals.jsonl"
  cat "$work_dir/signals.jsonl"
  rg -q '"action":"buy_candidate"' "$work_dir/signals.jsonl"
  rg -q '"action":"no_edge"' "$work_dir/signals.jsonl"
  target/release/hft-evaluate fixtures/paired-forecasts.json > "$work_dir/evaluation.json"
  cat "$work_dir/evaluation.json"
  rg -q '"synthetic": true' "$work_dir/evaluation.json"
  rg -q '"model_inference_measured": false' "$work_dir/evaluation.json"
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
  echo "PASS: replay, weather, Base, reversal, signals, paired scoring, durable restart, news dedup, live refusal"
} > reports/run-through.txt
cat reports/run-through.txt
echo "Temporary evidence retained at $work_dir"
