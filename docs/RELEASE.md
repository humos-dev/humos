# Releasing humOS

How to cut a signed, notarized macOS build of humOS. Automated via GitHub Actions — you tag, it builds.

## Prerequisites (one-time setup)

Before the first release, you need:

1. **Apple Developer Program membership** — $99/year, [developer.apple.com](https://developer.apple.com).
2. **Developer ID Application certificate** — create in Xcode or the Apple Developer portal, then export from Keychain Access as a `.p12` with a password.
3. **App-specific password** — generate at [appleid.apple.com](https://appleid.apple.com) → Sign-In and Security → App-Specific Passwords. Used for notarization.

Then add these **6 secrets** to the repo at `Settings → Secrets and variables → Actions`:

| Secret | Value |
|---|---|
| `APPLE_CERTIFICATE` | Your `.p12` file, base64-encoded (`base64 -i cert.p12 \| pbcopy`) |
| `APPLE_CERTIFICATE_PASSWORD` | The password you set when exporting the `.p12` |
| `APPLE_SIGNING_IDENTITY` | Full cert name, e.g. `Developer ID Application: Bolu Ogunbiyi (ABC123XYZ)` |
| `APPLE_ID` | Your Apple ID email |
| `APPLE_PASSWORD` | The app-specific password (not your Apple ID password) |
| `APPLE_TEAM_ID` | 10-char team ID from the Apple Developer portal |

## Cutting a release

1. Bump the version in `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml`, and `package.json` — all three must match.
2. Update the `CHANGELOG.md` heading with the new version and date.
3. Commit: `git commit -am "release: v0.4.0"`
4. Tag and push: `git tag v0.4.0 && git push origin main --tags`
5. Open the **Actions** tab. The `Release` workflow builds, signs, notarizes, and drafts a GitHub Release with the `.dmg` attached. Takes ~10–15 min.
6. Open the draft release, review the auto-generated notes, edit if needed, and click **Publish**.

## Troubleshooting

- **"Developer ID not found"** — `APPLE_SIGNING_IDENTITY` must match the cert name exactly, including the team ID in parentheses. Open Keychain Access and copy it verbatim.
- **"Notarization failed"** — your app-specific password probably rotated. Regenerate at appleid.apple.com and update `APPLE_PASSWORD`.
- **"Tag already exists"** — delete the remote tag first: `git push --delete origin v0.4.0`, then re-tag and push.
