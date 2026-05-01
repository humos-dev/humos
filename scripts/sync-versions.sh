#!/usr/bin/env bash
# Sync the version field across every file that should mirror the canonical
# value in src-tauri/tauri.conf.json.
#
# Usage:
#   ./scripts/sync-versions.sh         # propagate tauri.conf.json -> others
#   ./scripts/sync-versions.sh --check # exit 1 if files are out of sync
#
# Files touched:
#   src-tauri/Cargo.toml   ([package] version)
#   package.json           ("version")

set -euo pipefail

cd "$(git rev-parse --show-toplevel)"

CANONICAL_FILE="src-tauri/tauri.conf.json"
if [ ! -f "$CANONICAL_FILE" ]; then
  echo "ERROR: $CANONICAL_FILE not found." >&2
  exit 1
fi

# Extract the top-level "version" from tauri.conf.json. Path is passed via
# environment to avoid shell quoting hazards in the python heredoc.
VERSION=$(CANONICAL_FILE="$CANONICAL_FILE" python3 -c '
import json, os, sys
with open(os.environ["CANONICAL_FILE"]) as f:
    data = json.load(f)
v = data.get("version")
if not v:
    sys.exit("tauri.conf.json missing top-level version field")
sys.stdout.write(v)
')

# Validate X.Y.Z (allow optional pre-release suffix like -alpha.1).
if ! echo "$VERSION" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+(-[A-Za-z0-9.-]+)?$'; then
  echo "ERROR: version '$VERSION' does not look like X.Y.Z" >&2
  exit 1
fi

CHECK_ONLY=0
if [ "${1:-}" = "--check" ]; then
  CHECK_ONLY=1
fi

drift=0

# Cargo.toml [package] version
CARGO_VERSION=$(awk '
  /^\[package\]/ { in_pkg = 1; next }
  /^\[/ { in_pkg = 0 }
  in_pkg && /^version *= *"/ {
    match($0, /"[^"]+"/)
    print substr($0, RSTART+1, RLENGTH-2)
    exit
  }
' src-tauri/Cargo.toml)

if [ "$CARGO_VERSION" != "$VERSION" ]; then
  drift=1
  if [ "$CHECK_ONLY" -eq 1 ]; then
    echo "drift: src-tauri/Cargo.toml is $CARGO_VERSION, canonical is $VERSION"
  else
    awk -v ver="$VERSION" '
      /^\[package\]/ { in_pkg = 1; print; next }
      /^\[/ { in_pkg = 0 }
      in_pkg && /^version *= *"/ { print "version = \"" ver "\""; next }
      { print }
    ' src-tauri/Cargo.toml > src-tauri/Cargo.toml.tmp
    mv src-tauri/Cargo.toml.tmp src-tauri/Cargo.toml
    echo "updated: src-tauri/Cargo.toml -> $VERSION"
  fi
fi

# package.json top-level version
PKG_VERSION=$(python3 -c '
import json
with open("package.json") as f:
    print(json.load(f).get("version", ""))
')

if [ "$PKG_VERSION" != "$VERSION" ]; then
  drift=1
  if [ "$CHECK_ONLY" -eq 1 ]; then
    echo "drift: package.json is $PKG_VERSION, canonical is $VERSION"
  else
    NEW_VERSION="$VERSION" python3 -c '
import json, os
with open("package.json") as f:
    data = json.load(f)
data["version"] = os.environ["NEW_VERSION"]
with open("package.json", "w") as f:
    json.dump(data, f, indent=2)
    f.write("\n")
'
    echo "updated: package.json -> $VERSION"
  fi
fi

if [ "$CHECK_ONLY" -eq 1 ]; then
  if [ "$drift" -ne 0 ]; then
    echo "Run scripts/sync-versions.sh to fix." >&2
    exit 1
  fi
  echo "all version sources match $VERSION"
fi

if [ "$drift" -eq 0 ] && [ "$CHECK_ONLY" -eq 0 ]; then
  echo "no drift, all sources at $VERSION"
fi
