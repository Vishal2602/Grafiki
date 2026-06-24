# Grafiki Product Requirements Document

## 1. Product Summary

Grafiki is a local-first knowledge memory layer for AI-assisted software development. It gives multiple AI coding sessions a shared, persistent understanding of a project: what decisions were made, what work is active, what entities matter, what changed recently, and what context should be handed to the next session.

Grafiki is not a generic notes app. Its core job is to make AI coding sessions start smarter, stay coordinated, and end with useful handoff context.

## 2. Problem

Developers increasingly run several AI coding tools across terminals, IDEs, browsers, and agents. These sessions do not share state. Each session must rediscover architecture, conventions, decisions, and in-progress work. When a context window fills or a session ends, project knowledge is often lost or copied manually into fragile notes.

The result is repeated investigation, inconsistent decisions, noisy handoffs, and lower trust in parallel AI work.

## 3. Target Users

- Solo developers using multiple AI coding tools on one project.
- Engineering leads coordinating parallel AI-assisted implementation sessions.
- Small teams that want local, inspectable, project memory without committing to a cloud knowledge platform.
- Power users of MCP-compatible clients such as Claude Code, Cursor, Windsurf, Cline, Copilot, Codex, and Aider.

## 4. Product Principles

- Local-first: one project database stored locally in SQLite.
- Explicit by default: developer-approved knowledge is trusted more than automatically extracted knowledge.
- Fast path first: session start, search, decision logging, and handoff must be low friction.
- Cross-tool: CLI, MCP, HTTP, and future connectors all write to the same store.
- Provenance-aware: every record should retain enough source context to explain where it came from.
- Modular: advanced systems such as embeddings, graph analysis, TUI, and connectors are optional layers around a small reliable core.

## 5. Goals

- Give every AI coding session a concise briefing at startup.
- Persist decisions, observations, state, context documents, and handoffs across sessions.
- Support scoped project memory so backend work does not pull in unrelated frontend details unless relevant.
- Provide both human CLI workflows and MCP tools for AI agents.
- Let coding agents ask Grafiki for a compact project-memory briefing before or during implementation.
- Let coding agents auto-capture a coding session from repository state into reviewable memory candidates.
- Let coding agents and the desktop shell stream transcript, screen, IDE, file, terminal, git, and agent events into a raw local capture ledger before summarization.
- Preserve evidence links from raw/imported sources through candidate review into trusted memory.
- Log local agent query activity so developers can inspect what agents asked and what memory was returned.
- Make project knowledge searchable by exact terms first, and later by semantic similarity.
- Keep all data local and exportable.

## 6. Non-Goals

- Cloud-hosted collaboration in the initial product.
- Fully automatic truth extraction without review.
- Replacing issue trackers, documentation systems, or source control.
- Large language model dependency for the core workflow.
- Multi-machine sync in the first implementation.

## 7. Core User Journeys

### 7.1 Start A Session

As a developer starting an AI coding session, I run `grafiki start --type codex --goal "..." --scope project/backend`. Grafiki creates a session, resolves the scope chain, and returns a concise briefing with relevant decisions, active state, recent changes, and known gotchas.

Acceptance criteria:
- A session ID is created using a ULID.
- Briefing includes only global and scope-chain knowledge.
- Output is available as plain text, Markdown, and JSON.
- If a briefing cache is fresh, Grafiki can return it without regenerating.

### 7.2 Save Knowledge

As a developer or agent, I can save an entity, add observations, and relate entities so future sessions do not rediscover the same facts.

Acceptance criteria:
- Entities have stable human-readable IDs.
- Observations carry category, confidence, source, scope, and validity.
- Relations carry relation type, confidence, source type, source, metadata, and validity.
- All writes create append-only events.

### 7.3 Log Decisions

As a developer, I can record a decision with reasoning, alternatives, tags, scope, and supersession information.

Acceptance criteria:
- Decisions can be active, superseded, revisit, or revoked.
- A decision can supersede another decision.
- Decision history remains queryable.
- Decision writes create events and affect future briefings.

### 7.4 Search Project Memory

As a developer or agent, I can search observations, decisions, context, entities, or all records.

Acceptance criteria:
- Phase 1 supports keyword search using SQLite FTS5.
- Phase 3 adds optional semantic search with persisted vectors and sqlite-vec indexing.
- Hybrid search merges keyword and semantic results with reciprocal rank fusion.
- If embeddings are unavailable, search falls back to keyword mode.

### 7.5 End Or Handoff A Session

As a developer ending a session, I can record summary, accomplishments, remaining work, files changed, decisions made, and handoff context.

