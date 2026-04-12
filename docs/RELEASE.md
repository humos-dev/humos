# Releasing humOS

How to cut a macOS release. Automated via GitHub Actions, you tag, it builds.

The build is **unsigned** (no Apple Developer ID required). Homebrew handles
quarantine removal automatically. Direct .dmg users run `xattr -cr /Applications/humOS.app`.

## Prerequisites (one-time setup)

No Apple secrets needed. Just make sure the repo has Actions enabled (it is by default).

## Cutting a release

1. Bump the version in `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml`, and `package.json`. All three must match.
2. Update the `CHANGELOG.md` heading with the new version and date.
3. Commit: `git commit -am "release: v0.4.0"`
4. Tag and push: `git tag v0.4.0 && git push origin main --tags`
5. Open the **Actions** tab. The `Release` workflow builds and drafts a GitHub Release with the `.dmg` attached. Takes ~5-10 min.
6. Open the draft release, review the auto-generated notes, edit if needed, and click **Publish**.

## After publishing

1. Compute the sha256: `shasum -a 256 humOS_0.4.0_aarch64.dmg`
2. Update `version` and `sha256` in the Homebrew tap repo (`homebrew-humos/Casks/humos.rb`)
3. Push to the tap repo. Users can now `brew upgrade --cask humos`.

## Troubleshooting

- **"Tag already exists"** — delete the remote tag first: `git push --delete origin v0.4.0`, then re-tag and push.
- **Gatekeeper blocks the app** — this is expected for unsigned builds. Users should install via Homebrew (`brew install --cask humos`) or run `xattr -cr /Applications/humOS.app` after dragging from the .dmg.
- **Build fails on macos-14 runner** — check that `npm ci` succeeds (lock file in sync) and that `cargo build` passes locally first.
