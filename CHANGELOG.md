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

- Added evidence links for review candidates and approved memory.
- Added local agent query audit logs for `grafiki ask`.
- Added init-time import of `CLAUDE.md`, Cursor rules, Cline memory bank files, and recent git history into reviewable candidates.
- Added capture ingest redaction for obvious secrets before persistence.
- Added desktop Agent Activity pane.
- Added launch docs and open-source project hygiene.
