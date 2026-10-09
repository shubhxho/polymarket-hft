#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
profile=${HFT_PROFILE:-compact}
case "$profile" in compact|release) ;; *) echo 'Unsupported HFT_PROFILE' >&2; exit 1;; esac
cargo build --profile "$profile" --locked --offline --bin hft-weather
bin_dir=$(cargo metadata --no-deps --format-version 1 --locked --offline | sed -n 's/.*"target_directory":"\([^"]*\)".*/\1/p')
mkdir -p data reports
case "${1:-simulate}" in
  generate)
    output_dir="data/weather-$(date +%Y%m%d-%H%M%S)-$$"
    "$bin_dir/$profile/hft-weather" generate "$output_dir" > reports/weather-data-manifest.json.tmp
    mv reports/weather-data-manifest.json.tmp reports/weather-data-manifest.json
    printf '%s\n' "$output_dir" > data/weather-latest.tmp
    mv data/weather-latest.tmp data/weather-latest
    ;;
  simulate)
    test -f data/weather-latest || { echo 'Run make weather-data first' >&2; exit 1; }
    output_dir=$(cat data/weather-latest)
    case "$output_dir" in data/weather-*) ;; *) echo 'Invalid weather data path' >&2; exit 1;; esac
    "$bin_dir/$profile/hft-weather" simulate "$output_dir" > reports/weather-10000.json.tmp
    mv reports/weather-10000.json.tmp reports/weather-10000.json
    ;;
  *) echo 'usage: weather-simulations.sh generate|simulate' >&2; exit 1;;
esac
