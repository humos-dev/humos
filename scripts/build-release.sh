#!/bin/bash
# Build humOS and package as ZIP (macOS 26 blocks unsigned DMGs)
set -euo pipefail

APP_NAME="humOS"
# Tauri uses the Cargo workspace root target/, not src-tauri/target/
# Using src-tauri/target/ installs a stale old bundle. Always use workspace root.
BUNDLE_DIR="target/release/bundle/macos"
APP_PATH="$BUNDLE_DIR/$APP_NAME.app"
VERSION=$(grep '"version"' src-tauri/tauri.conf.json | head -1 | sed 's/.*: "\(.*\)".*/\1/')
ARCH=$(uname -m)
ZIP_NAME="${APP_NAME}_${VERSION}_${ARCH}.zip"
ZIP_PATH="$BUNDLE_DIR/$ZIP_NAME"

# Tauri validates bundle resources at build time. humos-daemon must exist at
# target/release/humos-daemon before the app build runs, but it is also built
# by the same workspace. Always write a fresh placeholder so the validation
# passes and any stale placeholder from a prior failed build is replaced.
mkdir -p target/release
printf '#!/bin/sh\n' > target/release/humos-daemon
chmod +x target/release/humos-daemon
echo "==> Building humos-daemon..."
cargo build --release -p humos-daemon

# Verify the build produced a real Mach-O binary, not a leftover placeholder.
if ! file target/release/humos-daemon | grep -q "Mach-O"; then
  echo "ERROR: humos-daemon is not a valid Mach-O binary. Build may have failed silently."
  exit 1
fi

echo "==> Building humOS v$VERSION ($ARCH)..."
npm run tauri build -- --bundles app 2>&1 || true

if [ ! -d "$APP_PATH" ]; then
  echo "ERROR: $APP_PATH not found. Build failed."
  exit 1
fi

echo "==> Writing docs/version.json..."
cat > docs/version.json << VEOF
{
  "version": "$VERSION",
  "url": "https://github.com/humos-dev/humos/releases/tag/v$VERSION",
  "date": "$(date +%Y-%m-%d)"
}
VEOF
echo "    docs/version.json updated to v$VERSION"

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
