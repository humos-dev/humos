# Releasing humOS

How to cut a macOS release. Automated via GitHub Actions, you tag, it builds.

The build is **unsigned** (no Apple Developer ID required). Homebrew handles
quarantine removal automatically via a `postflight` hook. Direct .zip users run `xattr -cr /Applications/humOS.app` after dragging to Applications.

## Prerequisites (one-time setup)

No Apple secrets needed. Just make sure the repo has Actions enabled (it is by default).

## Cutting a release

1. Bump the version in `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml`, and `package.json`. All three must match.
2. Update the `CHANGELOG.md` heading with the new version and date. Every entry **must** open with a `### How to update` block immediately after the version heading - this is what users see on the GitHub release page and via the in-app "See what's new" link:

   ```
   ### How to update

   curl -fsSL https://humos.dev/install.sh | sh

   Or download the ZIP, extract it, then run:
   xattr -cr ~/Downloads/humOS.app && open ~/Downloads/humOS.app

   macOS says "damaged" or "file can't be found"? That is Gatekeeper,
   not actual damage. The xattr -cr command above clears it.
   ```
3. Commit: `git commit -am "release: v0.4.0"`
4. Tag and push: `git tag v0.4.0 && git push origin main --tags`
5. Open the **Actions** tab. The `Release` workflow builds and drafts a GitHub Release with the `.zip` attached. Takes ~5-10 min.
6. Open the draft release, review the auto-generated notes, edit if needed, and click **Publish**.

## After publishing

1. Compute the sha256: `shasum -a 256 humOS_0.4.4_aarch64.zip`
2. Update `version` and `sha256` in the Homebrew tap repo (`homebrew-humos/Casks/humos.rb`)
3. Push to the tap repo. Users can now `brew upgrade --cask humos`.

## Troubleshooting

- **"Tag already exists"** - delete the remote tag first: `git push --delete origin v0.4.0`, then re-tag and push.
- **Gatekeeper blocks the app** - this is expected for unsigned builds. Users should install via Homebrew (`brew install --cask humos`) or run `xattr -cr /Applications/humOS.app` after unzipping and dragging to Applications.
- **Build fails on macos-14 runner** - check that `npm ci` succeeds (lock file in sync) and that `cargo build` passes locally first.
