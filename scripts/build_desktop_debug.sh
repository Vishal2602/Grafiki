#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_DIR="${ROOT_DIR}/apps/grafiki-desktop"
DMG_PATH="${ROOT_DIR}/target/debug/bundle/dmg/Grafiki_0.1.0_aarch64.dmg"
APP_PATH="${ROOT_DIR}/target/debug/bundle/macos/Grafiki.app"

cd "${ROOT_DIR}"
"${ROOT_DIR}/scripts/prepare_desktop_sidecar.sh" debug

cd "${APP_DIR}"
npm run tauri -- build --debug

cd "${ROOT_DIR}"
hdiutil verify "${DMG_PATH}"

if [[ "${INSTALL_TO_APPLICATIONS:-0}" == "1" ]]; then
  ditto "${APP_PATH}" /Applications/Grafiki.app
  echo "Installed /Applications/Grafiki.app"
fi

echo "Built ${APP_PATH}"
echo "Built ${DMG_PATH}"
