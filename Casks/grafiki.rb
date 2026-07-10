# Homebrew Cask for the Grafiki desktop app.
#
# STATUS: TEMPLATE, NOT YET PUBLISHED. No tagged GitHub release exists yet, so
# the `url` below does not resolve to a real artifact, and `sha256 :no_check`
# is a placeholder — it disables integrity checking on the downloaded DMG and
# MUST NOT be used for a real distributed release.
#
# Before this cask is published to a live tap:
#   1. Cut a real signed/notarized release (see docs/PRODUCTION_RELEASE.md)
#      that produces `Grafiki_<version>_aarch64.dmg` under GitHub Releases.
#   2. Replace `sha256 :no_check` with the actual DMG checksum:
#        shasum -a 256 Grafiki_<version>_aarch64.dmg
#   3. Create a repo named `homebrew-grafiki` under Vishal2602 and put this
#      file at `Casks/grafiki.rb`.
#   4. Users then install with:
#        brew tap Vishal2602/grafiki
#        brew install --cask grafiki
#
# `brew install --cask` removes the macOS quarantine flag, so the app opens
# cleanly even before Developer ID signing/notarization is set up.
#
# Per release: bump `version` and replace the `sha256` with the DMG's checksum
# (`shasum -a 256 Grafiki_<version>_aarch64.dmg`).
cask "grafiki" do
  version "0.1.0"
  sha256 :no_check # PLACEHOLDER — see STATUS note above; must be a real DMG sha256 before publishing

  url "https://github.com/Vishal2602/Grafiki/releases/download/v#{version}/Grafiki_#{version}_aarch64.dmg"
  name "Grafiki"
  desc "Local-first memory layer for AI coding agents"
  homepage "https://github.com/Vishal2602/Grafiki"

  app "Grafiki.app"

  zap trash: [
    "~/.grafiki",
  ]
end
