#!/usr/bin/env bash
set -euo pipefail

REPO="humos-dev/humos"

if [ "$(uname -m)" != "arm64" ]; then
  echo "humOS requires Apple Silicon (arm64). Detected: $(uname -m)" >&2
  exit 1
fi

echo "Finding latest humOS release..."
DOWNLOAD_URL=$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" \
  | grep '"browser_download_url"' \
  | grep 'arm64\.zip' \
  | head -1 \
  | cut -d'"' -f4)

if [ -z "$DOWNLOAD_URL" ]; then
  echo "Could not find a release ZIP. Check https://github.com/$REPO/releases" >&2
  exit 1
fi

TMPDIR=$(mktemp -d)
ZIP="$TMPDIR/humos.zip"

echo "Downloading $DOWNLOAD_URL..."
curl -fL --progress-bar "$DOWNLOAD_URL" -o "$ZIP"

# Clear quarantine before extracting so the app inherits none of it.
xattr -cr "$ZIP"

echo "Extracting..."
unzip -q "$ZIP" -d "$TMPDIR"

APP=$(find "$TMPDIR" -name "humOS.app" -maxdepth 2 | head -1)
if [ -z "$APP" ]; then
  echo "humOS.app not found in archive." >&2
  rm -rf "$TMPDIR"
  exit 1
fi

if [ -d "/Applications/humOS.app" ]; then
  echo "Removing existing /Applications/humOS.app..."
  rm -rf "/Applications/humOS.app"
fi

echo "Installing to /Applications..."
mv "$APP" /Applications/humOS.app

rm -rf "$TMPDIR"

echo ""
echo "humOS installed. Open it with:"
echo "  open /Applications/humOS.app"
