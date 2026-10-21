#!/usr/bin/env bash
# Generate TeslaCam-like fixtures for the Mac app tests (idempotent; existing files are kept).
#
# Layout (mirrors a real TeslaCam USB drive), written to apps/mac/.fixtures/TeslaCam/SavedClips/:
#
#   2026-10-20_12-01-30/   "full": 2 segments x 6 cameras x 60 s
#     2026-10-20_12-00-00-<camera>.mp4   (offset 0 s)
#     2026-10-20_12-01-00-<camera>.mp4   (offset 60 s)
#     event.json
#
#   2026-10-20_13-00-50/   "gaps": 3 segments x 20 s at offsets 0, 20, 40 s
#     right_repeater       no file in any segment (missing camera)
#     back                 no file in segment 13-00-20 (missing for 20..40 s)
#     front                segment 13-00-20 lasts only 8 s (gap 28..40 s)
#     event.mp4            20 s, 1280x720, camera id 7 (representative video)
#     event.json
#
# Every frame carries a machine-readable counter in its top-left corner:
#   row y=0..15  : 16 blocks of 16x16, bit k (LSB first) of the EVENT-wide frame index
#                  (round(offset_seconds * 36) + n) is white when set.
#   x=256..271   : parity block (white when popcount(index) is odd).
#   row y=16..31 : 3 blocks encoding (camera index + 1), LSB first (7 = event.mp4).
# Blocks sit on the 16x16 macroblock grid so H.264 keeps them crisp. Clips are 36 fps, 1280x960.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SAVED="$ROOT/.fixtures/TeslaCam/SavedClips"
FPS=36
CAMERAS=(front back left_pillar left_repeater right_pillar right_repeater)
FONT=/System/Library/Fonts/Supplemental/Arial.ttf

filter_for() {
  local id=$1 offset_frames=$2 caption=$3 hue=$4
  local f="hue=h=${hue}"
  f+=",drawtext=fontfile=${FONT}:text='${caption}':fontsize=110:fontcolor=white:borderw=6:x=(w-tw)/2:y=(h-th)/2"
  f+=",drawtext=fontfile=${FONT}:text='frame %{eif\\:n+${offset_frames}\\:d}':fontsize=48:fontcolor=yellow:borderw=4:x=(w-tw)/2:y=h/2+90"
  f+=",drawbox=x=0:y=0:w=272:h=32:color=black:t=fill"
  local k
  for k in $(seq 0 15); do
    f+=",drawbox=x=$((k * 16)):y=0:w=16:h=16:color=white:t=fill:enable='mod(floor((n+${offset_frames})/$((1 << k))),2)'"
  done
  # Parity of the 16 counter bits.
  local parity="mod("
  for k in $(seq 0 15); do
    parity+="mod(floor((n+${offset_frames})/$((1 << k))),2)+"
  done
  parity+="0,2)"
  f+=",drawbox=x=256:y=0:w=16:h=16:color=white:t=fill:enable='${parity}'"
  local b
  for b in 0 1 2; do
    if (( (id >> b) & 1 )); then
      f+=",drawbox=x=$((b * 16)):y=16:w=16:h=16:color=white:t=fill"
    fi
  done
  f+=",format=yuv420p"
  printf '%s' "$f"
}

# encode <file> <camera id 1..7> <offset seconds> <duration seconds> <size> <caption> <hue>
encode() {
  local file=$1 id=$2 offset=$3 duration=$4 size=$5 caption=$6 hue=$7
  if [[ -s "$file" ]]; then return 0; fi
  ffmpeg -hide_banner -loglevel error -y \
    -f lavfi -i "testsrc2=size=${size}:rate=${FPS}:duration=${duration}" \
    -vf "$(filter_for "$id" "$((offset * FPS))" "$caption" "$hue")" \
    -c:v libx264 -preset veryfast -crf 26 -profile:v high -bf 0 -g ${FPS} \
    -movflags +faststart -an "$file.tmp.mp4"
  mv "$file.tmp.mp4" "$file"
  echo "wrote $file"
}

# camera <event dir> <segment timestamp> <camera index> <offset s> <duration s>
camera() {
  local dir=$1 segment=$2 index=$3 offset=$4 duration=$5
  local name=${CAMERAS[$index]}
  encode "$dir/${segment}-${name}.mp4" "$((index + 1))" "$offset" "$duration" 1280x960 "$name" "$((index * 60))"
}

if [[ "${1:-}" == "--sample" ]]; then
  # Quick 2 s preview of camera 2 at offset 60 s for eyeballing the filter.
  ffmpeg -hide_banner -loglevel error -y \
    -f lavfi -i "testsrc2=size=1280x960:rate=${FPS}:duration=2" \
    -vf "$(filter_for 3 $((60 * FPS)) left_pillar 120)" -c:v libx264 -preset veryfast -crf 26 -bf 0 -g ${FPS} "$2"
  exit 0
fi

pids=()
run() { "$@" & pids+=($!); }

FULL="$SAVED/2026-10-20_12-01-30"
mkdir -p "$FULL"
for c in 0 1 2 3 4 5; do
  run camera "$FULL" 2026-10-20_12-00-00 "$c" 0 60
  run camera "$FULL" 2026-10-20_12-01-00 "$c" 60 60
done

GAPS="$SAVED/2026-10-20_13-00-50"
mkdir -p "$GAPS"
for c in 0 1 2 3 4; do # no right_repeater (index 5) anywhere
  run camera "$GAPS" 2026-10-20_13-00-00 "$c" 0 20
  if (( c == 0 )); then
    run camera "$GAPS" 2026-10-20_13-00-20 "$c" 20 8 # short front clip
  elif (( c != 1 )); then                            # no back clip in this segment
    run camera "$GAPS" 2026-10-20_13-00-20 "$c" 20 20
  fi
  run camera "$GAPS" 2026-10-20_13-00-40 "$c" 40 20
done
run encode "$GAPS/event.mp4" 7 0 20 1280x720 event 300

for p in "${pids[@]}"; do wait "$p"; done

cat > "$FULL/event.json" <<'JSON'
{"timestamp":"2026-10-20T12:01:30","city":"Shibuya","street":"","est_lat":"35.6595","est_lon":"139.7005","reason":"user_interaction_honk","camera":"0"}
JSON
cat > "$GAPS/event.json" <<'JSON'
{"timestamp":"2026-10-20T13:00:50","city":"Shibuya","street":"","est_lat":"35.6595","est_lon":"139.7005","reason":"user_interaction_honk","camera":"0"}
JSON
echo "done: $SAVED"