Acceptance criteria:
- Ended sessions are marked completed, abandoned, or handed-off.
- Handoff can create a child session linked to its parent.
- Handoff output includes explicit next steps and relevant context.
- Session end and handoff create events.

## 8. Functional Requirements

### 8.1 Storage

- Store project data in `~/.grafiki/<project>.db`.
- Use SQLite WAL mode with foreign keys enabled.
- Use embedded migrations with schema version tracking.
- Support project detection through `--project`, `.grafiki`, Git remote, then directory name.

### 8.2 Data Model

Core records:
- Entities
- Observations
- Relations
- Decisions
- Sessions
- State items
- Context documents
- Events
- Briefing cache

Schema correction from the technical spec:
- Relations must include `confidence`, `source_type`, and `source` because graph analysis and visualization depend on them.
- Connector-generated data must be represented as reviewable candidates before it becomes trusted graph knowledge.
- Automatically imported/captured data must retain evidence links and stay reviewable before it becomes trusted memory.

### 8.3 Interfaces

Phase 1:
- CLI for init, start, end, handoff, status, decide, search, save, context, log, events.

Phase 2:
- MCP server using stdio.
- HTTP API on localhost for daemon and programmatic access.
- High-level agent ask endpoint/tool that combines scoped status, relevant memory search, and capture guidance.

Important architecture rule:
- `grafiki serve` runs the HTTP API, background jobs, and connectors.
- `grafiki mcp --project <project>` runs stdio MCP for clients.
- MCP can talk to the daemon over HTTP when available, otherwise use direct SQLite.

Desktop direction:
- Grafiki Desktop should be a sharp, Macro-inspired memory console, not a generic notes app.
- The desktop app opens directly into the working project memory console.
- Core desktop navigation includes Overview, Search, Graph, Memory Review, Relations, Sessions, Decisions, Context, and Settings.
- The interface must be keyboard-first, command-palette-first, and entity-first.
- Multi-pane layout is a first-class requirement. Users can open search, graph, relations, sessions, decisions, context, and detail views side by side.
- Pane state should be URL-synced so layouts can be restored, shared, bookmarked, and debugged.
- Session records should be maintainable from the desktop app, including correcting type/status/goal/scope/summary and completing a specific active session.
- The first desktop architecture should use Tauri commands over `grafiki-core` for local reads/writes, while keeping HTTP and MCP as separate agent-facing interfaces.
- Detailed desktop requirements live in `docs/DESKTOP_APP_PLAN.md`.
- The launch desktop surfaces should converge on Today, Memory Review, and Agent Activity so Grafiki feels like a coding-memory product rather than a generic notes workspace.

### 8.4 Scope Resolution

Scopes are slash-delimited paths. A session scoped to `open-insurance/backend/enrichment` can see:

- global scope: `""`
- project scope: `open-insurance`
- parent scope: `open-insurance/backend`
- current scope: `open-insurance/backend/enrichment`

Acceptance criteria:
- Scope resolution is deterministic.
- Invalid scopes are rejected.
- Deeper scopes always include all ancestor scopes.

### 8.5 Briefing

The default briefing mode uses SQL templates, not an LLM.

Briefing inputs:
- Active decisions in scope chain
- In-progress, blocked, and needs-review state items
- Recent completed sessions
- Recent observations
- Blockers
- Events since last briefing cursor

Acceptance criteria:
- Briefing target is under 2,000 tokens.
- Old or low-confidence observations are truncated first.
- Briefing cache invalidates when overlapping scoped events are written.

### 8.6 Connectors

Connectors are optional background workers for Slack, Gmail, Calendar, Granola, Linear, and GitHub.

Requirement:
- Connectors must extract candidate knowledge first.
- Candidates can be approved, edited, rejected, or auto-approved by explicit policy.
- Raw event retention and redaction are configurable.

Additional connector tables:
- `connector_cursors`
- `raw_connector_events`
- `extraction_candidates`
- `extraction_reviews`

## 9. Nonfunctional Requirements

- Startup commands should feel instant for normal project databases.
- Briefing generation target: less than 50ms for 1,000 observations in template mode.
- Keyword search target: less than 5ms for common queries.
- Hybrid search target after embeddings: less than 30ms.
- Core operations must work offline.
- Data must be exportable as JSON, SQLite, Markdown, wiki, GraphML, DOT, and HTML in later phases.
- CLI errors must be human-readable and return non-zero status codes.
- HTTP errors must return structured JSON error bodies.
- MCP errors must use JSON-RPC error responses.

## 10. Phased Delivery

### Phase 1: Core Memory Loop

