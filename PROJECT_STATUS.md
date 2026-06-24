# Grafiki Project Status

Updated: 2026-05-31

## Current Shape

Grafiki now has a working local-first core memory loop:

- CLI project init with first-run imports, sessions, handoff, decisions, entities, observations, relations, state, context, evidence-linked candidate review with edit/bulk actions, agent memory ask, agent query audit logs, workspace capture consent config, raw automatic capture sessions/events, terminal command metadata capture, zsh shell-hook generation, workspace file-change snapshots, git working-tree summaries, events, logs, status, scoped search, stable context search ids, graph traversal, reports, analysis, export, and JSON import.
- Export formats: JSON, Markdown, wiki directory, DOT, GraphML, and self-contained HTML.
- Local HTTP API with read/write routes for the current memory loop, including session handoff, cited agent memory ask, agent query audit listing, workspace capture consent config, raw transcript/screen/IDE capture ingestion with redaction, Codex/Claude Code/Cursor transcript import, terminal command metadata capture, workspace file-change snapshots, git working-tree summaries, git working-tree auto-capture into evidence-linked reviewable candidates, full memory-record detail lookup, generic record update/delete maintenance, and candidate propose/list/edit/approve/reject/bulk review.
- Daemon start/status/stop commands for the local HTTP API.
- Optional HTTP token auth, required when explicitly binding beyond localhost.
- Stdio MCP server with an agent tool surface for session handoff, high-level `grafiki_ask` memory briefings with evidence, `grafiki_agent_activity`, raw `grafiki_capture_*` event streaming, `grafiki_capture_config`, `grafiki_capture_config_set`, `grafiki_capture_import_transcripts`, `grafiki_capture_terminal_command`, `grafiki_capture_watch_files`, `grafiki_capture_git_summary`, `grafiki_auto_capture` working-tree capture, full memory-record detail lookup, generic record update/delete maintenance, and candidate review with edit/bulk tools.
- Cargo integration tests for CLI export/import, HTTP token behavior, HTTP record/candidate maintenance, daemon lifecycle, and MCP search/record/candidate maintenance.
- Repeatable smoke script covering CLI export/import, capture consent config, transcript import, terminal/file/git capture adapters, candidate edit/approval, HTTP, HTTP capture config and transcript/terminal/file/git capture, HTTP record/candidate edit/bulk maintenance, daemon lifecycle, token checks, MCP, MCP capture config and transcript/terminal/file/git capture, MCP record/candidate maintenance, embedding provider mismatch fallbacks, and indexed-record status checks, plus a passing fastembed/sqlite-vec smoke script for the real local embedding stack.
- Client setup guide for MCP and HTTP callers.
- Semantic search plan covering search modes, embedding queue, local model choice, sqlite-vec adapter, and fallback behavior.
- Phase 3 scaffolding for search modes, keyword fallback metadata, embedding job tables, write-path job enqueueing, embedding status inspection with provider/backend/index/freshness visibility, deterministic test embeddings, persisted deterministic vectors, semantic/hybrid search over stored vectors with result scores, stronger weighted hybrid ranking, larger retrieval-quality fixtures for topic separation and coding-agent memory questions, a daemon/server embedding worker loop, a feature-gated fastembed MiniLM provider, a vector backend trait, feature-gated sqlite-vec indexing in the embedding worker/search path, and keyword fallback when semantic providers are unavailable.
- Desktop app direction documented in `docs/DESKTOP_APP_PLAN.md`: sharp Macro-inspired memory console, command palette, launcher, left rail, inspector, and URL-synced multi-pane layout while staying focused on AI project memory.
- First Tauri desktop alpha in `apps/grafiki-desktop`, with React/Vite frontend, Tauri v2 shell, Rust commands bridged to `grafiki-core`, a working URL-synced multi-pane app frame, motion polish, native project folder picker, project init, session start/end controls with real session history, specific active-session completion, direct active-session handoff, handoff review with parent/child/session context actions, editable session type/status/goal/scope/summary/accomplishments/remaining/files, visible session parent/child/handoff metadata, native local-daemon status/start/stop controls, Capture Consent settings for source toggles/blocked paths/policies, real memory capture commands, automatic capture start/stop/status controls, transcript import into the review queue, macOS screen snapshot capture into the raw capture ledger, scoped search with keyword/semantic/hybrid modes, record-type filters, URL-persisted retrieval state, embedding freshness visibility, search-local process/rebuild controls, a graph pane backed by the core relationship traversal, a Memory Review pane grouped by source/day with keyboard focus flow, noisy-candidate selection, candidate edit/approve/reject/bulk promotion into trusted memory, and evidence-chip previews, an Agent Activity pane for local `grafiki ask` audit logs, a focused Relations pane for browsing/filtering/removing graph links, real decisions/context/state list panes, inline edit and safe delete actions for context/state/decisions plus detail-view maintenance for decisions/entities/observations/relations/sessions, JSON import/export, real memory-detail loading for inspector/detail panes, a custom Grafiki app icon, and debug macOS `.app`/`.dmg` bundles that launch successfully.
- Repeatable desktop sidecar preparation script at `scripts/prepare_desktop_sidecar.sh` plus debug build script at `scripts/build_desktop_debug.sh`, which refresh the bundled `grafiki` CLI used by desktop daemon controls and can optionally install `/Applications/Grafiki.app` via `INSTALL_TO_APPLICATIONS=1`.
- Repeatable desktop smoke script at `scripts/smoke_desktop.sh`, covering Rust tests, frontend/Tauri bundle creation, macOS launch, and CLI daemon-status verification.
- Production release notes in `docs/PRODUCTION_RELEASE.md`, separating the verified local debug build from the still-external Apple signing/notarization work.

## Rough Completion

- Usable local alpha: about 90-95% complete.
- Full original Grafiki vision: about 45-50% complete.

## Remaining Major Work

- Phase 2 hardening follow-up: more negative-path tests, clearer daemon troubleshooting, and broader client setup examples for specific agent products.
- Desktop app foundation: add broader desktop tests, richer handoff review, sharper onboarding, and polish signed release packaging.
- Semantic search: deeper relevance tuning with larger real-world corpora after the first coding-memory eval fixtures.
- Rich graph intelligence: communities, stronger analysis, surprising connections, and better graph scoring.
- TUI and hooks: terminal dashboard plus one-command hook install/uninstall for Codex, Claude Code, Cursor, Windsurf, and related tool integrations.
- Connectors/adapters: IDE-native file watchers, richer git diff/commit evidence, GitHub, Linear, Slack, Gmail, Calendar, richer transcript adapter fixture coverage, source adapters, and stronger redaction controls.
- Packaging: Apple Developer ID signing, notarization, release binaries, and user-facing docs.

## Current Next Step

Plan the next product layer: desktop reshape/onboarding, agent setup UX, and capture UX for Claude, Codex, and Cursor before broader launch packaging.
