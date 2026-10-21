#!/usr/bin/env bash
# Generate TeslaCam-like fixtures for the Mac app tests (idempotent; existing files are kept).
#
# Two parts, both under apps/mac/.fixtures/TeslaCam/:
#
# 1. The media test events (exact frame counters; used by teslacam-media's main-thread tests), in
#    SavedClips/ — described below.
# 2. A realistic library for looking at the app (`--library` is implied; skip it with
#    `--media-only`): RecentClips sessions over four days, SavedClips and SentryClips events with
#    event.json and thumb.png, a Sentry event without its left repeater, a cluster of three events
#    within ten minutes, EncryptedClips entries and one unknown file. Clips are 480x360 at 12 fps so
#    the whole set encodes in about a minute.
#
# Media test layout (mirrors a real TeslaCam USB drive), written to apps/mac/.fixtures/TeslaCam/SavedClips/:
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
# Every frame carries a machine-readable counter near its top-left corner, at (32, 32) so that the
# 12 px rounded corners of the export frame never cover it:
#   row y=32..47 : 16 blocks of 16x16 from x=32, bit k (LSB first) of the EVENT-wide frame index
#                  (round(offset_seconds * 36) + n) is white when set.
#   x=288..303   : parity block (white when popcount(index) is odd).
#   row y=48..63 : 3 blocks encoding (camera index + 1), LSB first (7 = event.mp4).
# Blocks sit on the 16x16 macroblock grid so H.264 keeps them crisp. Clips are 36 fps, 1280x960.
# Files generated before the counter moved are detected and regenerated.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SAVED="$ROOT/.fixtures/TeslaCam/SavedClips"
FPS=36
CAMERAS=(front back left_pillar left_repeater right_pillar right_repeater)
FONT=/System/Library/Fonts/Supplemental/Arial.ttf
OX=32 # counter origin
OY=32
LAYOUT_VERSION=2 # bump when the burned-in layout changes

