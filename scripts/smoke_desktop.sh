#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_BUNDLE="$ROOT/target/debug/bundle/macos/Grafiki.app"
CLI="$ROOT/target/debug/grafiki"
BEFORE_PIDS="$(mktemp)"
AFTER_PIDS="$(mktemp)"
NEW_PIDS=""

cleanup() {
  rm -f "$BEFORE_PIDS" "$AFTER_PIDS"
  if [[ -n "$NEW_PIDS" ]]; then
    kill $NEW_PIDS >/dev/null 2>&1 || true
  fi
}
trap cleanup EXIT

echo "== desktop smoke: Rust tests =="
cargo test -p grafiki-core -p grafiki-desktop

echo "== desktop smoke: app bundle =="
INSTALL_TO_APPLICATIONS="${INSTALL_TO_APPLICATIONS:-0}" "$ROOT/scripts/build_desktop_debug.sh"

if [[ "${INSTALL_TO_APPLICATIONS:-0}" == "1" ]]; then
  APP_BUNDLE="/Applications/Grafiki.app"
fi

APP_EXEC="$APP_BUNDLE/Contents/MacOS/grafiki-desktop"
if [[ ! -x "$APP_EXEC" ]]; then
  echo "Grafiki app executable not found: $APP_EXEC" >&2
  exit 1
fi

if [[ ! -x "$CLI" ]]; then
  echo "Grafiki CLI not found: $CLI" >&2
  exit 1
fi

echo "== desktop smoke: launch =="
pgrep -x grafiki-desktop | sort > "$BEFORE_PIDS" || true
open -n "$APP_BUNDLE"
sleep "${LAUNCH_WAIT_SECONDS:-4}"
pgrep -x grafiki-desktop | sort > "$AFTER_PIDS" || true
NEW_PIDS="$(comm -13 "$BEFORE_PIDS" "$AFTER_PIDS" || true)"

if [[ -z "$NEW_PIDS" ]]; then
  echo "Grafiki did not stay running after launch." >&2
  exit 1
fi

echo "Grafiki launched with PID(s): $NEW_PIDS"

echo "== desktop smoke: daemon status =="
"$CLI" daemon status --path "$ROOT" --format json

echo "desktop smoke ok"
