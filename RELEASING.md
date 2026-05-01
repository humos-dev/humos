# Releasing humOS

The full release flow runs in one command:

```
./scripts/release.sh patch    # 0.5.6 -> 0.5.7
./scripts/release.sh minor    # 0.5.6 -> 0.6.0
./scripts/release.sh major    # 0.5.6 -> 1.0.0
./scripts/release.sh 0.6.0    # explicit version
```

Add `--dry-run` to see what the command would do without changing anything.

## What you do BEFORE running it

1. Make sure all the work for this release is committed on `main`.
2. Add a section to `CHANGELOG.md` for the version you are about to release:

   ```
   ## [0.6.0] - 2026-05-17

   ### Added
   - opencode adapter via the Provider trait. signal() now broadcasts
     across both Claude Code and opencode tabs in one call.
   ...
   ```

   The section header must be `## [X.Y.Z]` matching the version. The
   release script reads release notes from this section. If the section
   is missing or named wrong, the script aborts before doing anything.

3. Confirm you are on `main` with a clean working tree.

## What the script does

1. Sanity checks: on `main`, working tree clean, `gh` CLI installed,
   `origin` remote configured, CHANGELOG section exists for the new
   version. Aborts on any failure.
2. Bumps `src-tauri/tauri.conf.json` to the new version (this is the
   canonical version source).
3. Runs `scripts/sync-versions.sh` to propagate the bump to
   `src-tauri/Cargo.toml` and `package.json`.
4. Runs `scripts/build-release.sh` to compile the app, package it as
   `humOS_X.Y.Z_arm64.zip`, and rewrite `docs/version.json` with the
   new version and matching release URL.
5. Commits `chore: release vX.Y.Z`.
6. Creates an annotated tag `vX.Y.Z`.
7. Pushes `main` and the tag to `origin`. Vercel auto-deploys `docs/`
   on push, which rewrites `humos.dev/version.json`.
8. Creates a GitHub release with the ZIP attached and release notes
   pulled from the CHANGELOG section.

## What happens for users after a release ships

The app polls `humos.dev/version.json` on startup. When the deployed
version is newer than the installed version, the update banner shows
between the header and the session grid:

```
↑ humOS X.Y.Z available · See what's new ↗ · ×
```

The banner is keyed on the remote version, so each new release
re-triggers it for users who dismissed the previous one. The "See
what's new" link uses the `url` field from `version.json` so it always
points to a real published release.

## Testing the banner before a release

Open the app, then in the devtools console:

```
localStorage.setItem("humos-test-update-banner", "9.9.9")
```

Reload. The banner appears as if v9.9.9 were available. Clear with:

```
localStorage.removeItem("humos-test-update-banner")
```

This bypasses the fetch and the dismiss check, so it always renders.
Works in both `tauri dev` and the production app.

## When the update banner does not appear

The banner is silent on three failure modes that the test mode does not
exercise:

- Network error or 3s timeout fetching `humos.dev/version.json`
- Malformed JSON at that URL
- Same version (no update available)

All three are now logged via `console.warn`, so open the devtools
console to see why the banner is missing.

## Rollback

If a release ships broken:

1. Delete the GitHub release: `gh release delete vX.Y.Z --repo humos-dev/humos`
2. Delete the tag: `git push origin :refs/tags/vX.Y.Z` and `git tag -d vX.Y.Z`
3. Revert the `chore: release vX.Y.Z` commit on main and push.
4. Vercel re-deploys `docs/version.json` with the previous version, and
   users on the broken release stop seeing the banner pointing at it.

The ZIP cached on user machines is fine, since the app does not
auto-update. They keep using whatever they installed last until they
choose to download the next release.
