# humOS Homebrew Cask

## What this is

This directory holds the source for the humOS Homebrew cask. The actual cask lives in a separate tap repo at `github.com/BoluOgunbiyi/homebrew-humos` — Homebrew discovers taps by the `homebrew-<name>` prefix, so the repo name must match exactly. The file here (`Casks/humos.rb`) is the canonical draft; copy it into the tap repo on every release.

## One-time setup

Run these once, before the first public release:

1. Create a new public GitHub repo named exactly `homebrew-humos` under your account.
2. Copy `humos.rb` into `Casks/humos.rb` in that repo.
3. Commit and push:
   ```
   git add Casks/humos.rb
   git commit -m "Add humos cask v0.4.0"
   git push
   ```
4. Test the tap locally:
   ```
   brew tap humos-dev/humos
   brew install --cask humos
   ```

## Per-release update

Every release, after the GitHub release is published:

1. Download the dmg and compute the sha256:
   ```
   shasum -a 256 humOS_0.4.0_aarch64.dmg
   ```
2. In `homebrew-humos/Casks/humos.rb`, update `version` and replace `sha256 :no_check` with the real hash.
3. Commit and push to the tap repo.
4. Verify the bump is picked up:
   ```
   brew update
   brew upgrade --cask humos
   ```

## Automation option

[`dawidd6/action-homebrew-bump-formula`](https://github.com/dawidd6/action-homebrew-bump-formula) can do steps 1–3 automatically from the humOS release workflow — it computes the sha256, rewrites the cask, and opens a PR against the tap repo. Nice-to-have once releases feel routine.
