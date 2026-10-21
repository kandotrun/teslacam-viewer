#!/usr/bin/env bash
# Generate TeslaCam-like fixtures for the Mac spike.
#
# Layout (mirrors a real TeslaCam USB drive):
#   apps/mac/.fixtures/TeslaCam/SavedClips/2026-10-20_12-01-30/
#     2026-10-20_12-00-00-<camera>.mp4   (segment 0, 60 s)
#     2026-10-20_12-01-00-<camera>.mp4   (segment 1, 60 s)
#     event.json
#
# Every frame carries a machine-readable counter in its top-left corner:
#   row y=0..15  : 16 blocks of 16x16, bit k (LSB first) of the EVENT-wide
#                  frame index (segment * 2160 + n) is white when set.
#   x=256..271   : parity block (white when popcount(index) is odd).
#   row y=16..31 : 3 blocks encoding (camera index + 1), LSB first.
# Blocks sit on the 16x16 macroblock grid so H.264 keeps them crisp.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
OUT="$ROOT/.fixtures/TeslaCam/SavedClips/2026-10-20_12-01-30"
mkdir -p "$OUT"

FPS=36
SECONDS_PER_SEGMENT=60
FRAMES_PER_SEGMENT=$((FPS * SECONDS_PER_SEGMENT))
CAMERAS=(front back left_pillar left_repeater right_pillar right_repeater)
SEGMENTS=(2026-10-20_12-00-00 2026-10-20_12-01-00)
FONT=/System/Library/Fonts/Supplemental/Arial.ttf

filter_for() {
  local cam_index=$1 seg_index=$2 cam_name=$3
  local offset=$((seg_index * FRAMES_PER_SEGMENT))
  local hue=$((cam_index * 60))
  local f="hue=h=${hue}"
  f+=",drawtext=fontfile=${FONT}:text='${cam_name}':fontsize=110:fontcolor=white:borderw=6:x=(w-tw)/2:y=(h-th)/2"
  f+=",drawtext=fontfile=${FONT}:text='seg ${seg_index} frame %{eif\\:n+${offset}\\:d}':fontsize=48:fontcolor=yellow:borderw=4:x=(w-tw)/2:y=h/2+90"
  f+=",drawbox=x=0:y=0:w=272:h=32:color=black:t=fill"
  local k
  for k in $(seq 0 15); do
    f+=",drawbox=x=$((k * 16)):y=0:w=16:h=16:color=white:t=fill:enable='mod(floor((n+${offset})/$((1 << k))),2)'"
  done
  # Parity of the 16 counter bits.
  local parity="mod("
  for k in $(seq 0 15); do
    parity+="mod(floor((n+${offset})/$((1 << k))),2)+"
  done
  parity+="0,2)"
  f+=",drawbox=x=256:y=0:w=16:h=16:color=white:t=fill:enable='${parity}'"
  local id=$((cam_index + 1)) b
  for b in 0 1 2; do
    if (( (id >> b) & 1 )); then
      f+=",drawbox=x=$((b * 16)):y=16:w=16:h=16:color=white:t=fill"
    fi
  done
  f+=",format=yuv420p"
  printf '%s' "$f"
}

encode() {
  local cam_index=$1 seg_index=$2
  local cam=${CAMERAS[$cam_index]} seg=${SEGMENTS[$seg_index]}
  local file="$OUT/${seg}-${cam}.mp4"
  if [[ -s "$file" ]]; then return 0; fi
  ffmpeg -hide_banner -loglevel error -y \
    -f lavfi -i "testsrc2=size=1280x960:rate=${FPS}:duration=${SECONDS_PER_SEGMENT}" \
    -vf "$(filter_for "$cam_index" "$seg_index" "$cam")" \
    -c:v libx264 -preset veryfast -crf 26 -profile:v high -bf 0 -g ${FPS} \
    -movflags +faststart -an "$file.tmp.mp4"
  mv "$file.tmp.mp4" "$file"
  echo "wrote $file"
}

if [[ "${1:-}" == "--sample" ]]; then
  # Quick 2 s preview of camera 2 / segment 1 for eyeballing the filter.
  ffmpeg -hide_banner -loglevel error -y \
    -f lavfi -i "testsrc2=size=1280x960:rate=${FPS}:duration=2" \
    -vf "$(filter_for 2 1 left_pillar)" -c:v libx264 -preset veryfast -crf 26 -bf 0 -g ${FPS} "$2"
  exit 0
fi

pids=()
for s in 0 1; do
  for c in 0 1 2 3 4 5; do
    encode "$c" "$s" &
    pids+=($!)
  done
done
for p in "${pids[@]}"; do wait "$p"; done

cat > "$OUT/event.json" <<'JSON'
{"timestamp":"2026-10-20T12:01:30","city":"Shibuya","street":"","est_lat":"35.6595","est_lon":"139.7005","reason":"user_interaction_honk","camera":"0"}
JSON
echo "done: $OUT"
