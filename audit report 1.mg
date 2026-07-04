I audited the full repo: Rust core/CLI/eval crates, Tauri desktop backend, React app, scripts, release config, tests, and dependency checks. I did not change files; working tree is clean.
Findings
High: Hosted terminal captures output even though default capture config says terminal output is off. The default is terminal_output: "off" in [project.rs (line 146)](/Users/vishalsunilkumar/Documents/Project/Grafiki/crates/grafiki-core/src/project.rs:146), but the desktop terminal starts a capture session unconditionally in [terminal.rs (line 423)](/Users/vishalsunilkumar/Documents/Project/Grafiki/apps/grafiki-desktop/src-tauri/src/terminal.rs:423) and flushes terminal output into capture events in [terminal.rs (line 661)](/Users/vishalsunilkumar/Documents/Project/Grafiki/apps/grafiki-desktop/src-tauri/src/terminal.rs:661). Redaction still runs downstream, but the user’s output-capture preference is bypassed.

High: Editing extraction candidates can persist secrets without candidate redaction. edit_candidate accepts replacement JSON/rationale and writes them directly in [memory.rs (line 3996)](/Users/vishalsunilkumar/Documents/Project/Grafiki/crates/grafiki-core/src/memory.rs:3996). The desktop review UI sends edited payload/rationale from [App.tsx (line 2359)](/Users/vishalsunilkumar/Documents/Project/Grafiki/apps/grafiki-desktop/src/App.tsx:2359). The create/propose path has redaction tests, but this edit path needs the same redaction pass.

Medium: Desktop state edits erase owner, blockers, and dependencies. The frontend update type has no owner/blocker/dependency fields in [types.ts (line 497)](/Users/vishalsunilkumar/Documents/Project/Grafiki/apps/grafiki-desktop/src/types.ts:497), and the Tauri update path calls upsert_state with owner: None, empty blockers, and empty depends_on in [lib.rs (line 1765)](/Users/vishalsunilkumar/Documents/Project/Grafiki/apps/grafiki-desktop/src-tauri/src/lib.rs:1765). Saving a state record can silently discard existing structured state.

Medium: Candidate approval is non-atomic and can duplicate trusted records. approve_candidate checks pending status, drops the DB connection, creates the trusted record, then later marks the candidate approved in [memory.rs (line 3944)](/Users/vishalsunilkumar/Documents/Project/Grafiki/crates/grafiki-core/src/memory.rs:3944). Concurrent approvals, or a crash between creation and status update, can create duplicate decisions/observations.

Medium: Capture session detail can hide scoped raw events. capture_session_detail counts all events by session, then fetches events through list_capture_events with scope: "" in [memory.rs (line 4693)](/Users/vishalsunilkumar/Documents/Project/Grafiki/crates/grafiki-core/src/memory.rs:4693). Since empty scope only chains to global scope in [scope.rs (line 27)](/Users/vishalsunilkumar/Documents/Project/Grafiki/crates/grafiki-core/src/scope.rs:27), scoped events can disappear from the detail view.

Medium: Desktop release sidecar likely loses embedding features. The release workflow first builds CLI with fastembed,sqlite-vec in [.github/workflows/release.yml (line 57)](/Users/vishalsunilkumar/Documents/Project/Grafiki/.github/workflows/release.yml:57), then calls the sidecar script, which rebuilds release CLI without those features in [prepare_desktop_sidecar.sh (line 15)](/Users/vishalsunilkumar/Documents/Project/Grafiki/scripts/prepare_desktop_sidecar.sh:15). The copied desktop sidecar may not match the intended feature set.

Medium: npm audit --audit-level=high fails. Current advisories include serialize-javascript <=7.0.4 through WebdriverIO/Mocha and vite 8.0.0 - 8.0.15. Mostly dev/e2e/dev-server exposure, but still a real audit failure.

Low: CI hygiene issues. cargo fmt --all --check fails in crates/grafiki-core/src/extract.rs and crates/grafiki-core/src/memory.rs. Vite also warns the main JS chunk is large: 762.84 kB minified.



