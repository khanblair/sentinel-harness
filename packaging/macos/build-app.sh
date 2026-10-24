#!/usr/bin/env bash
# Builds Sentinel.app from a finished `sentinel` binary, then signs it.
#
#   packaging/macos/build-app.sh BINARY VERSION BUILD OUT_DIR
#
# BINARY is the (universal) release binary with the UI embedded. The app runs `sentinel app`
# when Finder opens it (the binary does that itself when it lives inside a .app bundle).
#
# Signing: with SIGNING_IDENTITY set (a "Developer ID Application: ..." name) the app is signed
# with the hardened runtime and a secure timestamp, which notarization needs. Without it the app
# gets an ad-hoc signature: it runs on this Mac, but another Mac asks you to confirm the first
# open (right-click, Open).
set -euo pipefail

BINARY="$1"; VERSION="$2"; BUILD="$3"; OUT="$4"
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
APP="$OUT/Sentinel.app"

rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$BINARY" "$APP/Contents/MacOS/sentinel"
chmod 755 "$APP/Contents/MacOS/sentinel"
sed -e "s/@VERSION@/$VERSION/" -e "s/@BUILD@/$BUILD/" "$HERE/Info.plist.in" > "$APP/Contents/Info.plist"

# The icon: the app's 512 pixel icon, scaled down to the sizes an .icns holds.
ICONSET="$OUT/Sentinel.iconset"
rm -rf "$ICONSET"; mkdir -p "$ICONSET"
SRC="$ROOT/crates/sentinel-ui/assets/icons/icon-512.png"
for size in 16 32 64 128 256 512; do
  sips -z "$size" "$size" "$SRC" --out "$ICONSET/icon_${size}x${size}.png" > /dev/null
done
cp "$ICONSET/icon_32x32.png" "$ICONSET/icon_16x16@2x.png"
cp "$ICONSET/icon_64x64.png" "$ICONSET/icon_32x32@2x.png"
cp "$ICONSET/icon_256x256.png" "$ICONSET/icon_128x128@2x.png"
cp "$ICONSET/icon_512x512.png" "$ICONSET/icon_256x256@2x.png"
rm "$ICONSET/icon_64x64.png"
iconutil -c icns "$ICONSET" -o "$APP/Contents/Resources/Sentinel.icns"
rm -rf "$ICONSET"

if [ -n "${SIGNING_IDENTITY:-}" ]; then
  codesign --force --options runtime --timestamp --sign "$SIGNING_IDENTITY" "$APP/Contents/MacOS/sentinel"
  codesign --force --options runtime --timestamp --sign "$SIGNING_IDENTITY" "$APP"
  echo "signed with: $SIGNING_IDENTITY"
else
  codesign --force --sign - "$APP/Contents/MacOS/sentinel"
  codesign --force --sign - "$APP"
  echo "signed ad hoc (no Developer ID certificate was given)"
fi
codesign --verify --strict --verbose=2 "$APP"
echo "$APP"
