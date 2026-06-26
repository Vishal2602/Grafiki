# Homebrew Cask for the Grafiki desktop app.
#
# Distribute via your own tap (recommended to start):
#   1. Create a repo named `homebrew-grafiki` and put this file at `Casks/grafiki.rb`.
#   2. Users install with:
#        brew tap <owner>/grafiki
#        brew install --cask grafiki
#
# `brew install --cask` removes the macOS quarantine flag, so the app opens
# cleanly even before Developer ID signing/notarization is set up.
#
# Per release: bump `version` and replace the `sha256` with the DMG's checksum
# (`shasum -a 256 Grafiki_<version>_aarch64.dmg`). Replace OWNER below.
cask "grafiki" do
  version "0.1.0"
  sha256 :no_check # replace with the real DMG sha256 once releases are signed

  url "https://github.com/OWNER/grafiki/releases/download/v#{version}/Grafiki_#{version}_aarch64.dmg"
  name "Grafiki"
  desc "Local-first memory layer for AI coding agents"
  homepage "https://github.com/OWNER/grafiki"

  app "Grafiki.app"

  zap trash: [
    "~/.grafiki",
  ]
end
