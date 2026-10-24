#!/usr/bin/env bash
# Sends a signed Sentinel.app or Sentinel.dmg to Apple for notarization, waits for the answer, and
# staples the ticket so the file also opens offline.
#
#   packaging/macos/notarize.sh PATH
#
# Needs APPLE_ID, APPLE_TEAM_ID, and APPLE_APP_PASSWORD in the environment. For an .app the script
# zips a copy first, because Apple takes a zip, a pkg, or a dmg. When Apple says no, the script
# prints Apple's log (it names the exact file and problem) and fails. After stapling it asks macOS
# itself whether Gatekeeper accepts the file, and fails if it does not.
set -euo pipefail

TARGET="$1"
: "${APPLE_ID:?APPLE_ID is not set}" "${APPLE_TEAM_ID:?APPLE_TEAM_ID is not set}" "${APPLE_APP_PASSWORD:?APPLE_APP_PASSWORD is not set}"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

case "$TARGET" in
  *.app)
    SUBMIT="$WORK/$(basename "$TARGET").zip"
    ditto -c -k --keepParent "$TARGET" "$SUBMIT"
    ;;
  *.dmg) SUBMIT="$TARGET" ;;
  *) echo "notarize.sh takes an .app or a .dmg" >&2; exit 2 ;;
esac

echo "Submitting $(basename "$SUBMIT") to Apple ..."
xcrun notarytool submit "$SUBMIT" --apple-id "$APPLE_ID" --team-id "$APPLE_TEAM_ID" \
  --password "$APPLE_APP_PASSWORD" --wait --output-format json > "$WORK/result.json" || true
cat "$WORK/result.json"; echo

STATUS="$(/usr/bin/python3 -c 'import json,sys; print(json.load(open(sys.argv[1])).get("status",""))' "$WORK/result.json" 2>/dev/null || true)"
ID="$(/usr/bin/python3 -c 'import json,sys; print(json.load(open(sys.argv[1])).get("id",""))' "$WORK/result.json" 2>/dev/null || true)"
if [ "$STATUS" != "Accepted" ]; then
  echo "Apple did not accept the file (status: ${STATUS:-none}). Apple's log:" >&2
  if [ -n "$ID" ]; then
    xcrun notarytool log "$ID" --apple-id "$APPLE_ID" --team-id "$APPLE_TEAM_ID" \
      --password "$APPLE_APP_PASSWORD" >&2 || true
  fi
  exit 1
fi

xcrun stapler staple "$TARGET"
xcrun stapler validate "$TARGET"
case "$TARGET" in
  *.app) spctl -a -vvv -t exec "$TARGET" ;;
  *.dmg) spctl -a -vvv -t open --context context:primary-signature "$TARGET" ;;
esac
echo "Notarized and stapled: $TARGET"
