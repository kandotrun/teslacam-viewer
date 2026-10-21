#!/usr/bin/env bash
# Run the GPUI tiles spike, sample its CPU with ps, and try a window capture.
# Usage: measure-tiles.sh <out-dir> <tiles binary> [tiles flags...]
set -euo pipefail
OUT=$1; shift
BIN=$1; shift
SPIKE="$(cd "$(dirname "$0")/.." && pwd)"
mkdir -p "$OUT"
LOG="$OUT/tiles.log"
"$BIN" "$@" > "$LOG" 2>&1 &
PID=$!
echo "pid=$PID" > "$OUT/ps.txt"
# Sample CPU% every second for the whole run.
(
  while kill -0 "$PID" 2>/dev/null; do
    printf '%s %s\n' "$(date +%s)" "$(ps -o %cpu=,rss= -p "$PID" 2>/dev/null || true)" >> "$OUT/ps.txt"
    sleep 1
  done
) &
SAMPLER=$!
for _ in $(seq 1 300); do grep -q PLAYING "$LOG" && break; sleep 0.1; done
sleep 4
WID=""
for _ in 1 2 3 4 5; do
  WID=$("$SPIKE/target/release/winid" "$PID" | grep -v '^-1' | head -1 | cut -d' ' -f1 || true)
  [[ -n "$WID" ]] && break
  sleep 0.5
done
echo "window_id=$WID" >> "$OUT/ps.txt"
if [[ -n "$WID" ]]; then
  if screencapture -x -o -l "$WID" "$OUT/window.png" 2>"$OUT/screencapture.err"; then
    echo "screencapture=ok" >> "$OUT/ps.txt"
  else
    echo "screencapture=failed" >> "$OUT/ps.txt"
  fi
fi
wait "$PID" || true
wait "$SAMPLER" || true
cat "$LOG"
cat "$OUT/ps.txt"
