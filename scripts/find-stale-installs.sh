#!/usr/bin/env bash
# Detect orphan humOS.app copies sitting outside /Applications.
#
# A stray humOS.app on Desktop, Downloads, or in a backup folder eventually
# causes "I installed the latest version but it's acting weird" reports.
# Spotlight, Dock searches, or LaunchServices may pick the wrong copy.
#
# Run by hand or call from scripts/release.sh after a successful ship to
# nudge cleanup of any leftover ZIP-extracted copies.
#
# Usage:
#   ./scripts/find-stale-installs.sh           # report only
#   ./scripts/find-stale-installs.sh --delete  # delete confirmed-stale copies (with prompt)

set -euo pipefail

DELETE=0
[ "${1:-}" = "--delete" ] && DELETE=1

red=$'\033[31m'
yellow=$'\033[33m'
green=$'\033[32m'
reset=$'\033[0m'

# Standard install location is canonical. Anything else is orphan candidate.
CANONICAL="/Applications/humOS.app"

# Paths we scan for stray copies. Order matters for output readability.
SEARCH_PATHS=(
  "$HOME/Downloads"
  "$HOME/Desktop"
  "$HOME/Documents"
  "$HOME"
  "/tmp"
)

# Also scan mounted volumes if present.
for vol in /Volumes/*; do
  [ -d "$vol" ] && SEARCH_PATHS+=("$vol")
done

# Build the find expression. Limit depth to avoid scanning entire backup
# directories or external drives forever.
echo "==> scanning for stray humOS.app copies"
found=()
for p in "${SEARCH_PATHS[@]}"; do
  [ ! -d "$p" ] && continue
  while IFS= read -r hit; do
    # Skip the canonical install.
    [ "$hit" = "$CANONICAL" ] && continue
    found+=("$hit")
  done < <(find "$p" -maxdepth 4 -type d -name 'humOS.app' 2>/dev/null)
done

if [ ${#found[@]} -eq 0 ]; then
  printf "%sno stray copies found%s\n" "$green" "$reset"
  exit 0
fi

printf "%sfound %d stray copy/copies:%s\n" "$yellow" "${#found[@]}" "$reset"
echo ""

for app in "${found[@]}"; do
  ver=$(/usr/libexec/PlistBuddy -c "Print :CFBundleShortVersionString" "$app/Contents/Info.plist" 2>/dev/null || echo "?")
  size=$(du -sh "$app" 2>/dev/null | awk '{print $1}')
  mtime=$(stat -f '%Sm' "$app" 2>/dev/null)
  printf "  %s\n" "$app"
  printf "    version: %s   size: %s   mtime: %s\n" "$ver" "$size" "$mtime"
done

if [ "$DELETE" -ne 1 ]; then
  echo ""
  echo "Re-run with --delete to remove these (with confirmation)."
  exit 0
fi

echo ""
read -r -p "Delete all listed copies? [y/N] " confirm
case "$confirm" in
  y|Y|yes|YES) ;;
  *) echo "aborted"; exit 0 ;;
esac

for app in "${found[@]}"; do
  rm -rf "$app"
  printf "  %sdeleted%s %s\n" "$red" "$reset" "$app"
done
echo ""
printf "%sdone%s\n" "$green" "$reset"
