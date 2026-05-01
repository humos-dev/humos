# Git hooks

Tracked copies of git hooks that should fire on every commit in this repo.

## What's enforced

`pre-commit` blocks two things:

1. **Em dashes** in any staged file. Project convention is zero tolerance.
   Use commas, periods, or `...` instead.
2. **AI-slop vocabulary** in prose files (`.md`, `.html`). Words like
   "delve", "robust", "comprehensive", "pivotal" and phrases like
   "here's the kicker" get blocked. Code files (`.rs`, `.ts`, etc.) are
   exempt because words like "significant" can be legitimate identifier
   names. The hook scopes word checks to prose files only.

## Install

```
./scripts/install-hooks.sh
```

This copies every script under `scripts/hooks/` into `.git/hooks/` and
makes them executable. Re-run after pulling changes to the hook scripts.

## Override (use sparingly)

```
git commit --no-verify
```

Bypasses the hook for one commit. Reserved for genuine cases like importing
third-party content that already contains banned patterns.

## Disable temporarily

```
chmod -x .git/hooks/pre-commit
```

## Why local-only

`.git/hooks/` is not tracked by git, so the hook doesn't propagate via
`git pull`. The tracked copy in `scripts/hooks/` plus the installer is
the standard pattern for solo or small-team repos that don't want the
weight of husky.
