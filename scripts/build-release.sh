#!/bin/bash
# Build humOS and package as ZIP (macOS 26 blocks unsigned DMGs)
set -euo pipefail

APP_NAME="humOS"
# Tauri uses the Cargo workspace root target/, not src-tauri/target/
# Using src-tauri/target/ installs a stale old bundle — always use workspace root
BUNDLE_DIR="target/release/bundle/macos"
APP_PATH="$BUNDLE_DIR/$APP_NAME.app"
VERSION=$(grep '"version"' src-tauri/tauri.conf.json | head -1 | sed 's/.*: "\(.*\)".*/\1/')
ARCH=$(uname -m)
ZIP_NAME="${APP_NAME}_${VERSION}_${ARCH}.zip"
ZIP_PATH="$BUNDLE_DIR/$ZIP_NAME"

echo "==> Building humOS v$VERSION ($ARCH)..."
npm run tauri build -- --bundles app 2>&1 || true

if [ ! -d "$APP_PATH" ]; then
  echo "ERROR: $APP_PATH not found. Build failed."
  exit 1
fi

echo "==> Clearing quarantine on .app..."
xattr -cr "$APP_PATH"

echo "==> Creating ZIP..."
cd "$BUNDLE_DIR"
ditto -c -k --keepParent "$APP_NAME.app" "$ZIP_NAME"
cd - > /dev/null

echo ""
echo "Done: $ZIP_PATH ($(du -h "$ZIP_PATH" | cut -f1))"
echo ""
echo "Install manually:"
echo "  unzip $ZIP_PATH -d /Applications"
echo "  xattr -cr /Applications/humOS.app"
echo ""
echo "Upload to GitHub release:"
echo "  gh release upload v$VERSION $ZIP_PATH --repo humos-dev/humos --clobber"
