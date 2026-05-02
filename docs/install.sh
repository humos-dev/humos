#!/usr/bin/env bash
set -euo pipefail

REPO="humos-dev/humos"

if [ "$(uname -m)" != "arm64" ]; then
  echo "humOS requires Apple Silicon (arm64). Detected: $(uname -m)" >&2
  exit 1
fi

# Clean up temp dir on any exit (success or failure).
TMPDIR=$(mktemp -d)
trap 'rm -rf "$TMPDIR"' EXIT

echo "Finding latest humOS release..."
API_RESPONSE=$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" 2>&1 || true)

if echo "$API_RESPONSE" | grep -q '"API rate limit exceeded"'; then
  echo "GitHub API rate limit hit. Wait a few minutes and try again." >&2
  exit 1
fi

DOWNLOAD_URL=$(echo "$API_RESPONSE" \
  | grep '"browser_download_url"' \
  | grep 'arm64\.zip' \
  | head -1 \
  | cut -d'"' -f4)

if [ -z "$DOWNLOAD_URL" ]; then
  echo "Could not find a release ZIP. Check https://github.com/$REPO/releases" >&2
  exit 1
fi

ZIP="$TMPDIR/humos.zip"
echo "Downloading $DOWNLOAD_URL..."
curl -fL --progress-bar "$DOWNLOAD_URL" -o "$ZIP"

echo "Extracting..."
unzip -q "$ZIP" -d "$TMPDIR"

APP=$(find "$TMPDIR" -name "humOS.app" -maxdepth 2 | head -1)
if [ -z "$APP" ]; then
  echo "humOS.app not found in archive." >&2
  exit 1
fi

# Clear Gatekeeper quarantine from the extracted app bundle.
echo "Clearing macOS quarantine..."
xattr -cr "$APP"

if [ -d "/Applications/humOS.app" ]; then
  echo "Removing existing /Applications/humOS.app..."
  rm -rf "/Applications/humOS.app"
fi

echo "Installing to /Applications..."
if [ -w "/Applications" ]; then
  mv "$APP" /Applications/humOS.app
else
  osascript -e "do shell script \"mv '$(printf "%q" "$APP")' '/Applications/humOS.app'\" with administrator privileges" 2>/dev/null || {
    echo "Authentication cancelled or failed. Use the manual install." >&2
    exit 1
  }
fi

echo ""
echo "humOS installed. Open it with:"
echo "  open /Applications/humOS.app"
