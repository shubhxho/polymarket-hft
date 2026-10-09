#!/bin/sh
set -eu
minimum=${HFT_MIN_FREE_MB:-128}
case "$minimum" in ''|*[!0-9]*) echo 'HFT_MIN_FREE_MB must be a positive integer' >&2; exit 1;; esac
[ "$minimum" -gt 0 ] || exit 1
available=$(df -Pk . | awk 'NR==2 {print int($4/1024)}')
[ -n "$available" ] || { echo 'Unable to inspect free storage' >&2; exit 1; }
if [ "$available" -lt "$minimum" ]; then
  echo "Build stopped: ${available} MiB free; at least ${minimum} MiB required. Installed .local/bin CLIs can run without Cargo. Free storage before retrying." >&2
  exit 1
fi
