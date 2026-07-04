# Changelog

## Unreleased

### don-norman-design-critic UX fixes (Jul 4)
Fixed the top-3 issues and all five first-week friction hypotheses (H1–H5) from a
full-product UX critique, then fixed three real data-integrity bugs an adversarial
review found in the fix itself.

- **H1 — silent pipeline failures.** Home now names the first broken link in the
  capture→extraction chain (not initialized / consent off / no local model) instead
  of just staying quietly empty.
- **H2 — dishonest capture copy.** The session launcher no longer unconditionally
  claims "captured automatically" when consent is off; the Home live card shows
  the real reason capture is off instead of a bare `false`.
- **H3 — Review's error asymmetry.** Approve now has a 10-second Undo; a rejected
  candidate can be reopened; single reject is instant (no modal) with an opt-in
  "Reject with a note" for anyone who wants to record why.
- **H4 — scope filter confusion.** The Scope filter is a dropdown of known scopes,
  not free text; a hint appears when other-scope pendings are hidden.
- **H5 — machine internals in the edit form.** Editing a candidate is now a Title +
  Content form (label adapts per record type) with the raw JSON moved behind an
  "Advanced" disclosure; the confidence number input is gone.
- Home ledger rows show the actual memory titles a session produced instead of a
  raw "N captured events" count; the "Today" header no longer lies when the newest
  activity was yesterday or older.
- Settings' Capture Consent panel only lists sources that are actually wired
  (Git/Transcripts/Terminal/Files) — five checkboxes that wrote to config but
  gated nothing (ide/screen/browser/audio/system) are gone.
- The Review empty state has an "Extract now" action instead of being a dead end.
- The chat lens surfaces a banner when the agent is waiting on a permission
  decision — previously the last chat bubble looked like a finished turn while
  the terminal underneath was actually stuck.

**Adversarial review then found three real bugs in the undo/edit-form fixes
above, all fixed before shipping:**
- `revert_candidate_approval` (undo) could hard-delete an entity shared with a
  different, already-approved candidate's observation — entities are upserted by
  name across approvals, so undoing one candidate's entity approval cascaded away
  unrelated trusted memory. Undo now refuses when the entity has other
  observations/relations attached, rather than silently corrupting them.
- The same function had no concurrency guard (unlike `approve_candidate`, which
  was hardened for exactly this in the Jul 2 audit) and could wedge a candidate
  permanently if its trusted record had been deleted independently (e.g. via
  Browse). It now claims atomically before deleting, and a missing trusted
  record is treated as already-undone rather than a fatal error.
- The new edit form always wrote entity/state edits to a `content` key, but
  approval reads `observe`/`details` first for those types if present — so
  editing an entity or state candidate's body could be silently discarded at
  approval time. The form now resolves to whichever key approval will actually
  read, per record type — reproduced end-to-end against a real candidate and
  confirmed fixed (edited value survives approval into trusted memory).
- Known limitation (documented, not fixed): undoing an approval that superseded
  an older decision/observation does not restore the older record's prior
  status — full bitemporal restore is future work.

### Terminal & the Granola loop (Jul 1)
- Hosted terminal sessions survive tab switches (detached PTY pool with scrollback reattach) and full app relaunches (disk descriptors + replayed tail + `claude --continue`).
- Fixed all four auto-capture breaks: chunked HTTP decoding for Ollama responses, retry-safe extraction cursor (schema v5 `capture_cursors` — a failed model call no longer consumes the session), installed-model detection with a helpful missing-model error, and desktop-driven extraction of hosted-terminal output into Review.

### The redesign — "Snowy Rainforest" (Jul 1–2)
- Ground-up UX plan (`docs/UX_REDESIGN.md`) and design system (`docs/DESIGN.md`): permanent Evergreen rail in both themes, snow-paper sheet, two-tier green accent, Newsreader serif display + Inter + JetBrains Mono, rows-not-cards, arrival-only motion.
- New shell: sheet-on-frame layout with a custom evergreen titlebar; nav = Home · Sessions · Memory · Review · Settings with a live pending badge.
- Home is now the session ledger: weekly stats, live-session card with capture pulse, resume banner, pending-memories banner, day-grouped session timeline, and a floating "Ask your memory" bar.
- First-run onboarding (under 90 seconds): welcome → project folder → honest local-AI detection → straight into a captured session.
- Live sessions show a "Learned this session" side peek with inline approve/reject; sessions open from the ledger into a detail view (memories + raw event trail).
- Claude Code sessions gain a Chat lens — the live session rendered as a conversation with a composer that types into the PTY — next to the raw Terminal lens.
- ⌘K command palette: navigation, session start/resume, extraction, and ask-your-memory fallthrough.
- Menubar presence: capture dot while a session records, live review count, open/quit.
- "Continue this with Claude": any cited memory answer can hand off into a live pre-briefed session.

### Audit fixes (Jul 4)
- Hosted terminal now honors capture consent: no capture session starts unless the workspace's `terminal` source is on AND `terminal_output` is not `off`; the toolbar says exactly why capture is off, and onboarding gained an explicit consent checkbox that enables it.
- Editing a review candidate runs the same secret redaction as proposing one.
- Empty scope now means "everything" on every browse surface (capture-session detail, Memory lists, raw events, sessions, state, agent activity, and the auto-extraction read) — sub-scoped rows can no longer vanish from default views.
- Desktop state edits preserve owner/blockers/dependencies instead of silently erasing them.
- Candidate approval claims the candidate before creating the trusted record — concurrent approvals can't duplicate memory; stale claims (crash mid-approval) are retakeable after 15 minutes.
- Release sidecar keeps `fastembed,sqlite-vec` features instead of silently rebuilding without them.
- Accessibility: dark-mode muted text meets contrast, rail keyboard focus is visible again, and the ask bar / palette / appearance controls have real labels.
- Browser preview no longer crashes on Tauri APIs (tray listener and terminal pane are guarded with an honest notice).
- Dev hygiene: e2e preflight fails fast when Vite isn't running; npm audit is clean (vite 8.1.3, serialize-javascript override).

### QA-campaign fixes (Jul 2)
- Review queue no longer hides sub-scoped candidates: listing with the default empty scope now returns the whole queue (previously root-scoped only), so the Home badge, the Review pane, and `grafiki candidates list` finally agree on the pending count.
- Extraction no longer proposes agent-UI boilerplate: Claude Code trust prompts, welcome banners, and keyboard-hint footers are scrubbed from captured terminal text before the model reads it, chrome-only sessions skip the model call entirely, and a paraphrase backstop rejects extracted items that are about the boilerplate itself (e.g. "Project Trust Confirmation").

- Added evidence links for review candidates and approved memory.
- Added local agent query audit logs for `grafiki ask`.
- Added init-time import of `CLAUDE.md`, Cursor rules, Cline memory bank files, and recent git history into reviewable candidates.
- Added capture ingest redaction for obvious secrets before persistence.
- Added desktop Agent Activity pane.
- Added launch docs and open-source project hygiene.
