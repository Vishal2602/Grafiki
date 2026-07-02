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
npm run test:e2e        # builds dist + debug binary, then runs the specs
npm run test:e2e:only   # specs only (binary already built)
```

Specs live in `apps/grafiki-desktop/tests/e2e/`. The smoke suite covers:
boot→Home (drives onboarding on a fresh profile with a /tmp project), all rail
destinations, the ⌘K palette → ask-memory routing, the Review keyboard-triage
legend, and the theme switch (asserts `html[data-theme]` flips and restores).

Notes:
- `package.json` pins `@wdio/native-utils` via `overrides` — the tauri-service
  ships a stale nested copy that otherwise shadows the fixed one.
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
- The MCP server is
  [mcp-tauri-automation](https://github.com/danielraffel/mcp-tauri-automation):

```bash
# review the repo first — it runs on your machine
git clone https://github.com/danielraffel/mcp-tauri-automation ~/tools/mcp-tauri-automation
cd ~/tools/mcp-tauri-automation && npm install && npm run build
claude mcp add tauri-automation -- node ~/tools/mcp-tauri-automation/dist/index.js
```

Then in any Claude session: start `tauri-wd`, and the agent can launch and
drive the app through MCP tools. (Early-stage project — treat as experimental
alongside layers 1–2.)

## Which layer when

| Question | Layer |
|---|---|
| "Did my change break anything obvious?" | 2 (run the suite) |
| "Does the new flow feel right / look right?" | 1 (agent drive + screenshots) |
| "Let an agent regression-hunt for an hour" | 3 (MCP) or 1 |
| Unit/integration logic (core, CLI) | `cargo test` (136+ tests) + eval gates |
