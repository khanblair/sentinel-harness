#!/usr/bin/env bash
# Stores the five signing secrets in the repository's GitHub Actions secrets, from YOUR terminal.
# Nothing is printed or written to disk, and the values go straight to `gh secret set`.
#
#   packaging/macos/set-signing-secrets.sh path/to/DeveloperID.p12
#
# Before you run it you need (docs/development.md, "Signing the .dmg"):
#   1. a "Developer ID Application" certificate exported from Keychain Access as a .p12 file
#   2. the Apple ID of your Developer Program account
#   3. an app-specific password made at account.apple.com (Sign-In and Security)
# and `gh auth login` done as the account that owns the repository.
set -euo pipefail

REPO="${REPO:-khanblair/sentinel-harness}"
P12="${1:-}"
if [ -z "$P12" ] || [ ! -f "$P12" ]; then
  echo "usage: $0 path/to/DeveloperID.p12" >&2
  exit 2
fi
command -v gh > /dev/null || { echo "Install the GitHub CLI first: brew install gh" >&2; exit 1; }
gh auth status > /dev/null 2>&1 || { echo "Run: gh auth login" >&2; exit 1; }

read -r -s -p "Password you gave the .p12 file: " P12_PASSWORD; echo

# Open the file in a throwaway keychain to prove the password is right and that it holds a
# Developer ID Application identity. The name is public: it is what the signature shows.
KEYCHAIN="$(mktemp -d)/check.keychain-db"
KC_PASSWORD="$(uuidgen)"
cleanup() { security delete-keychain "$KEYCHAIN" > /dev/null 2>&1 || true; }
trap cleanup EXIT
security create-keychain -p "$KC_PASSWORD" "$KEYCHAIN"
security import "$P12" -P "$P12_PASSWORD" -A -t cert -f pkcs12 -k "$KEYCHAIN" > /dev/null \
  || { echo "The .p12 file did not open. Check the password." >&2; exit 1; }
IDENTITY="$(security find-identity -v -p codesigning "$KEYCHAIN" | grep 'Developer ID Application' | head -1 | sed -E 's/.*"(.*)".*/\1/' || true)"
if [ -z "$IDENTITY" ]; then
  echo "That file has no 'Developer ID Application' certificate. (An 'Apple Development' or" >&2
  echo "'Apple Distribution' certificate cannot be used for apps outside the App Store.)" >&2
  exit 1
fi
echo "Found: $IDENTITY"
TEAM_GUESS="$(printf '%s' "$IDENTITY" | sed -E 's/.*\(([A-Z0-9]{10})\).*/\1/')"

read -r -p "Apple ID (the email of your Developer Program account): " APPLE_ID
read -r -p "Team ID [$TEAM_GUESS]: " TEAM_ID
TEAM_ID="${TEAM_ID:-$TEAM_GUESS}"
read -r -s -p "App-specific password (xxxx-xxxx-xxxx-xxxx): " APP_PASSWORD; echo

base64 -i "$P12" | gh secret set APPLE_CERTIFICATE --repo "$REPO"
printf '%s' "$P12_PASSWORD" | gh secret set APPLE_CERTIFICATE_PASSWORD --repo "$REPO"
printf '%s' "$APPLE_ID" | gh secret set APPLE_ID --repo "$REPO"
printf '%s' "$TEAM_ID" | gh secret set APPLE_TEAM_ID --repo "$REPO"
printf '%s' "$APP_PASSWORD" | gh secret set APPLE_APP_PASSWORD --repo "$REPO"

echo
echo "Done. Five secrets are set on $REPO. Now build a signed release:"
echo "  gh workflow run release.yml --repo $REPO -f tag=v0.1.1"
