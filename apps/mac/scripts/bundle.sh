#!/usr/bin/env bash
# Build the release binary and assemble "TeslaCam Viewer.app" in target/bundle/.
#
#   scripts/bundle.sh            # prints the bundle's path
#   open "target/bundle/TeslaCam Viewer.app"
#
# The bundle is neither signed nor notarized: macOS Gatekeeper asks before the first launch of a
# copy downloaded from elsewhere (right-click > Open). Fonts, icons and the car image are compiled
# into the binary, so the bundle holds the executable, Info.plist and the app icon, which is
# rendered from assets/AppIcon.svg with sips and iconutil.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
NAME="TeslaCam Viewer"
EXECUTABLE=teslacam-viewer
BUNDLE_ID=run.kan.teslacam-viewer
VERSION="$(sed -n 's/^version = "\(.*\)"$/\1/p' "$ROOT/Cargo.toml" | head -n 1)"

(cd "$ROOT" && cargo build --release -p teslacam-app)

APP="$ROOT/target/bundle/$NAME.app"
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$ROOT/target/release/$EXECUTABLE" "$APP/Contents/MacOS/$EXECUTABLE"
printf 'APPL????' > "$APP/Contents/PkgInfo"

ICONSET="$ROOT/target/bundle/AppIcon.iconset"
rm -rf "$ICONSET"
mkdir -p "$ICONSET"
sips -s format png "$ROOT/assets/AppIcon.svg" --out "$ICONSET/icon_512x512@2x.png" >/dev/null
for size in 16 32 128 256 512; do
  sips -z "$size" "$size" "$ICONSET/icon_512x512@2x.png" --out "$ICONSET/icon_${size}x${size}.png" >/dev/null
  double=$((size * 2))
  [[ $size == 512 ]] || sips -z "$double" "$double" "$ICONSET/icon_512x512@2x.png" \
    --out "$ICONSET/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$ICONSET" -o "$APP/Contents/Resources/AppIcon.icns"
rm -rf "$ICONSET"

cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key>
  <string>ja</string>
  <key>CFBundleDisplayName</key>
  <string>$NAME</string>
  <key>CFBundleIconFile</key>
  <string>AppIcon</string>
  <key>CFBundleExecutable</key>
  <string>$EXECUTABLE</string>
  <key>CFBundleIdentifier</key>
  <string>$BUNDLE_ID</string>
  <key>CFBundleInfoDictionaryVersion</key>
  <string>6.0</string>
  <key>CFBundleName</key>
  <string>$NAME</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleShortVersionString</key>
  <string>$VERSION</string>
  <key>CFBundleVersion</key>
  <string>$VERSION</string>
  <key>LSApplicationCategoryType</key>
  <string>public.app-category.video</string>
  <key>LSMinimumSystemVersion</key>
  <string>14.0</string>
  <key>NSHighResolutionCapable</key>
  <true/>
</dict>
</plist>
PLIST

plutil -lint "$APP/Contents/Info.plist" >/dev/null

# Seal the whole bundle with an ad-hoc signature (no Developer ID, not notarized) so a downloaded copy
# is reported as from an unidentified developer rather than as damaged.
codesign --force --deep --sign - --identifier "$BUNDLE_ID" "$APP"

# A zip for GitHub Releases; ditto keeps the bundle's metadata and signature intact.
ZIP="$(dirname "$APP")/TeslaCam-Viewer-$VERSION-macos-arm64.zip"
rm -f "$ZIP"
ditto -c -k --keepParent "$APP" "$ZIP"

echo "$APP"
echo "$ZIP"
