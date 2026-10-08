#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
cargo build --release --locked --offline --bin hft
work_dir=$(mktemp -d "${TMPDIR:-/tmp}/hft-run-through.XXXXXX")
cli=target/release/hft
{
  "$cli" base
  "$cli" fair fixtures/forecast.json
  "$cli" replay fixtures/smoke.jsonl
  "$cli" replay fixtures/weather.jsonl
  "$cli" replay fixtures/reverse.jsonl
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
  echo "PASS: replay, weather, Base, reversal, durable restart, news dedup, live refusal"
} > reports/run-through.txt
cat reports/run-through.txt
echo "Temporary evidence retained at $work_dir"
