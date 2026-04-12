# humos.rb — Homebrew cask for humOS
#
# This file is the source of truth for the cask, but it is consumed from a
# separate tap repo: github.com/BoluOgunbiyi/homebrew-humos
# Copy this file to Casks/humos.rb in that repo on each release.
#
# Users install via:
#   brew tap BoluOgunbiyi/humos
#   brew install --cask humos
#
# NOTE: Apple Silicon (aarch64) is the primary target for v0.4.0.
# Intel (x64) support will land when GitHub Actions builds both architectures;
# at that point, split this into `on_arm` / `on_intel` blocks with separate
# urls and sha256 values.

cask "humos" do
  # BUMP `version` every release. Update `sha256` at the same time —
  # compute with: shasum -a 256 humOS_<version>_aarch64.dmg
  version "0.4.0"
  sha256 :no_check

  url "https://github.com/BoluOgunbiyi/humos/releases/download/v#{version}/humOS_#{version}_aarch64.dmg"
  name "humOS"
  desc "Unix primitives for AI agent coordination"
  homepage "https://humos.dev"

  livecheck do
    url :url
    strategy :github_latest
  end

  depends_on macos: ">= :ventura"

  app "humOS.app"

  zap trash: [
    "~/.humOS",
    "~/Library/Application Support/dev.humos.app",
    "~/Library/Caches/dev.humos.app",
    "~/Library/Preferences/dev.humos.app.plist",
  ]
end
