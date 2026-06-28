# Grafiki — "Fully Works + Full Polish" Checklist

Tracks the remaining work after the 9 production-hardening milestones (M1–M5, CI,
Distribution) landed on branch `production-hardening`. Current state: builds, 56
tests pass, fmt + clippy `-D warnings` clean, smoke green, frontend + desktop build.

Effort key: **S** ≈ <½ day · **M** ≈ ½–2 days · **L** ≈ multi-day.

---

## A. Blocks "fully works" (functional gaps a real user hits)

- [ ] **A1 — Replace `window.prompt`/`confirm` in the desktop UI.** Tauri's webview can
  suppress them, so reject / delete / import-with-rationale silently no-op. Use the
  Tauri dialog plugin or in-app modals. `apps/grafiki-desktop/src/App.tsx` (reject ~L1871,
  and other prompt/confirm sites). **M**
- [ ] **A2 — Capture-event dedup on re-ingest.** Re-running `capture import-transcripts`
  / re-ingest duplicates everything. Add a `content_hash` column (migration **v2** — runner
  exists in `db/schema.rs`) + `INSERT OR IGNORE`; dedup transcripts/file snapshots by content,
  but still allow legitimately-repeated terminal commands (per-source-type policy). `memory.rs`
  `ingest_capture_event`. **M**
- [ ] **A3 — Offline-first embedding model.** Release builds ship `fastembed`, which downloads
  MiniLM at runtime on first use → first run offline/airgapped fails. Bundle/vendor the ONNX
  model, or show a clear "downloading model…" state + offline error. `embeddings.rs` + release
  workflow. **M**
- [ ] **A4 — Desktop daemon auth token.** The app launches the local daemon with an empty token
  (unauthenticated). Generate a random token on launch and thread it through. `lib.rs` (~L1854),
  `api.ts` (~L1070). **S**
- [ ] **A5 — Finish desktop delete/update coverage.** Some record types still return "not
  available in the desktop alpha". Wire every type the CLI/MCP can edit/delete to the UI.
  `lib.rs` delete/update commands. **S–M** (verify against current code)

## B. Full polish (UI/UX completeness)

- [ ] **B6 — Async Tauri commands.** export / screencapture / daemon control / 5k-file
  auto-capture walk run synchronously and freeze the window → `async` / `spawn_blocking`. `lib.rs`. **M**
- [ ] **B7 — Accessibility finish.** Real `role="dialog"` + aria-modal + focus-trap + Escape on
  Launcher & CommandPalette. (focus-visible, dark mode, reduced-motion already done.) `App.tsx`. **M**
- [ ] **B8 — Lifecycle hygiene.** Stop the app-started daemon on quit (`RunEvent::ExitRequested`);
  bound screenshot retention + fix whole-second filename collisions. `lib.rs` (~L1366). **S**
- [ ] **B9 — State round-trip completeness.** export/import still drops state
  `details`/`blockers`/`depends_on` (needs `StateItem` field additions that ripple into the
  status view). `memory.rs`. **M**
- [ ] **B10 — Small UX bugs.** min-confidence free-text can hide all candidates; duplicate-pane
  uses active not clicked pane; `titleForPane` crash on tampered URL hash; restrictive CSP
  (`tauri.conf.json`). `App.tsx`. **S each**

## C. Hardening leftovers (P1, not user-visible)

- [ ] **C11 — HTTP token hygiene.** Daemon passes `--token` via argv (visible in `ps`) and accepts
  `?token=`; move to env/stdin and drop the query param. `main.rs`. **S**
- [ ] **C12 — Embedding housekeeping.** Delete orphan `vec0` rows on record delete; prune stale
  vectors on provider/dimension switch; reuse the ONNX provider in the daemon worker instead of
  rebuilding it every 2s. `memory.rs` / `main.rs`. **S–M**
- [ ] **C13 — `redaction_profile`.** Stored/advertised but unused — implement none/default/strict
  or remove it from config/CLI/MCP. `memory.rs` + `project.rs`. **M**
- [ ] **C14 — Misc audit P2s.** MCP stdin size cap + protocol-version negotiation; chunked
  `Transfer-Encoding`; daemon PID-reuse race; entity `LIKE` wildcard escaping; empty/colliding
  slug ids; `add_context` can't update an existing key. **S each**

## D. External / ops (mostly you, not code)

- [ ] **D15 — Encryption at rest (SQLCipher + OS keychain).** Perms (0600/0700) done; this is the
  real thing, and must land after the migration runner (it exists). **L**
- [ ] **D16 — Apple Developer ID ($99/yr).** Only blocker to a Gatekeeper-clean DMG; release
  workflow + signing config already wired and dormant.
- [ ] **D17 — Push to a GitHub remote.** CI + release workflows only run once the branch is pushed.
- [ ] **D18 — Cross-platform.** macOS-hardcoded `screencapture`/HOME/`kill`; Windows desktop build.
  Linux CLI already works. **M**

---

## Recommended path to the finish line

**Section A (A1–A5) + B6–B8** ≈ 1.5–2 weeks → makes it feel finished and never
silently misbehave. Then C/D as ongoing hardening, plus the $99 signing whenever a
clean DMG is wanted.

**Convention:** one milestone per PR/commit, each verified (tests + `cargo fmt --check`
+ `cargo clippy --all-targets -D warnings` + `scripts/smoke.sh` + frontend/desktop build)
before committing — same as M1–Distribution.