Deliver:
- Rust workspace
- SQLite schema and migrations
- Project detection
- CLI init/start/end/handoff/status
- Entities, observations, relations, decisions, sessions, state, context, events
- Scope resolution
- Template briefing
- FTS5 keyword search
- JSON/plain/Markdown output

Exit criteria:
- A developer can initialize a project, start a session, save knowledge, search it, record decisions, and generate a handoff.

### Phase 2: Agent Interface

Deliver:
- MCP stdio server
- HTTP API
- Daemon mode
- CLI-to-daemon fallback behavior
- Health and stats endpoints

Exit criteria:
- An MCP client can retrieve briefings, save observations, log decisions, search, update state, and end a session.

### Phase 3: Semantic Search

Deliver:
- Embedding queue
- Local embeddings with fastembed and all-MiniLM-L6-v2
- sqlite-vec tables
- Hybrid search with reciprocal rank fusion
- Embedding rebuild/status commands

Exit criteria:
- Conceptual searches retrieve useful observations even without exact terms.

### Phase 4: Graph Intelligence

Deliver:
- Graph traversal
- Analysis pipeline
- God nodes
- Communities
- Surprising connections
- Orphans
- Suggested queries
- Markdown reports

Exit criteria:
- `grafiki analyze` produces a useful project report from existing graph data.

### Phase 5: Visualization And Export

Deliver:
- vis.js HTML graph
- Wiki export
- GraphML export
- DOT export

Exit criteria:
- A developer can inspect and share project knowledge without running Grafiki.

### Phase 6: Desktop Memory Console

Deliver:
- Tauri desktop shell
- Macro-inspired app frame with left rail, top status strip, main pane, and inspector
- Command palette and launcher
- URL-synced multi-pane layout
- Overview, search, graph, relations, sessions, decisions, context, and settings views
- Tauri command bridge to `grafiki-core`
- Daemon and embedding status visibility

Exit criteria:
- A developer can use Grafiki's core memory loop visually, search and inspect memory across panes, and start or end sessions without using the terminal for routine workflows.

### Phase 7: TUI And Hooks

Deliver:
- Ratatui dashboard
- Claude Code, Cursor, Windsurf, and Codex hook integrations

Exit criteria:
- Users can monitor sessions and nudge AI tools to check Grafiki before broad file searches.

### Phase 8: Connectors

Deliver:
- Shared connector trait and extraction pipeline
- Slack, Gmail, Calendar, Granola, Linear, GitHub connector crates
- Candidate review workflow
- Privacy and redaction controls

Exit criteria:
- External tools can enrich Grafiki without silently polluting trusted memory.

## 11. Success Metrics

- Time to useful first briefing: under 5 minutes after install.
- Session handoff quality: a new session can continue work without rereading raw chat history.
- Search reuse: developers search Grafiki before broad repository search for project context questions.
- Decision recall: active and superseded decisions are easily retrievable.
- Trust: users can explain where important observations came from.

## 12. Key Risks

- Scope creep from connectors and visualization before the core memory loop is excellent.
- Graph noise from automatic extraction.
- MCP and daemon lifecycle confusion.
- Embeddings increasing installation friction.
- Schema changes becoming painful after early adoption.

Mitigations:
- Keep Phase 1 small and durable.
- Treat connector output as candidates.
- Separate `serve` and `mcp` responsibilities.
- Make embeddings optional.
- Use migrations from day one.

## 13. First Build Milestone

The first milestone is not the full system. It is a compiled Rust workspace with tested core utilities, followed by the Phase 1 schema and CLI.

Current implementation status:
- Done: Rust workspace, `grafiki-core`, and `grafiki-cli`.
- Done: scope validation, ULID helper, SQLite schema, `.grafiki` project detection, and `grafiki init`.
- Done: session start/end/handoff, decisions, entities, observations, relations, state, context, evidence-linked candidate review with edit/bulk actions, agent memory ask/audit, events, logs, status, search, graph traversal, reports, analysis, export, JSON import, a localhost HTTP API with daemon lifecycle and optional token auth, a first MCP stdio tool surface, and a Tauri desktop alpha with URL-synced panes plus review/agent-activity surfaces.
- Done: export formats for JSON, Markdown, wiki directory, DOT, GraphML, and self-contained HTML.
- Done: client setup examples, shell smoke coverage, and first Cargo integration tests for CLI/HTTP/daemon/MCP.
- Next: build the Tauri desktop memory console foundation with Macro-inspired sharpness, command palette, launcher, and URL-synced multi-pane layout.
- Later: automatic session hooks, richer graph analysis, packaging, AI-tool integrations, and external connectors.
