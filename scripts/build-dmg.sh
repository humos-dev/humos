#!/bin/bash
# Build humOS and repackage DMG as APFS (macOS 26+ dropped HFS+ mount support)
set -euo pipefail

APP_NAME="humOS"
BUNDLE_DIR="src-tauri/target/release/bundle/macos"
APP_PATH="$BUNDLE_DIR/$APP_NAME.app"
VERSION=$(grep '"version"' src-tauri/tauri.conf.json | head -1 | sed 's/.*: "\(.*\)".*/\1/')
ARCH=$(uname -m)
DMG_NAME="${APP_NAME}_${VERSION}_${ARCH}.dmg"
DMG_PATH="$BUNDLE_DIR/$DMG_NAME"
TEMP_DIR=$(mktemp -d)

echo "==> Building humOS v$VERSION ($ARCH)..."
npm run tauri build

echo "==> Removing Tauri's HFS+ DMGs..."
rm -f "$BUNDLE_DIR"/*.dmg

echo "==> Creating APFS DMG..."
mkdir -p "$TEMP_DIR/dmg"
cp -R "$APP_PATH" "$TEMP_DIR/dmg/"
ln -s /Applications "$TEMP_DIR/dmg/Applications"

hdiutil create \
  -volname "$APP_NAME" \
  -srcfolder "$TEMP_DIR/dmg" \
  -ov \
  -fs APFS \
  -format UDZO \
  "$DMG_PATH"

rm -rf "$TEMP_DIR"

echo "==> Clearing quarantine..."
xattr -cr "$DMG_PATH"

echo ""
echo "Done: $DMG_PATH ($(du -h "$DMG_PATH" | cut -f1))"
echo "Install: open $DMG_PATH"