filter_for() {
  local id=$1 offset_frames=$2 caption=$3 hue=$4
  local f="hue=h=${hue}"
  f+=",drawtext=fontfile=${FONT}:text='${caption}':fontsize=110:fontcolor=white:borderw=6:x=(w-tw)/2:y=(h-th)/2"
  f+=",drawtext=fontfile=${FONT}:text='frame %{eif\\:n+${offset_frames}\\:d}':fontsize=48:fontcolor=yellow:borderw=4:x=(w-tw)/2:y=h/2+90"
  f+=",drawbox=x=$((OX - 16)):y=$((OY - 16)):w=304:h=64:color=black:t=fill"
  local k
  for k in $(seq 0 15); do
    f+=",drawbox=x=$((OX + k * 16)):y=${OY}:w=16:h=16:color=white:t=fill:enable='mod(floor((n+${offset_frames})/$((1 << k))),2)'"
  done
  # Parity of the 16 counter bits.
  local parity="mod("
  for k in $(seq 0 15); do
    parity+="mod(floor((n+${offset_frames})/$((1 << k))),2)+"
  done
  parity+="0,2)"
  f+=",drawbox=x=$((OX + 256)):y=${OY}:w=16:h=16:color=white:t=fill:enable='${parity}'"
  local b
  for b in 0 1 2; do
    if (( (id >> b) & 1 )); then
      f+=",drawbox=x=$((OX + b * 16)):y=$((OY + 16)):w=16:h=16:color=white:t=fill"
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

# Regenerate everything when the fixtures were written with another counter layout.
if [[ "$(cat "$SAVED/.layout" 2>/dev/null)" != "$LAYOUT_VERSION" ]]; then
  rm -rf "$SAVED/2026-10-20_12-01-30" "$SAVED/2026-10-20_13-00-50"
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
echo "$LAYOUT_VERSION" > "$SAVED/.layout"
echo "done: $SAVED"

[[ "${1:-}" == "--media-only" ]] && exit 0

# ---------------------------------------------------------------------------------------------
# 2. The library fixtures
# ---------------------------------------------------------------------------------------------

TESLACAM="$ROOT/.fixtures/TeslaCam"
LIBRARY_VERSION=1 # bump when the library set changes
if [[ "$(cat "$TESLACAM/.library" 2>/dev/null)" != "$LIBRARY_VERSION" ]]; then
  rm -rf "$TESLACAM/RecentClips" "$TESLACAM/SentryClips" "$TESLACAM/EncryptedClips" \
    "$TESLACAM/SavedClips/2026-10-19_19-31-05" "$TESLACAM/SavedClips/2026-10-20_20-41-10"
fi

# library_clip <file> <camera index> <duration s> <caption>
library_clip() {
  local file=$1 index=$2 duration=$3 caption=$4
  if [[ -s "$file" ]]; then return 0; fi
  local name=${CAMERAS[$index]}
  ffmpeg -hide_banner -loglevel error -y \
    -f lavfi -i "testsrc2=size=480x360:rate=12:duration=${duration}" \
    -vf "hue=h=$((index * 60)):s=0.5,eq=brightness=-0.28,drawtext=fontfile=${FONT}:text='${name}':fontsize=34:fontcolor=white:borderw=3:x=(w-tw)/2:y=(h-th)/2-24,drawtext=fontfile=${FONT}:text='${caption}':fontsize=20:fontcolor=white@0.85:borderw=2:x=(w-tw)/2:y=h/2+24,format=yuv420p" \
    -c:v libx264 -preset ultrafast -crf 30 -g 24 -movflags +faststart -an "$file.tmp.mp4"
  mv "$file.tmp.mp4" "$file"
}

# segment <dir> <timestamp> <duration s> [missing camera index...]
segment() {
  local dir=$1 timestamp=$2 duration=$3
  shift 3
  local c
  for c in 0 1 2 3 4 5; do
    if [[ " $* " == *" $c "* ]]; then continue; fi
    run library_clip "$dir/${timestamp}-${CAMERAS[$c]}.mp4" "$c" "$duration" "$timestamp"
  done
}

# thumbnail <event dir> <segment timestamp>: thumb.png from the front clip, like the car writes it.
thumbnail() {
  local dir=$1 timestamp=$2
  [[ -s "$dir/thumb.png" ]] && return 0
  ffmpeg -hide_banner -loglevel error -y -ss 2 -i "$dir/${timestamp}-front.mp4" -frames:v 1 \
    -vf scale=192:144 "$dir/thumb.png"
}

# event_json <event dir> <detection timestamp> <reason> [city] [street]
event_json() {
  printf '{"timestamp":"%s","city":"%s","street":"%s","est_lat":"35.6581","est_lon":"139.7017","reason":"%s","camera":"0"}\n' \
    "$2" "${4:-}" "${5:-}" "$3" > "$1/event.json"
}

pids=()
RECENT="$TESLACAM/RecentClips"
mkdir -p "$RECENT"
# 2026-10-18: a morning drive and an evening drive.
segment "$RECENT" 2026-10-18_09-10-04 60
segment "$RECENT" 2026-10-18_09-11-04 42
segment "$RECENT" 2026-10-18_17-20-31 60
# 2026-10-19: a three-minute commute.
segment "$RECENT" 2026-10-19_08-12-00 60
segment "$RECENT" 2026-10-19_08-13-00 60
segment "$RECENT" 2026-10-19_08-14-00 25
# 2026-10-20: a morning drive and the evening drive next to the Sentry and Saved events.
segment "$RECENT" 2026-10-20_07-55-12 60
segment "$RECENT" 2026-10-20_07-56-12 60
segment "$RECENT" 2026-10-20_20-45-02 60
segment "$RECENT" 2026-10-20_20-46-02 31
# 2026-10-21: one short drive.
segment "$RECENT" 2026-10-21_07-58-40 60

SENTRY="$TESLACAM/SentryClips"
OBJECT="$SENTRY/2026-10-20_20-36-01"
mkdir -p "$OBJECT"
segment "$OBJECT" 2026-10-20_20-35-00 60
NIGHT="$SENTRY/2026-10-19_02-16-30"
mkdir -p "$NIGHT"
segment "$NIGHT" 2026-10-19_02-15-02 60 3 # no left repeater
segment "$NIGHT" 2026-10-19_02-16-02 60 3
SHAKE="$SENTRY/2026-10-18_22-05-44"
mkdir -p "$SHAKE"
segment "$SHAKE" 2026-10-18_22-04-10 60

SAVED_DINNER="$SAVED/2026-10-19_19-31-05"
mkdir -p "$SAVED_DINNER"
segment "$SAVED_DINNER" 2026-10-19_19-30-05 60
SAVED_EVENING="$SAVED/2026-10-20_20-41-10"
mkdir -p "$SAVED_EVENING"
segment "$SAVED_EVENING" 2026-10-20_20-40-08 60

for p in "${pids[@]}"; do wait "$p"; done

thumbnail "$OBJECT" 2026-10-20_20-35-00
thumbnail "$NIGHT" 2026-10-19_02-15-02
thumbnail "$SAVED_DINNER" 2026-10-19_19-30-05
thumbnail "$SAVED_EVENING" 2026-10-20_20-40-08
event_json "$OBJECT" 2026-10-20T20:35:30 sentry_aware_object_detection 渋谷区 神南一丁目
event_json "$NIGHT" 2026-10-19T02:15:40.250 sentry_aware_object_detection 渋谷区 宇田川町
event_json "$SHAKE" 2026-10-18T22:04:55 sentry_aware_accel_0.31 目黒区
event_json "$SAVED_DINNER" 2026-10-19T19:31:05 user_interaction_honk 港区 南青山五丁目
event_json "$SAVED_EVENING" 2026-10-20T20:41:10 user_interaction_dashcam_icon_tapped 渋谷区 神南一丁目

ENCRYPTED="$TESLACAM/EncryptedClips"
mkdir -p "$ENCRYPTED"
printf 'encrypted' > "$ENCRYPTED/2026-10-17_10-00-00-front.mp4"
printf 'encrypted' > "$ENCRYPTED/2026-10-17_10-00-00-back.mp4"
printf 'notes' > "$TESLACAM/notes.txt"

echo "$LIBRARY_VERSION" > "$TESLACAM/.library"
echo "done: $TESLACAM"
