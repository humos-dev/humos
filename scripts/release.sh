#!/usr/bin/env bash
# Atomic humOS release: bump version, build, commit, tag, push, create
# GitHub release with the ZIP attached.
#
# Usage:
#   ./scripts/release.sh patch     # 0.5.6 -> 0.5.7
#   ./scripts/release.sh minor     # 0.5.6 -> 0.6.0
#   ./scripts/release.sh major     # 0.5.6 -> 1.0.0
#   ./scripts/release.sh 0.6.0     # explicit version
#   ./scripts/release.sh patch --dry-run    # show what would happen, do nothing
#
# What it does, in order:
#   1. Sanity checks: on main, working tree clean, gh CLI present, origin set.
#   2. Bumps src-tauri/tauri.conf.json to the new version.
#   3. Runs scripts/sync-versions.sh to propagate to Cargo.toml + package.json.
#   4. Runs scripts/build-release.sh to produce the .app and ZIP, and to
#      regenerate docs/version.json with the new version and a fresh URL.
#   5. Commits "chore: release vX.Y.Z" with the four updated files.
#   6. Tags the commit as vX.Y.Z.
#   7. Pushes main and the tag to origin.
#   8. Creates a GitHub release with the ZIP attached and notes pulled from
#      the matching CHANGELOG.md section.
#
# After step 7, Vercel auto-deploys docs/, so humos.dev/version.json points
# to the new release within about a minute. Existing users get the banner
# on their next app launch.
#
# Prereqs:
#   - gh CLI installed and authenticated
#   - Working tree clean, on main, ahead of origin OK
#   - The matching CHANGELOG.md section already written (this script does
#     not invent release notes)

set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

DRY_RUN=0
ARG="${1:-}"
shift || true
for f in "$@"; do
  case "$f" in
    --dry-run) DRY_RUN=1 ;;
    *) echo "unknown flag: $f"; exit 1 ;;
  esac
done

run() {
  if [ "$DRY_RUN" -eq 1 ]; then
    echo "DRY-RUN: $*"
  else
    "$@"
  fi
}

# ---- Sanity checks ----
BRANCH=$(git symbolic-ref --short HEAD)
if [ "$BRANCH" != "main" ]; then
  echo "ERROR: must be on main, got '$BRANCH'" >&2
  exit 1
fi

if [ -n "$(git status --porcelain)" ]; then
  echo "ERROR: working tree dirty. Commit or stash first." >&2
  git status --short
  exit 1
fi

if ! command -v gh > /dev/null; then
  echo "ERROR: gh CLI not found. Install with: brew install gh" >&2
  exit 1
fi

if ! git remote get-url origin > /dev/null 2>&1; then
  echo "ERROR: no 'origin' remote configured" >&2
  exit 1
fi

# ---- Compute new version ----
CURRENT=$(python3 -c 'import json; print(json.load(open("src-tauri/tauri.conf.json"))["version"])')

case "$ARG" in
  patch)
    NEW=$(CURRENT="$CURRENT" python3 -c '
import os
v = os.environ["CURRENT"].split(".")
print(f"{v[0]}.{v[1]}.{int(v[2])+1}")
')
    ;;
  minor)
    NEW=$(CURRENT="$CURRENT" python3 -c '
import os
v = os.environ["CURRENT"].split(".")
print(f"{v[0]}.{int(v[1])+1}.0")
')
    ;;
  major)
    NEW=$(CURRENT="$CURRENT" python3 -c '
import os
v = os.environ["CURRENT"].split(".")
print(f"{int(v[0])+1}.0.0")
')
    ;;
  [0-9]*.[0-9]*.[0-9]*)
    NEW="$ARG"
    ;;
  *)
    echo "Usage: $0 {patch|minor|major|X.Y.Z} [--dry-run]" >&2
    exit 1
    ;;
esac

if ! echo "$NEW" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+$'; then
  echo "ERROR: computed version '$NEW' is not X.Y.Z" >&2
  exit 1
fi

if ! grep -q "^## \[$NEW\]" CHANGELOG.md; then
  echo "ERROR: CHANGELOG.md has no '## [$NEW]' section." >&2
  echo "Add release notes for v$NEW before running this script." >&2
  exit 1
fi

echo "==> Release v$NEW (was v$CURRENT)"
[ "$DRY_RUN" -eq 1 ] && echo "    DRY RUN: nothing will be modified."
read -r -p "Continue? [y/N] " confirm
case "$confirm" in
  y|Y|yes|YES) ;;
  *) echo "aborted"; exit 0 ;;
esac

# ---- 1. Bump canonical version ----
run env NEW="$NEW" python3 -c '
import json, os
with open("src-tauri/tauri.conf.json") as f:
    data = json.load(f)
data["version"] = os.environ["NEW"]
with open("src-tauri/tauri.conf.json", "w") as f:
    json.dump(data, f, indent=2)
    f.write("\n")
'
echo "==> Bumped tauri.conf.json to $NEW"

# ---- 2. Sync derived versions ----
run ./scripts/sync-versions.sh

# ---- 3. Build (produces ZIP + docs/version.json) ----
run ./scripts/build-release.sh

# ---- 4. Commit ----
run git add src-tauri/tauri.conf.json src-tauri/Cargo.toml package.json docs/version.json
run git commit -m "chore: release v$NEW"

# ---- 5. Tag ----
run git tag -a "v$NEW" -m "humOS v$NEW"

# ---- 6. Push ----
run git push origin main
run git push origin "v$NEW"

# ---- 7. Extract notes from CHANGELOG ----
NOTES=$(awk -v ver="$NEW" '
  /^## \[/ {
    if (in_section) exit
    if (index($0, "[" ver "]")) { in_section = 1; next }
  }
  in_section { print }
' CHANGELOG.md)

if [ -z "$NOTES" ]; then
  echo "WARN: empty release notes extracted for v$NEW" >&2
fi

ARCH=$(uname -m)
ZIP_PATH="target/release/bundle/macos/humOS_${NEW}_${ARCH}.zip"

if [ "$DRY_RUN" -eq 0 ] && [ ! -f "$ZIP_PATH" ]; then
  echo "ERROR: $ZIP_PATH not found. Build step must have failed." >&2
  exit 1
fi

# ---- 8. GitHub release ----
run gh release create "v$NEW" "$ZIP_PATH" \
  --repo humos-dev/humos \
  --title "v$NEW" \
  --notes "$NOTES"

echo ""
echo "Released v$NEW."
echo "  Tag:        https://github.com/humos-dev/humos/releases/tag/v$NEW"
echo "  Vercel:     deploys docs/version.json in about a minute"
echo "  Banner:     existing users see the update on next app launch"
echo "  Verify:     curl https://humos.dev/version.json"
