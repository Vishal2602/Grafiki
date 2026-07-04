# Changelog

## Unreleased

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