I ran an end-to-end style test pass across backend, desktop build, web-rendered UI, responsive layout, keyboard/a11y, and packaging. I did not edit source files; git status is clean.
Test Matrix
Passed: cargo test --workspace
Passed: cargo clippy --workspace --all-targets -- -D warnings
Passed: cargo test -p grafiki-core --features code-index code_index
Passed: npm run build, but with a large chunk warning: index-*.js is 762.84 kB
Passed: bash scripts/smoke.sh
Failed: cargo fmt --all --check
Failed: npm audit --audit-level=high
Failed: npm run test:e2e
Failed partially: npm run tauri:build:debug builds the app binary and .app, then fails DMG bundling
Major Problems
Desktop e2e is broken. After installing missing tauri-driver, all 5 WebdriverIO specs failed: Home, rail navigation, command palette, Review, and Settings. The suite repeatedly logged Tauri core.invoke not available after 5s timeout, then failed with neither onboarding nor the app shell appeared. Relevant setup is in [package.json (line 15)](/Users/vishalsunilkumar/Documents/Project/Grafiki/apps/grafiki-desktop/package.json:15) and [wdio.conf.js (line 18)](/Users/vishalsunilkumar/Documents/Project/Grafiki/apps/grafiki-desktop/wdio.conf.js:18).

Browser/preview mode throws repeated Tauri API errors. The app calls Tauri listen(...) unguarded in [App.tsx (line 254)](/Users/vishalsunilkumar/Documents/Project/Grafiki/apps/grafiki-desktop/src/App.tsx:254), causing Cannot read properties of undefined (reading 'transformCallback') outside Tauri. Terminal preview is worse: new Channel() in [App.tsx (line 1461)](/Users/vishalsunilkumar/Documents/Project/Grafiki/apps/grafiki-desktop/src/App.tsx:1461) crashes the UI to a blank screen.

Mobile UI is not usable. At 375px and 320px, the fixed 208px rail from [styles.css (line 146)](/Users/vishalsunilkumar/Documents/Project/Grafiki/apps/grafiki-desktop/src/styles.css:146) leaves the workspace squeezed and horizontally overflowing. Stat cards, status pills, preview banner, and ask bar spill off-screen.

Accessibility issues. Axe found serious color contrast failures in dark mode for stat-card labels using --ink-3 from [styles.css (line 59)](/Users/vishalsunilkumar/Documents/Project/Grafiki/apps/grafiki-desktop/src/styles.css:59). Keyboard focus is also weakened because rail/brand focus outlines are explicitly removed in [styles.css (line 94)](/Users/vishalsunilkumar/Documents/Project/Grafiki/apps/grafiki-desktop/src/styles.css:94). Several controls rely on placeholders or nearby text instead of real labels: ask inputs in [App.tsx (line 1282)](/Users/vishalsunilkumar/Documents/Project/Grafiki/apps/grafiki-desktop/src/App.tsx:1282), command palette input in [App.tsx (line 893)](/Users/vishalsunilkumar/Documents/Project/Grafiki/apps/grafiki-desktop/src/App.tsx:893), and the Appearance select in [App.tsx (line 2972)](/Users/vishalsunilkumar/Documents/Project/Grafiki/apps/grafiki-desktop/src/App.tsx:2972).

Debug packaging fails at DMG creation. npm run tauri:build:debug builds target/debug/grafiki-desktop and target/debug/bundle/macos/Grafiki.app, but fails bundling Grafiki_0.1.0_aarch64.dmg with bundle_dmg.sh.

Security audit fails. npm audit reports high-severity advisories in serialize-javascript via WebdriverIO/Mocha and vite 8.0.0 - 8.0.15.

Formatting check fails. Rustfmt wants changes in [extract.rs (line 240)](/Users/vishalsunilkumar/Documents/Project/Grafiki/crates/grafiki-core/src/extract.rs:240) and [memory.rs (line 3915)](/Users/vishalsunilkumar/Documents/Project/Grafiki/crates/grafiki-core/src/memory.rs:3915).

UI Flows Tested
Onboarding, Home, Sessions, Memory, Review, Settings, command palette, theme switch, and browser preview all rendered. Onboarding completes in browser preview with mock data. Review and Settings are functional on desktop width. Command palette can route a question to Memory chat. Terminal launch from preview crashes to a blank page.
Screenshots and raw UI/a11y reports are in /tmp/grafiki-ui-audit/, especially 20-mobile.png, 30-terminal-preview.png, and ui-flow-a11y-report.json