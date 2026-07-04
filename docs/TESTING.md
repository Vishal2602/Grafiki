# Testing the Grafiki desktop app

Three complementary layers, from exploratory to deterministic. All three drive
the REAL app (real Rust backend, real DB resolution) — no mocked shells.

## 1 · Agent verification (exploratory, zero setup)

Any Claude Code session in this repo can drive the app like a user: launch it,
click through flows via macOS accessibility, screenshot evidence, and report
PASS/FAIL. The protocol lives in
**`.claude/skills/verifier-grafiki-desktop/`** — invoke by asking the agent to
"verify the desktop app" (the `/verify` skill discovers it automatically).

One-time human setup: grant **Screen Recording** and **Accessibility** to the
terminal/IDE hosting the agent (System Settings → Privacy & Security).

Best for: "does the whole loop still feel right", visual bugs, exploratory
QA after a feature lands. An agent found the unscrollable-pane and stale-HMR
bugs this way.

## 2 · WebdriverIO e2e suite (deterministic, CI-able)

Native macOS driving via the embedded provider (`tauri-plugin-wdio-webdriver`,
registered in DEBUG builds only — release binaries contain no automation
server).

```bash
cd apps/grafiki-desktop
npm run dev             # keep this running; debug builds load tauri.conf.json build.devUrl
npm run test:e2e        # in another shell: builds dist + debug binary, then runs the specs
npm run test:e2e:only   # specs only (binary already built)
```

Specs live in `apps/grafiki-desktop/tests/e2e/`. The smoke suite covers:
boot→Home (drives onboarding on a fresh profile with a /tmp project), all rail
destinations, the ⌘K palette → ask-memory routing, the Review keyboard-triage
legend, and the theme switch (asserts `html[data-theme]` flips and restores).

Status: **5/5 passing, ~0.5s** (deterministic across runs).

Notes:
- `npm run test:e2e:preflight` checks `http://127.0.0.1:1420/` so a missing Vite
  server fails with a one-line setup error instead of WebDriver/Tauri
  `core.invoke` timeout noise.
- `package.json` pins `@wdio/native-utils` via `overrides` — the tauri-service
  ships a stale nested copy that otherwise shadows the fixed one.
- Three Rust-side pieces make the service fully functional: the embedded
  driver plugin (`tauri-plugin-wdio-webdriver`), its companion
  `tauri-plugin-wdio` (window state/mocking — without it every command pays a
  5s sync timeout), and `app.withGlobalTauri: true` in tauri.conf.json.
- DRIVER CAVEAT: the embedded driver (v1.2) intermittently stalls native
  element-find calls for 90s+ and its select action skips React's change
  event. The specs therefore do all DOM queries/interactions via
  `browser.execute` (see the `q` helpers in smoke.spec.js) — reliable in
  every run. Revisit native finds when the driver matures.
- The suite launches its own app instance; it shares `~/.grafiki` and
  localStorage with your dev profile. Specs must stay non-destructive toward
  real memory (use /tmp projects for anything that writes).

## 3 · MCP agent bridge (element-level agent control)

For autonomous agent QA sessions with structured tools (click_element,
type_text, wait_for_element, execute_tauri_command…) instead of screen-reading:

- The app side is already wired: `tauri-plugin-webdriver-automation` runs in
  debug builds (an HTTP automation server on a random localhost port).
- The W3C driver CLI is installed: `tauri-wd` (via
  `cargo install tauri-webdriver-automation`), listens on :4444.
- The MCP server
  [mcp-tauri-automation](https://github.com/danielraffel/mcp-tauri-automation)
  is INSTALLED at `~/tools/mcp-tauri-automation` (repo reviewed: 3 deps —
  MCP SDK, webdriverio, zod; only lifecycle hook is `prepare: tsc`) and
  registered project-locally as `tauri-automation` (`claude mcp list` → ✔).

To use in an agent session:

```bash
tauri-wd --port 4444 &        # the W3C bridge (installed via cargo)
# then ask the agent to use the tauri-automation MCP tools:
# launch_app / click_element / type_text / capture_screenshot / …
```

(Early-stage project — treat as experimental alongside layers 1–2.)

Lessons from the first real QA campaign (2026-07-02, 4 runs / 24 checks):

- `type_text` keystrokes never reach xterm's hidden `.xterm-helper-textarea`
  (embedded-driver limitation). To exercise the PTY, invoke the app's own IPC
  from in-page JS instead — `withGlobalTauri` is on in debug builds:
  `window.__TAURI__.core.invoke('terminal_write', {id, data: 'echo hi\r'})`.
  The live session id is in `~/.grafiki/terminal_sessions.json` (newest
  `updated_at`). Output streams into `.xterm-rows` in <0.5 s.
- `execute_script` does not await promises; stash results on `window.__qa`
  and read them with a second call.
- The app restores its persisted pane and auto-attaches resumable sessions on
  launch, so never assume a fresh run lands on Home or an empty launcher.

## Which layer when

| Question | Layer |
|---|---|
| "Did my change break anything obvious?" | 2 (run the suite) |
| "Does the new flow feel right / look right?" | 1 (agent drive + screenshots) |
| "Let an agent regression-hunt for an hour" | 3 (MCP) or 1 |
| Unit/integration logic (core, CLI) | `cargo test` (136+ tests) + eval gates |
