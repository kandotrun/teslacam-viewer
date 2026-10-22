#!/usr/bin/env bash
# Generate the frame-counter clips the web sync e2e test reads back (tests/e2e/sync.spec.ts).
#
# Writes tests/e2e/fixtures/sync/segment-<i>.mp4 for i = 0, 1, 2: one clip per 60 s segment of a
# three-segment event. The picker serves the same clip to all six cameras of a segment, so every
# camera of a frame-accurate player shows the same counter at the same moment.
#
# Each clip lasts 8 s at 36 fps (like HW3 cars), 320x240. Every frame carries the counter layout of
# apps/mac/scripts/gen-fixtures.sh, in the top-left corner:
#   row y=32..47 : 16 blocks of 16x16 from x=32, bit k (LSB first) of the EVENT-wide frame index
#                  (segment * 60 * 36 + n) is white when set.
#   x=288..303   : parity block (white when popcount(index) is odd).
#   y=48..63     : one white block at x=32: present on every decoded frame, absent on black.
# Blocks sit on the 16x16 macroblock grid so H.264 keeps them crisp.
#
# The clips are committed; rerun this (needs ffmpeg, macOS Arial) only to change them.
set -euo pipefail

OUT="$(cd "$(dirname "$0")/.." && pwd)/tests/e2e/fixtures/sync"
FPS=36
SECONDS_PER_CLIP=8
SEGMENT_SECONDS=60
FONT=/System/Library/Fonts/Supplemental/Arial.ttf
OX=32
OY=32

filter_for() {
  local offset_frames=$1
  local f="drawtext=fontfile=${FONT}:text='frame %{eif\\:n+${offset_frames}\\:d}':fontsize=28:fontcolor=yellow:borderw=3:x=(w-tw)/2:y=h-th-24"
  f+=",drawbox=x=$((OX - 16)):y=$((OY - 16)):w=304:h=64:color=black:t=fill"
  local k parity="mod("
  for k in $(seq 0 15); do
    f+=",drawbox=x=$((OX + k * 16)):y=${OY}:w=16:h=16:color=white:t=fill:enable='mod(floor((n+${offset_frames})/$((1 << k))),2)'"
    parity+="mod(floor((n+${offset_frames})/$((1 << k))),2)+"
  done
  parity+="0,2)"
  f+=",drawbox=x=$((OX + 256)):y=${OY}:w=16:h=16:color=white:t=fill:enable='${parity}'"
  f+=",drawbox=x=${OX}:y=$((OY + 16)):w=16:h=16:color=white:t=fill"
  f+=",format=yuv420p"
  printf '%s' "$f"
}

mkdir -p "$OUT"
for segment in 0 1 2; do
  file="$OUT/segment-${segment}.mp4"
  ffmpeg -hide_banner -loglevel error -y \
    -f lavfi -i "color=c=0x2a3340:size=320x240:rate=${FPS}:duration=${SECONDS_PER_CLIP}" \
    -vf "$(filter_for "$((segment * SEGMENT_SECONDS * FPS))")" \
    -c:v libx264 -preset veryfast -crf 30 -profile:v high -bf 0 -g ${FPS} \
    -movflags +faststart -an "$file"
  echo "wrote $file"
done
