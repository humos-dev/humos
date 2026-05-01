#!/usr/bin/env bash
# humOS release preflight: every gate that must pass before a release
# is allowed to bump versions. Runs in seconds, fails loudly.
#
# This script encodes the cadence steps that CAN be machine-checked. The
# parts of the cadence that require human judgment (Plan, Design, Audit)
# stay procedural. The parts that are mechanical (test, typecheck, lint,
# version sync, hook installation) live here.
#
# Used by:
#   scripts/release.sh   (called as the first step before any mutation)
#   manual run           (./scripts/preflight.sh)

set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

red=$'\033[31m'
green=$'\033[32m'
reset=$'\033[0m'

fail() {
  printf "%s%s%s\n" "$red" "$1" "$reset" >&2
  exit 1
}

ok() {
  printf "  %s✓%s %s\n" "$green" "$reset" "$1"
}

# ---- 1. Pre-commit hook installed ----
# A release built on a clone where the hook was never installed cannot
# trust its history: prior commits may carry em dashes or AI-slop that
# the hook would have rejected. Force install if missing.
if [ ! -x .git/hooks/pre-commit ]; then
  fail "ERROR: pre-commit hook not installed. Run ./scripts/install-hooks.sh first."
fi
ok "pre-commit hook installed"

# ---- 2. Version sources aligned ----
./scripts/sync-versions.sh --check >/dev/null 2>&1 || {
  echo "${red}ERROR: version sources are out of sync.${reset}" >&2
  ./scripts/sync-versions.sh --check 2>&1 | sed 's/^/  /' >&2
  echo "  Run ./scripts/sync-versions.sh to align them." >&2
  exit 1
}
ok "version sources aligned"

# ---- 3. Rust tests ----
# `--test-threads=1` keeps tests that mutate process-wide state (env vars,
# global locks) from racing each other. Slightly slower, more deterministic.
echo "==> cargo test (lib, serial)"
(cd src-tauri && cargo test --lib --quiet -- --test-threads=1 2>&1 | tail -20) || \
  fail "ERROR: cargo test failed. Fix tests before tagging a release."
ok "cargo test passed"

# ---- 4. TypeScript typecheck ----
echo "==> tsc --noEmit"
npx tsc --noEmit -p . 2>&1 | tail -20
if [ "${PIPESTATUS[0]:-0}" -ne 0 ]; then
  fail "ERROR: tsc found type errors. Fix before tagging a release."
fi
ok "TypeScript typecheck passed"

echo ""
printf "%sall preflight gates passed%s\n" "$green" "$reset"
