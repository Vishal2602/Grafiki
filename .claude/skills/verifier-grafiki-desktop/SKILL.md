---
name: verifier-grafiki-desktop
description: Drive and verify the Grafiki desktop app (Tauri, macOS) as an agent — launch, click through real flows via accessibility, screenshot evidence. Use when asked to verify/test the desktop app UI or a change to it.
---

# Grafiki desktop verifier

Battle-tested harness for driving the REAL app (native window, real Rust
backend) from an agent session. Everything here was proven in live sessions.

## Prerequisites (one-time, human)

System Settings → Privacy & Security → grant to the terminal/IDE hosting the
agent: **Screen Recording** and **Accessibility**. Without them: screenshots
come back empty / System Events throws error -1719 or -1728.

## Launch

```bash
cd apps/grafiki-desktop
npm run prepare-sidecar:debug   # once per fresh checkout
npx tauri dev                   # background task; wait for the window
until pgrep -f "target/debug/grafiki-desktop" >/dev/null; do sleep 1; done; sleep 6
```

Kill stale instances first if port 1420 is taken:
`pkill -f "target/debug/grafiki-desktop"; pkill -f "vite --host 127.0.0.1"; lsof -ti:1420 | xargs kill -9`

## Evidence capture

```bash
GEO=$(osascript .claude/skills/verifier-grafiki-desktop/scripts/window-rect.applescript)
screencapture -x -R "$GEO" shot.png   # then Read shot.png
```

Always re-read the geometry after a relaunch — the window moves.

## Driving

- **Buttons** (incl. nav rail, launcher, End session, banner actions):
  `osascript scripts/click.applescript "Button Name"` — searches the AX tree
  (`entire contents of window 1`) for a button with that exact name.
- **Typing** (into terminal or focused field):
  `osascript -e 'tell application "System Events" to tell (first application process whose name contains "grafiki") to keystroke "text"'`
  then `key code 36` for Return, `key code 53` for Escape.
- **⌘K palette**: `keystroke "k" using command down`, type to filter, Return.
  The palette is often the easiest way to navigate/act — prefer it.
- **`<select>` dropdowns**: AX class is `pop up button`. `click` it, delay
  0.5, `keystroke "OptionName"`, `key code 36`. NOTE: re-selecting the current
  value does NOT fire onChange — cycle through a different option first.
- **Divs with onClick (ledger rows)**: NOT reachable via AX press — verify
  those paths at the data layer (sqlite3 against `~/.grafiki/<project>.db`)
  or through the wdio suite instead.

## Key flows to verify (the product loop)

1. **Boot** → Home shows serif "Today", stat strip, ask bar. (Fresh machine
   boots into onboarding instead — drive: Get started → type a /tmp folder →
   Create memory here → Continue/Skip → pick an agent or Skip.)
2. **Session lifecycle**: Sessions → launcher button (e.g. Shell) → type
   `echo probe-$((6*7))` → expect `probe-42`. Switch to Memory and back —
   session must survive with scrollback. End session → launcher returns and
   `~/.grafiki/terminal_sessions.json` drops the descriptor.
3. **Relaunch resume**: quit app, relaunch → terminal shows dimmed
   "── previous session ──" replay in the same cwd.
4. **Chat lens** (claude sessions): toolbar Terminal|Chat tabs; bubbles tail
   the live transcript; composer sends to the PTY.
5. **Review**: kbd legend visible; j/k/a/r work when the pane is active.
6. **Theme**: Settings → Appearance (or ⌘K "Toggle dark mode") — sheet flips,
   evergreen rail must NOT change.
7. **Tray**: menubar glyph (three bars); "●" title only while capturing.

## Gotchas learned the hard way

- **Vite HMR can leave stale bundles** after Rust rebuilds relaunch the app —
  if behavior contradicts fresh code, send ⌘R (webview reload) first.
- Rust file edits make the dev watcher REBUILD AND RELAUNCH the app
  mid-drive; re-acquire the window and expect killed PTY sessions.
- `scrollIntoView` is banned in this app — it scrolls the window root.
- Extraction requires a running Ollama with an installed model; without it
  the "Learned" peek stays empty (that's honest, not broken).
- The dev app shares localStorage + `~/.grafiki` with the user's real data:
  DON'T approve/reject their real pending candidates; prefer /tmp projects
  for destructive flows.

## Companion suites

- Deterministic specs: `npm run test:e2e` (WebdriverIO, embedded macOS driver).
- MCP element-level bridge: see docs/TESTING.md §3 (mcp-tauri-automation).
