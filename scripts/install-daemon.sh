#!/usr/bin/env bash
# Install the humos-daemon as a macOS LaunchAgent so it starts on login
# and restarts automatically if it crashes.
#
# Run once after building the daemon:
#   cargo build --release -p humos-daemon
#   ./scripts/install-daemon.sh
#
# To uninstall:
#   launchctl bootout gui/$UID/dev.humos.daemon
#   rm ~/Library/LaunchAgents/dev.humos.daemon.plist

set -e

REPO_ROOT=$(git rev-parse --show-toplevel)
BINARY="$REPO_ROOT/target/release/humos-daemon"
PLIST="$HOME/Library/LaunchAgents/dev.humos.daemon.plist"
LABEL="dev.humos.daemon"

if [ ! -f "$BINARY" ] || [ ! -x "$BINARY" ]; then
  echo "ERROR: binary not found or not executable at $BINARY"
  echo "Run: cargo build --release -p humos-daemon"
  exit 1
fi

mkdir -p "$HOME/.humOS"

# Unload existing agent before writing the new plist to avoid launchctl
# reading a partially-written file during replacement.
launchctl bootout "gui/$UID/$LABEL" 2>/dev/null || true

cat > "$PLIST" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>$LABEL</string>

    <key>ProgramArguments</key>
    <array>
        <string>$BINARY</string>
        <string>run</string>
    </array>

    <key>RunAtLoad</key>
    <true/>

    <key>KeepAlive</key>
    <true/>

    <key>StandardOutPath</key>
    <string>$HOME/.humOS/daemon.log</string>

    <key>StandardErrorPath</key>
    <string>$HOME/.humOS/daemon.log</string>

    <key>WorkingDirectory</key>
    <string>$HOME</string>
</dict>
</plist>
EOF

launchctl bootstrap "gui/$UID" "$PLIST"

echo "humos-daemon installed and started."
echo "Logs: tail -f ~/.humOS/daemon.log"
echo "Status: launchctl print gui/$UID/$LABEL"
