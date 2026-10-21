#!/usr/bin/env bash
# Second locked measurement batch: CA text fix + repaint drive comparison
# with GPUI's inactive-window frame cap disabled.
set -uo pipefail
OUT=$1
SPIKE="$(cd "$(dirname "$0")/.." && pwd)"
REL="$SPIKE/target/release"
cd "$SPIKE"
mkdir -p "$OUT"
echo "== start $(date)"
cargo build --release --bins 2>&1 | grep -E "^error|Finished"
echo "== Q6 export, CATextLayer forced display"
SPIKE_CA_DISPLAY=1 "$REL/export_probe" "$OUT/composite-ca2.mp4" ca 1280x720
ffprobe -v error -count_frames -select_streams v:0 \
  -show_entries stream=codec_name,profile,width,height,r_frame_rate,nb_read_frames,bit_rate:format=duration,size \
  -of compact "$OUT/composite-ca2.mp4"
ffmpeg -hide_banner -loglevel error -y -ss 65 -i "$OUT/composite-ca2.mp4" -frames:v 1 "$OUT/composite-ca2-65s.png"
python3 -I "$SPIKE/scripts/check-export.py" "$OUT/composite-ca2.mp4" 15
echo "== tiles poll, no inactive cap"
"$SPIKE/scripts/measure-tiles.sh" "$OUT/poll" "$REL/tiles" --drive poll --start 50 --play-secs 20 --pause-secs 10 --verify
echo "== tiles raf, no inactive cap"
"$SPIKE/scripts/measure-tiles.sh" "$OUT/raf" "$REL/tiles" --drive raf --start 50 --play-secs 20 --pause-secs 10 --verify
echo "== tiles poll, GPUI default inactive cap (30 fps)"
"$SPIKE/scripts/measure-tiles.sh" "$OUT/poll-capped" "$REL/tiles" --drive poll --start 50 --play-secs 10 --pause-secs 3 --verify --throttle-inactive
echo "== end $(date)"
