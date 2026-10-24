#!/bin/sh
# Lists relative markdown links in site/src/**/*.md whose target file is missing.
# Image placeholders (![alt](path)) are skipped until the screenshots exist.
# Exit 1 when a target is missing, 0 when every relative link resolves.
set -eu

site_dir=$(cd "$(dirname "$0")" && pwd)
list=$(mktemp)
trap 'rm -f "$list"' EXIT

find "$site_dir/src" -name '*.md' | sort > "$list"

missing=""
while IFS= read -r file; do
    dir=$(dirname "$file")
    targets=$(sed 's/!\[[^]]*]([^)]*)//g' "$file" | grep -o '\[[^]]*]([^)]*)' | sed 's/^.*](//; s/)$//' || true)
    while IFS= read -r target; do
        if [ -z "$target" ]; then
            continue
        fi
        if printf '%s\n' "$target" | grep -Eq '^(https?://|mailto:|#)'; then
            continue
        fi
        path=$(printf '%s\n' "$target" | sed 's/#.*$//; s/?.*$//')
        if [ -z "$path" ]; then
            continue
        fi
        if [ ! -e "$dir/$path" ]; then
            missing="$missing
missing: ${file#"$site_dir"/}: $target"
        fi
    done <<EOF
$targets
EOF
done < "$list"

if [ -n "$missing" ]; then
    printf '%s\n' "$missing" | sed '/^$/d'
    exit 1
fi
echo "all relative links resolve"
