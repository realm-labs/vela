#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

if ! command -v sample >/dev/null 2>&1; then
  echo "macOS sample command is required for this helper" >&2
  exit 2
fi

BENCH_ARGS=("$@")
if [[ ${#BENCH_ARGS[@]} -eq 0 ]]; then
  BENCH_ARGS=(--runtime vela --iterations 500000 --repeats 1 --warmup 1 scalar)
fi

cargo bench -p vela_vm --bench external_compare --no-run >/dev/null
BENCH_BIN="$(
  find target/release/deps -maxdepth 1 -type f -perm -111 -name 'external_compare-*' -print0 |
  xargs -0 ls -t |
  head -n 1
)"

STAMP="$(date -u +%Y%m%dT%H%M%SZ)"
OUT_DIR="perf-results/profiles"
mkdir -p "$OUT_DIR"
OUT_FILE="$OUT_DIR/${STAMP}-external_compare.sample.txt"

PROFILE_STDOUT="$(mktemp)"
"$BENCH_BIN" "${BENCH_ARGS[@]}" >"$PROFILE_STDOUT" 2>&1 &
PID=$!
sleep 1
sample "$PID" 10 -file "$OUT_FILE" >/dev/null
wait "$PID" || true
cat "$PROFILE_STDOUT"
rm -f "$PROFILE_STDOUT"
echo "profile=$OUT_FILE"
