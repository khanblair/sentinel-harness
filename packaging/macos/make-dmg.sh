#!/usr/bin/env bash
# Puts Sentinel.app in a disk image with a shortcut to Applications, and signs the image.
#
#   packaging/macos/make-dmg.sh APP DMG_PATH
#
# SIGNING_IDENTITY signs the image the same way build-app.sh signs the app. Without it the image
# is left unsigned (the app inside is still signed).
set -euo pipefail

APP="$1"; DMG="$2"
STAGE="$(mktemp -d)"
trap 'rm -rf "$STAGE"' EXIT

cp -R "$APP" "$STAGE/Sentinel.app"
ln -s /Applications "$STAGE/Applications"
rm -f "$DMG"
hdiutil create -volname "Sentinel" -srcfolder "$STAGE" -fs HFS+ -format UDZO -ov "$DMG" > /dev/null

if [ -n "${SIGNING_IDENTITY:-}" ]; then
  codesign --force --timestamp --sign "$SIGNING_IDENTITY" "$DMG"
  echo "disk image signed"
fi
echo "$DMG"
