#!/usr/bin/env bash
# Locked batch: build + run the reader/writer export and the test suites.
# Usage: batch-tests.sh <out-dir>
set -uo pipefail
OUT=$1
SPIKE="$(cd "$(dirname "$0")/.." && pwd)"
cd "$SPIKE"
mkdir -p "$OUT"
echo "== start $(date)"
echo "== build export_rw (release)"
time cargo build --release --bins 2>&1 | grep -E "^(error|warning: unused)|Finished|-->" | head -40
if [[ -x target/release/export_rw ]]; then
  echo "== Q6 alternative: reader/writer + Core Image"
  target/release/export_rw "$OUT/composite-rw.mp4" 8000000
  ffprobe -v error -count_frames -select_streams v:0 \
    -show_entries stream=codec_name,profile,width,height,r_frame_rate,nb_read_frames,bit_rate:format=duration,size \
    -of compact "$OUT/composite-rw.mp4"
  ffmpeg -hide_banner -loglevel error -y -ss 65 -i "$OUT/composite-rw.mp4" -frames:v 1 "$OUT/composite-rw-65s.png"
  python3 -I "$SPIKE/scripts/check-export.py" "$OUT/composite-rw.mp4" 15
fi
echo "== Q9 tests (debug, test-support)"
time cargo test --tests 2>&1 | grep -E "^(error|test |running|test result)|panicked|media_main_thread|-->" | head -60
echo "== end $(date)"
