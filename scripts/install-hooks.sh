#!/usr/bin/env bash
# Install humOS git hooks into .git/hooks/.
#
# Run once after cloning the repo:
#   ./scripts/install-hooks.sh
#
# This copies (not symlinks) the tracked hook scripts into .git/hooks/ so
# they fire on commit. Re-run after pulling changes to scripts/hooks/.

set -e

repo_root=$(git rev-parse --show-toplevel)
src="$repo_root/scripts/hooks"
dst="$repo_root/.git/hooks"

if [ ! -d "$src" ]; then
  echo "ERROR: $src not found. Are you in the humOS repo?"
  exit 1
fi

installed=0
for hook in "$src"/*; do
  [ ! -f "$hook" ] && continue
  name=$(basename "$hook")
  cp "$hook" "$dst/$name"
  chmod +x "$dst/$name"
  echo "installed: $dst/$name"
  installed=$((installed + 1))
done

echo "$installed hook(s) installed."
