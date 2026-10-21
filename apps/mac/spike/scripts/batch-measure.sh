#!/usr/bin/env bash
# One locked measurement batch (release binaries must already be built).
# Usage: batch-measure.sh <out-dir>
set -uo pipefail
OUT=$1
SPIKE="$(cd "$(dirname "$0")/.." && pwd)"
REL="$SPIKE/target/release"
mkdir -p "$OUT"
echo "== start $(date)"

echo "== Q6 export (AVAssetExportSession + CoreAnimation tool)"
"$REL/export_probe" "$OUT/composite-ca.mp4" ca 1280x720
ffprobe -v error -count_frames -select_streams v:0 \
  -show_entries stream=codec_name,profile,width,height,r_frame_rate,avg_frame_rate,nb_read_frames,bit_rate:format=duration,size \
  -of compact "$OUT/composite-ca.mp4"
ffmpeg -hide_banner -loglevel error -y -ss 65 -i "$OUT/composite-ca.mp4" -frames:v 1 "$OUT/composite-ca-65s.png"
python3 -I "$SPIKE/scripts/check-export.py" "$OUT/composite-ca.mp4" 15

echo "== Q7/Q8 tiles poll-driven"
"$SPIKE/scripts/measure-tiles.sh" "$OUT/tiles-poll" "$REL/tiles" --drive poll --start 50 --play-secs 20 --pause-secs 10
echo "== Q7 tiles request_animation_frame-driven"
"$SPIKE/scripts/measure-tiles.sh" "$OUT/tiles-raf" "$REL/tiles" --drive raf --start 50 --play-secs 20 --pause-secs 10
echo "== Q3/Q4 tiles render-time sync verification (4x)"
"$SPIKE/scripts/measure-tiles.sh" "$OUT/tiles-verify" "$REL/tiles" --drive poll --start 10 --rate 4 --play-secs 10 --pause-secs 2 --verify

echo "== Q4 seeks with frame-ready wait"
SPIKE_WAIT_FRAMES=1 "$REL/sync_probe" seeks
echo "== end $(date)"
