#!/usr/bin/env bash
# Capture the app window to a PNG (development; see crates/teslacam-app/src/dev.rs).
#
#   scripts/capture.sh <out.png> [VAR=value ...]
#
# Runs the debug build with TESLACAM_CAPTURE=<out.png> plus the given TESLACAM_* variables. The app
# captures its own window with `screencapture -l` after TESLACAM_CAPTURE_DELAY_MS and quits; this
# script kills it if it is still running after 60 s, so nothing is left behind.
#
# Example:
#   scripts/capture.sh /tmp/library.png TESLACAM_OPEN="$PWD/.fixtures/TeslaCam" TESLACAM_WINDOW=1100x700
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT=$1
shift
BIN="$ROOT/target/debug/teslacam-viewer"
[[ -x "$BIN" ]] || (cd "$ROOT" && cargo build -p teslacam-app)

env TESLACAM_CAPTURE="$OUT" "$@" "$BIN" &
PID=$!
for _ in $(seq 1 600); do
  if ! kill -0 "$PID" 2>/dev/null; then
    wait "$PID" || true
    [[ -s "$OUT" ]] && echo "$OUT" && exit 0
    echo "capture failed" >&2 && exit 1
  fi
  sleep 0.1
done
kill "$PID" 2>/dev/null || true
echo "timed out" >&2
exit 1
