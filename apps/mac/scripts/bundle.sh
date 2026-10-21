#!/usr/bin/env bash
# Build the release binary and assemble "TeslaCam Viewer.app" in target/bundle/.
#
#   scripts/bundle.sh            # prints the bundle's path
#   open "target/bundle/TeslaCam Viewer.app"
#
# The bundle is neither signed nor notarized: macOS Gatekeeper asks before the first launch of a
# copy downloaded from elsewhere (right-click > Open). Fonts, icons and the car image are compiled
# into the binary, so the bundle holds the executable and Info.plist only.
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

cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key>
  <string>ja</string>
  <key>CFBundleDisplayName</key>
  <string>$NAME</string>
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
echo "$APP"
