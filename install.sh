#!/bin/sh
# Installs the latest Sentinel for macOS from the GitHub release.
#
#   curl -fsSL https://raw.githubusercontent.com/khanblair/sentinel-harness/main/install.sh | sh
#
# It downloads the disk image with curl, checks its SHA-256 against the release, and copies
# Sentinel.app into /Applications (or the folder in SENTINEL_INSTALL_DIR). A file that curl
# fetched has no quarantine flag, so macOS opens the app without the "could not verify" block.
# Read the script first if you like: it is short, and it only touches the install folder.
set -eu

REPO="khanblair/sentinel-harness"
DEST="${SENTINEL_INSTALL_DIR:-/Applications}"

if [ "$(uname -s)" != "Darwin" ]; then
  echo "This installer is for macOS. Linux and Windows builds are not released yet." >&2
  exit 1
fi

WORK="$(mktemp -d)"
MOUNT="$WORK/mount"
cleanup() {
  hdiutil detach "$MOUNT" -quiet 2>/dev/null || true
  rm -rf "$WORK"
}
trap cleanup EXIT INT TERM

echo "Looking up the latest release of $REPO ..."
API="https://api.github.com/repos/$REPO/releases/latest"
URLS="$(curl -fsSL "$API" | grep -o '"browser_download_url": *"[^"]*"' | sed 's/.*"\(https[^"]*\)"/\1/')"
DMG_URL="$(printf '%s\n' "$URLS" | grep '\.dmg$' | head -1)"
SUM_URL="$(printf '%s\n' "$URLS" | grep '\.dmg\.sha256$' | head -1)"
if [ -z "$DMG_URL" ] || [ -z "$SUM_URL" ]; then
  echo "The latest release has no .dmg and .dmg.sha256 files yet." >&2
  exit 1
fi

DMG="$WORK/$(basename "$DMG_URL")"
echo "Downloading $(basename "$DMG_URL") ..."
curl -fL --progress-bar -o "$DMG" "$DMG_URL"
curl -fsSL -o "$DMG.sha256" "$SUM_URL"

echo "Checking the SHA-256 ..."
WANT="$(awk '{print $1}' "$DMG.sha256")"
HAVE="$(shasum -a 256 "$DMG" | awk '{print $1}')"
if [ "$WANT" != "$HAVE" ]; then
  echo "The download does not match its checksum. Nothing was installed." >&2
  exit 1
fi

mkdir -p "$MOUNT"
hdiutil attach "$DMG" -nobrowse -readonly -mountpoint "$MOUNT" -quiet
if [ ! -d "$MOUNT/Sentinel.app" ]; then
  echo "The disk image has no Sentinel.app. Nothing was installed." >&2
  exit 1
fi

mkdir -p "$DEST"
rm -rf "$DEST/Sentinel.app"
cp -R "$MOUNT/Sentinel.app" "$DEST/Sentinel.app"
echo "Installed $DEST/Sentinel.app"
echo "Open it from Launchpad or Spotlight. It asks for a project folder the first time."
