# Grafiki — Complete Technical Specification

**Version:** 1.0
**Language:** Rust
**License:** MIT

---

## 1. What This Is

A local-first persistent knowledge store that acts as a shared knowledge store across multiple parallel AI coding sessions. It bridges the context gap between Claude Code, claude.ai, co-work sessions, Cursor, Copilot, Windsurf, Cline, Codex, Aider, and any MCP-compatible tool.

Every AI coding session reads from and writes to the same store. When a session ends or a context window fills up, knowledge persists and transfers cleanly to the next session.

---

## 2. Core Problem

Developers running parallel AI coding sessions have:

- No shared state between sessions
- No way to hand off context when a window fills up
- No persistent record of decisions made across sessions
- No awareness of what other sessions changed
- No big picture that every session understands

---

## 3. Design Principles

1. **Local-first.** Single SQLite file. No cloud, no Docker, no external databases.
2. **Developer-controlled.** The developer decides what to save. Explicit over automatic.
3. **Zero friction.** Single static binary, one command install.
4. **Cross-tool.** Works with any MCP client, any REST client, any terminal.
5. **Session-aware.** Understands AI coding sessions have distinct lifecycles.
6. **Correct by construction.** Rust's type system enforces integrity at compile time.
7. **Dual search.** Both keyword (FTS5) and semantic (sqlite-vec) search. Use the right tool for each query.

---

## 4. Architecture

```
Developer
    |
    +-- CLI (grafiki start / end / handoff / status / decide / search / save / context / log / graph)
    |
    +-- MCP Server (stdio via rmcp, for Claude Code / Cursor / Windsurf / Cline / Copilot / Codex)
    |
    +-- HTTP API (localhost:9700, JSON, for programmatic access and CLI-to-daemon IPC)
    |
    +-- TUI (interactive terminal dashboard via ratatui)
    |
    v
SQLite Database (~/.grafiki/<project>.db)
    |
    +-- knowledge graph (entities, observations, relations)
    +-- decisions log
    +-- sessions log
    +-- state tracker
    +-- context store
    +-- FTS5 keyword search indices
    +-- sqlite-vec semantic search indices
    +-- change events (watcher/notification layer)
```

All four interfaces read and write to the same SQLite file through a shared connection pool (r2d2). The `grafiki serve` command starts a single daemon hosting both the HTTP API and MCP stdio server. The CLI communicates via HTTP when the daemon is running, falls back to direct SQLite when it's not.

---

## 5. Crate Dependencies

```toml
[dependencies]
# Core
rusqlite = { version = "0.32", features = ["bundled", "vtab"] }
sqlite-vec = "0.1"
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
toml = "0.8"
ulid = "1"
chrono = { version = "0.4", features = ["serde"] }
r2d2 = "0.8"
r2d2_sqlite = "0.25"
zerocopy = { version = "0.8", features = ["derive"] }

# CLI
clap = { version = "4", features = ["derive", "env"] }
clap_complete = "4"

# MCP Server (official SDK)
rmcp = { version = "0.16", features = ["server", "transport-io", "macros"] }

# HTTP API
axum = "0.8"
tower = "0.5"
tower-http = { version = "0.6", features = ["cors", "trace"] }

# TUI
ratatui = "0.29"
crossterm = "0.28"

# Embedding generation (local)
candle-core = "0.8"
candle-nn = "0.8"
candle-transformers = "0.8"
hf-hub = "0.3"
tokenizers = "0.20"

# Utilities
thiserror = "2"
anyhow = "1"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
directories = "5"
arboard = "3"
colored = "2"
tabled = "0.17"
sha2 = "0.10"
reqwest = { version = "0.12", features = ["json"], optional = true }
petgraph = "0.6"                         # graph data structures, algorithms
open = "5"                               # open URLs/files in default browser

[dev-dependencies]
proptest = "1"
tempfile = "3"
assert_cmd = "2"
predicates = "3"

[features]
default = ["local-embeddings"]
local-embeddings = []
remote-embeddings = ["reqwest"]
```

---

## 6. ID Strategy

All record IDs use **ULIDs** (Universally Unique Lexicographically Sortable Identifiers) instead of UUIDs.

Why: ULIDs are sortable by creation time, globally unique, and encode as 26-character strings. This means any query ordered by ID is automatically ordered chronologically. ULIDs are also CRDT-friendly, making future multi-machine sync possible without schema changes.

Entity IDs remain human-readable slugs (e.g., "farouk", "sov-editor") for usability. Session IDs, observation IDs, and internal references use ULIDs.

---

## 7. Database Schema

Single SQLite file at `~/.grafiki/<project>.db`.

### 7.0 Pragmas (set on every connection open)

```sql
PRAGMA journal_mode = WAL;
PRAGMA busy_timeout = 5000;
PRAGMA foreign_keys = ON;
PRAGMA synchronous = NORMAL;
PRAGMA cache_size = -64000;        -- 64MB cache
PRAGMA temp_store = MEMORY;
```

### 7.1 Schema Version

```sql
CREATE TABLE schema_version (
    version     INTEGER PRIMARY KEY,
    applied_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    description TEXT NOT NULL
);
```

Migrations are embedded in the binary. On startup, checks `schema_version` and applies pending migrations sequentially in a transaction.

### 7.2 Entities

```sql
CREATE TABLE entities (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    entity_type TEXT NOT NULL
        CHECK (entity_type IN (
            'person', 'service', 'file', 'module', 'concept',
            'api', 'tool', 'library', 'config', 'endpoint'
        )),
    scope       TEXT NOT NULL DEFAULT '',
    metadata    TEXT,                                        -- JSON
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
) STRICT;

CREATE INDEX idx_entities_type ON entities(entity_type);
CREATE INDEX idx_entities_scope ON entities(scope);
```

### 7.3 Observations

```sql
CREATE TABLE observations (
    id          TEXT PRIMARY KEY,                            -- ULID
    entity_id   TEXT NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    content     TEXT NOT NULL,
    category    TEXT NOT NULL DEFAULT 'general'
        CHECK (category IN (
            'general', 'architecture', 'decision', 'blocker',
            'pattern', 'progress', 'gotcha', 'learned',
            'preference', 'convention', 'dependency', 'risk'
        )),
    source      TEXT,                                        -- "session:<ulid>" or "manual" or "auto:<tool>"
    confidence  REAL NOT NULL DEFAULT 1.0
        CHECK (confidence >= 0.0 AND confidence <= 1.0),
    valid_from  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    valid_to    TEXT,                                        -- NULL = still valid
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
) STRICT;

CREATE INDEX idx_obs_entity ON observations(entity_id);
CREATE INDEX idx_obs_category ON observations(category);
CREATE INDEX idx_obs_valid ON observations(valid_from, valid_to);
CREATE INDEX idx_obs_source ON observations(source);
```

### 7.4 Relations

```sql
CREATE TABLE relations (
    id          TEXT PRIMARY KEY,                            -- ULID
    from_entity TEXT NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    to_entity   TEXT NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    relation    TEXT NOT NULL
        CHECK (relation IN (
            'owns', 'depends_on', 'blocks', 'unblocks',
            'works_with', 'part_of', 'uses', 'produces',
            'consumes', 'calls', 'extends', 'replaces',
            'tests', 'deploys_to', 'related_to'
        )),
    weight      REAL NOT NULL DEFAULT 1.0,
    metadata    TEXT,                                        -- JSON
    valid_from  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    valid_to    TEXT,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),

    UNIQUE(from_entity, to_entity, relation)
) STRICT;

CREATE INDEX idx_rel_from ON relations(from_entity);
CREATE INDEX idx_rel_to ON relations(to_entity);
CREATE INDEX idx_rel_type ON relations(relation);
```

### 7.5 Decisions

```sql
CREATE TABLE decisions (
    id              TEXT PRIMARY KEY,                        -- ULID
    title           TEXT NOT NULL,
    reasoning       TEXT,
    alternatives    TEXT,                                    -- JSON array
    scope           TEXT NOT NULL DEFAULT '',
    status          TEXT NOT NULL DEFAULT 'active'
        CHECK (status IN ('active', 'superseded', 'revisit', 'revoked')),
    superseded_by   TEXT REFERENCES decisions(id),
    decided_in      TEXT,                                    -- session ULID
    tags            TEXT,                                    -- JSON array
    created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
) STRICT;

CREATE INDEX idx_dec_status ON decisions(status);
CREATE INDEX idx_dec_scope ON decisions(scope);
```

### 7.6 Sessions

```sql
CREATE TABLE sessions (
    id              TEXT PRIMARY KEY,                        -- ULID
    session_type    TEXT NOT NULL
        CHECK (session_type IN (
            'claude-code', 'claude-ai', 'co-work', 'cursor',
            'copilot', 'windsurf', 'cline', 'codex', 'aider', 'other'
        )),
    project         TEXT NOT NULL,
    scope           TEXT NOT NULL DEFAULT '',
    status          TEXT NOT NULL DEFAULT 'active'
        CHECK (status IN ('active', 'completed', 'abandoned', 'handed-off')),
    goal            TEXT,
    summary         TEXT,
    accomplishments TEXT,                                    -- JSON array
    remaining       TEXT,                                    -- JSON array
    files_changed   TEXT,                                    -- JSON array
    decisions_made  TEXT,                                    -- JSON array of decision ULIDs
    entities_touched TEXT,                                   -- JSON array of entity IDs
    handoff_context TEXT,
    parent_session  TEXT REFERENCES sessions(id),
    child_session   TEXT,
    started_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    ended_at        TEXT
) STRICT;

CREATE INDEX idx_sess_status ON sessions(status);
CREATE INDEX idx_sess_project ON sessions(project);
CREATE INDEX idx_sess_type ON sessions(session_type);
CREATE INDEX idx_sess_parent ON sessions(parent_session);
CREATE INDEX idx_sess_started ON sessions(started_at);
```

### 7.7 State

```sql
CREATE TABLE state (
    id          TEXT PRIMARY KEY,                            -- ULID
    key         TEXT NOT NULL UNIQUE,
    title       TEXT NOT NULL,
    status      TEXT NOT NULL
        CHECK (status IN (
            'planned', 'in-progress', 'blocked', 'needs-review',
            'done', 'abandoned'
        )),
    owner       TEXT,
    details     TEXT,
    blockers    TEXT,                                        -- JSON array
    depends_on  TEXT,                                        -- JSON array of state keys
    scope       TEXT NOT NULL DEFAULT '',
    priority    TEXT NOT NULL DEFAULT 'medium'
        CHECK (priority IN ('critical', 'high', 'medium', 'low')),
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
) STRICT;

CREATE INDEX idx_state_status ON state(status);
CREATE INDEX idx_state_priority ON state(priority);
CREATE INDEX idx_state_scope ON state(scope);
```

### 7.8 Context

```sql
CREATE TABLE context (
    id          TEXT PRIMARY KEY,                            -- ULID
    key         TEXT NOT NULL UNIQUE,
    title       TEXT NOT NULL,
    content     TEXT NOT NULL,
    category    TEXT NOT NULL
        CHECK (category IN (
            'architecture', 'audit', 'spec', 'reference',
            'onboarding', 'runbook', 'postmortem', 'guide'
        )),
    scope       TEXT NOT NULL DEFAULT '',
    version     INTEGER NOT NULL DEFAULT 1,
    checksum    TEXT NOT NULL,                               -- SHA-256 of content
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
) STRICT;

CREATE INDEX idx_ctx_category ON context(category);
CREATE INDEX idx_ctx_scope ON context(scope);
```

### 7.9 Change Events (Watcher Layer)

Append-only log of all mutations. Enables cross-session notifications.

```sql
CREATE TABLE events (
    id          TEXT PRIMARY KEY,                            -- ULID (sortable by time)
    event_type  TEXT NOT NULL
        CHECK (event_type IN (
            'entity_created', 'entity_updated',
            'observation_added', 'observation_invalidated',
            'relation_created', 'relation_removed',
            'decision_logged', 'decision_superseded',
            'state_changed',
            'session_started', 'session_ended', 'session_handoff',
            'context_added', 'context_updated'
        )),
    source_session TEXT,                                     -- session ULID that caused this event
    target_type TEXT NOT NULL,                               -- "entity", "decision", "state", etc.
    target_id   TEXT NOT NULL,                               -- ID of the affected record
    scope       TEXT NOT NULL DEFAULT '',
    summary     TEXT NOT NULL,                               -- human-readable: "Added observation to Farouk: owns entity commit service"
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
) STRICT;

CREATE INDEX idx_events_type ON events(event_type);
CREATE INDEX idx_events_scope ON events(scope);
CREATE INDEX idx_events_created ON events(created_at);
CREATE INDEX idx_events_session ON events(source_session);
```

### 7.10 Briefing Cache

```sql
CREATE TABLE briefing_cache (
    scope           TEXT PRIMARY KEY,
    briefing        TEXT NOT NULL,
    generated_at    TEXT NOT NULL,
    events_cursor   TEXT NOT NULL                            -- ULID of last event included in this briefing
) STRICT;
```

Staleness is determined by checking if any events exist with ULID > events_cursor within the scope chain.

### 7.11 FTS5 Keyword Search

```sql
CREATE VIRTUAL TABLE observations_fts USING fts5(
    content,
    content=observations,
    content_rowid=rowid,
    tokenize='porter unicode61'
);

CREATE VIRTUAL TABLE decisions_fts USING fts5(
    title,
    reasoning,
    content=decisions,
    content_rowid=rowid,
    tokenize='porter unicode61'
);

CREATE VIRTUAL TABLE context_fts USING fts5(
    title,
    content,
    content=context,
    content_rowid=rowid,
    tokenize='porter unicode61'
);
```

FTS5 sync triggers (INSERT, UPDATE, DELETE) for all three tables. Same pattern as previous spec.

### 7.12 sqlite-vec Semantic Search

```sql
CREATE VIRTUAL TABLE observations_vec USING vec0(
    embedding float[384]                                    -- all-MiniLM-L6-v2 produces 384-dim vectors
);

CREATE VIRTUAL TABLE decisions_vec USING vec0(
    embedding float[384]
);

CREATE VIRTUAL TABLE context_vec USING vec0(
    embedding float[384]
);
```

Embeddings are generated on insert/update via the embedding engine (local candle or remote API). The rowid in the vec table maps to the rowid in the source table.

---

## 8. Dual Search System

Every search query runs two strategies in parallel and merges results with reciprocal rank fusion (RRF).

**Keyword search (FTS5):** Best for exact terms, names, specific phrases. "entity commit gate", "Farouk", "line 1562".

**Semantic search (sqlite-vec):** Best for conceptual queries. "that bug where data doesn't flow from master policies", "how authentication works".

**Search pipeline:**

```
Query
  |
  +-- FTS5 search -> ranked results (BM25 score)
  |
  +-- Embed query -> sqlite-vec KNN search -> ranked results (cosine distance)
  |
  v
Reciprocal Rank Fusion
  |
  v
Merged, deduplicated, re-ranked results
```

**RRF formula:**
```
RRF_score(doc) = sum(1 / (k + rank_i(doc))) for each ranking i
```
Where k = 60 (standard constant). This ensures documents that rank highly in both keyword and semantic search surface at the top.

---

## 9. Embedding Engine

Generates 384-dimensional embeddings for observations, decisions, and context documents.

### 9.1 Local Mode (default, recommended)

Uses `candle` (Rust ML framework by Hugging Face) to run `all-MiniLM-L6-v2` locally. Model downloaded once from Hugging Face Hub and cached at `~/.grafiki/models/`.

Advantages: No API calls, no cost, works offline, fast (< 5ms per embedding on Apple Silicon).

### 9.2 Remote Mode (optional)

Calls an external API (Ollama, OpenAI, or Anthropic) for embedding generation. Configured in `config.toml`.

### 9.3 Embedding Strategy

Embeddings are generated:
- On insert: when a new observation, decision, or context doc is created
- On update: when content changes (detected via SHA-256 checksum)
- In bulk: `grafiki embed --rebuild` regenerates all embeddings (useful when switching models)

Embedding generation is async and non-blocking. If embedding fails (model not downloaded yet, API down), the record is saved without an embedding and queued for retry. A background task processes the retry queue periodically.

---

## 10. Scope System

Hierarchical slash-delimited paths controlling knowledge visibility.

```
open-insurance                        -- project root
open-insurance/backend                -- backend subsystem
open-insurance/backend/enrichment     -- enrichment pipeline
open-insurance/backend/extraction     -- extraction pipeline
open-insurance/frontend               -- frontend
open-insurance/frontend/sov-editor    -- SOV editor specifically
open-insurance/infra                  -- infrastructure
```

**Resolution algorithm:**

Given scope `open-insurance/backend/enrichment`, generate the scope chain:

```
["", "open-insurance", "open-insurance/backend", "open-insurance/backend/enrichment"]
```

Query: `WHERE scope IN (?, ?, ?, ?)`

A backend/enrichment session sees global + backend + enrichment knowledge. Not frontend. Not infra.

---

## 11. CLI Commands

Binary name: `grafiki`

### 11.1 grafiki init

```
grafiki init [project-name] [--path <dir>]
```

Creates `~/.grafiki/<project>.db` with all tables, indices, triggers, FTS5, and vec0 tables. Creates `.grafiki` file in project root. Optionally downloads embedding model.

### 11.2 grafiki start

```
grafiki start
    --type <claude-code|claude-ai|co-work|cursor|copilot|windsurf|cline|codex|aider|other>
    --goal <string>
    [--scope <string>]
    [--copy]
    [--format plain|json|md]
```

1. Creates session record (status: active)
2. Checks briefing cache staleness via events_cursor
3. Generates or retrieves briefing
4. Outputs briefing
5. If `--copy`, copies to system clipboard

### 11.3 grafiki end

```
grafiki end
    [--summary <string>]
    [--accomplishments <comma-separated>]
    [--remaining <comma-separated>]
    [--files <comma-separated>]
    [--session <ulid>]
    [--interactive]
```

1. Updates session record
2. Emits events for all changes
3. Invalidates briefing caches for overlapping scopes

### 11.4 grafiki handoff

```
grafiki handoff
    [--session <ulid>]
    [--copy]
    [--format plain|json|md]
```

1. Generates compressed handoff document from current session
2. Creates new child session linked to parent
3. Marks current session as "handed-off"
4. Outputs handoff context

**Handoff document contains:**
- Original goal
- What was accomplished
- What's remaining
- Decisions made in this session
- Files changed
- Entities created/updated
- Current relevant state items
- Explicit next steps

### 11.5 grafiki status

```
grafiki status [--scope <string>] [--verbose] [--format plain|json]
```

Active sessions, in-progress state items, recent decisions, blockers, stale caches, recent events.

### 11.6 grafiki decide

```
grafiki decide <title>
    [--reasoning <string>]
    [--alternatives <comma-separated>]
    [--tags <comma-separated>]
    [--scope <string>]
    [--supersedes <decision-ulid>]
```

### 11.7 grafiki search

```
grafiki search <query>
    [--type observations|decisions|context|entities|all]
    [--mode keyword|semantic|hybrid]
    [--scope <string>]
    [--category <string>]
    [--limit <int>]
    [--format plain|json]
```

Default mode is `hybrid` (FTS5 + sqlite-vec with RRF). Falls back to `keyword` if embeddings are not available.

### 11.8 grafiki save

```
grafiki save <entity-name>
    --type <entity_type>
    [--observe <text>]
    [--category <category>]
    [--scope <string>]
    [--relate <entity-id>:<relation>]
```

### 11.9 grafiki context

```
grafiki context add <key> --title <string> --category <cat> [--file <path>] [--content <string>] [--scope <string>]
grafiki context show <key> [--format plain|json|md]
grafiki context list [--category <cat>] [--scope <string>]
grafiki context update <key> [--file <path>] [--content <string>] [--title <string>]
grafiki context delete <key> [--confirm]
```

### 11.10 grafiki log

```
grafiki log [--last <int>] [--type <session_type>] [--scope <string>] [--format plain|json]
```

### 11.11 grafiki graph

```
grafiki graph <entity-id>
    [--depth <int>]
    [--algorithm bfs|dfs|shortest-path]
    [--target <entity-id>]            -- for shortest-path
    [--format plain|json|dot|html]
```

Traverses the knowledge graph from a given entity.

`--format dot` outputs Graphviz DOT format.
`--format html` generates an interactive vis.js visualization and opens it in the default browser. The HTML file is self-contained (single file, no external dependencies) and saved to `~/.grafiki/views/<project>-graph.html`.

```
grafiki graph --full [--format html]
```

Renders the entire knowledge graph. With `--format html`, produces a force-directed vis.js graph where:
- Node size scales with degree (more connections = larger node)
- Node color maps to entity_type (person = blue, service = green, module = purple, etc.)
- Edge labels show relation type
- Edges are color-coded by confidence (EXTRACTED = solid, INFERRED = dashed, AMBIGUOUS = dotted)
- Nodes are grouped by community (Leiden clustering)
- Click a node to see its observations
- Search bar filters nodes by name
- Filter panel toggles entity types and communities on/off

### 11.12 grafiki analyze

```
grafiki analyze [--scope <string>]
```

Runs the full analysis pipeline on the knowledge graph:

1. **Community detection** (Leiden algorithm): clusters entities into communities of related concepts. A community might be "auth system", "enrichment pipeline", "frontend components", etc.

2. **God nodes**: identifies the highest-degree entities (most connections). These are the concepts everything in your project revolves around.

3. **Surprising connections**: finds edges between entities in different communities. These are unexpected dependencies or relationships that cross domain boundaries. Ranked by a composite score: cross-community edges rank higher, edges between different entity types rank higher.

4. **Orphan entities**: entities with zero or one connections. These might be missing knowledge or forgotten components.

5. **Suggested queries**: generates 4-5 questions the graph is uniquely positioned to answer based on its structure.

Output is printed to terminal and saved to `~/.grafiki/reports/<project>-analysis.md`.

### 11.13 grafiki report

```
grafiki report [--scope <string>] [--format md|json] [--output <path>]
```

Generates a GRAPH_REPORT.md with:

```markdown
# Grafiki Report: open-insurance

## God Nodes (highest connectivity)
1. sov (14 connections) - Central product artifact
2. auth-service (9 connections) - JWT validation and refresh
3. farouk (8 connections) - Backend engineer

## Communities
- Backend Core: auth-service, entity-commit-service, celery-workers, postgres, redis
- Enrichment Pipeline: fema-api, attom-api, hazardhub, normalization-service
- Frontend: sov-editor, dashboard, properties-page, tanstack-query
- AI/ML: gemini, mistral-ocr, textract, milvus, rag-assistant

## Surprising Connections
1. sov-editor -> celery-workers (cross: Frontend -> Backend Core)
   Why: SOV editor triggers async extraction jobs via Celery
2. fema-api -> rag-assistant (cross: Enrichment -> AI/ML)
   Why: RAG assistant queries enrichment data for flood zone answers

## Orphan Entities
- migrations (1 connection, might need more context)
- stash-app (0 connections, separate project?)

## Suggested Queries
1. What happens if postgres goes down? (5 services depend on it)
2. How does data flow from document upload to SOV display?
3. What does Farouk own that has no test coverage?
```

### 11.14 grafiki serve

```
grafiki serve [--port <int>] [--project <string>] [--daemon]
```

Starts HTTP API + MCP stdio listener. With `--daemon`, forks to background with PID file.

### 11.15 grafiki events

```
grafiki events [--last <int>] [--since <ulid>] [--scope <string>] [--format plain|json]
```

Shows the event log. Useful for debugging cross-session awareness.

### 11.16 grafiki embed

```
grafiki embed --rebuild                 -- regenerate all embeddings
grafiki embed --status                  -- show embedding coverage stats
grafiki embed --download-model          -- download embedding model
```

### 11.17 grafiki install-service / uninstall-service

macOS: `~/Library/LaunchAgents/com.grafiki.server.plist`
Linux: `~/.config/systemd/user/grafiki.service`

### 11.18 grafiki export

```
grafiki export [--format json|sqlite|md|html|wiki|graphml|dot] [--output <path>]
grafiki import <path> [--merge|--replace]
```

Export formats:
- `json`: full graph as JSON (nodes, edges, observations, decisions, state)
- `sqlite`: raw database copy
- `md`: markdown summary document
- `html`: interactive vis.js graph (same as `grafiki graph --full --format html`)
- `wiki`: agent-navigable markdown vault. One `index.md` entry point, one article per community, one article per god node. Any AI agent pointed at `index.md` can navigate the entire knowledge base by reading files.
- `graphml`: GraphML format for Gephi, yEd, and other graph visualization tools
- `dot`: Graphviz DOT format

### 11.19 grafiki hook

```
grafiki hook install [--tool claude-code|cursor|windsurf|codex]
grafiki hook uninstall [--tool claude-code|cursor|windsurf|codex]
```

Installs a PreToolUse hook for the specified AI coding tool. When installed, the hook fires before file search operations (Glob, Grep) and reminds the AI to check the knowledge graph first:

For Claude Code: installs a PreToolUse hook in `.claude/settings.json` and adds a section to CLAUDE.md:
```
grafiki: Knowledge graph exists with {N} entities, {M} relations.
Check GRAPH_REPORT.md for god nodes and community structure before searching raw files.
```

For Cursor: updates `.cursorrules` with graph-aware instructions.
For Windsurf: updates `.windsurfrules`.
For Codex: updates `AGENTS.md` and installs a UserPromptSubmit hook.

### 11.20 grafiki completions

```
grafiki completions <bash|zsh|fish|powershell>
```

---

## 12. MCP Server

Uses `rmcp` (official Rust MCP SDK). Runs as stdio transport.

### 12.1 Configuration

```json
{
  "mcpServers": {
    "grafiki": {
      "command": "grafiki",
      "args": ["mcp", "--project", "open-insurance"]
    }
  }
}
```

### 12.2 Tools

```
grafiki_start_session     { type, goal, scope } -> { session_id, briefing }
grafiki_end_session       { session_id, summary, accomplishments, remaining, files_changed } -> { success, duration_minutes }
grafiki_handoff           { session_id } -> { new_session_id, handoff_context }
grafiki_get_briefing      { scope } -> { briefing }
grafiki_save_entity       { name, entity_type, scope, metadata? } -> { entity_id, created }
grafiki_add_observation   { entity_id, content, category, confidence? } -> { observation_id }
grafiki_add_relation      { from_entity, to_entity, relation, metadata? } -> { relation_id }
grafiki_log_decision      { title, reasoning?, alternatives?, tags?, scope? } -> { decision_id }
grafiki_search            { query, type?, mode?, scope?, limit? } -> { results[] }
grafiki_get_status        { scope? } -> { active_sessions, state_items, recent_decisions, blockers }
grafiki_update_state      { key, status?, details?, blockers?, owner? } -> { success }
grafiki_get_context       { key } -> { title, content, category, version }
grafiki_get_graph         { entity_id, depth? } -> { entities[], relations[] }
grafiki_get_events        { since?, scope?, limit? } -> { events[] }
```

### 12.3 Auto-Discovery Instructions

Generated by `grafiki init` and appended to CLAUDE.md / .cursorrules / .windsurfrules:

```markdown
## Grafiki

You have access to Grafiki via MCP tools.

1. At session start, call `grafiki_get_briefing` to understand current context.
2. When you make architectural decisions, call `grafiki_log_decision`.
3. When you discover patterns, gotchas, or conventions, call `grafiki_add_observation`.
4. When you encounter a new service, API, person, or concept, call `grafiki_save_entity`.
5. When you identify dependencies, call `grafiki_add_relation`.
6. When a work item status changes, call `grafiki_update_state`.
7. When the session ends, call `grafiki_end_session` with a summary.

Categories for observations: architecture, decision, blocker, pattern, progress, gotcha, learned, convention, dependency, risk.
```

---

## 13. HTTP API

JSON REST on localhost:9700. Full CRUD for all tables plus search, briefing, graph, and events.

### Sessions
```
POST   /api/sessions/start
POST   /api/sessions/:id/end
POST   /api/sessions/:id/handoff
GET    /api/sessions/active
GET    /api/sessions/:id
GET    /api/sessions?limit=&type=&scope=
```

### Knowledge Graph
```
POST   /api/entities
GET    /api/entities/:id
GET    /api/entities?type=&scope=
PUT    /api/entities/:id
DELETE /api/entities/:id
POST   /api/entities/:id/observations
GET    /api/entities/:id/observations
DELETE /api/observations/:id
POST   /api/relations
GET    /api/relations?entity=
DELETE /api/relations/:id
GET    /api/graph/:entity_id?depth=&algorithm=
```

### Decisions
```
POST   /api/decisions
GET    /api/decisions?status=&scope=
GET    /api/decisions/:id
PUT    /api/decisions/:id
POST   /api/decisions/:id/supersede
```

### State
```
POST   /api/state
GET    /api/state?status=&scope=&priority=
PUT    /api/state/:key
DELETE /api/state/:key
```

### Context
```
POST   /api/context
GET    /api/context/:key
GET    /api/context?category=&scope=
PUT    /api/context/:key
DELETE /api/context/:key
```

### Search, Briefing, Events, Analysis, System
```
GET    /api/search?q=&type=&mode=&scope=&limit=
GET    /api/briefing?scope=
GET    /api/events?since=&scope=&limit=
POST   /api/analyze?scope=                               -- run analysis pipeline
GET    /api/analyze/report?scope=                         -- get latest analysis report
GET    /api/analyze/communities?scope=                    -- get community assignments
GET    /api/analyze/god-nodes?scope=&limit=               -- get highest-degree entities
GET    /api/graph/full?format=json                        -- full graph as JSON for vis.js
GET    /api/graph/html                                    -- rendered vis.js HTML
GET    /api/health
GET    /api/stats
```

---

## 14. TUI Dashboard

Built with `ratatui` + `crossterm`.

```
grafiki tui [--project <string>]
```

**Layout:**

```
+--[ Grafiki: open-insurance ]--------------------------------------------+
|                                                                                |
|  ACTIVE SESSIONS                  | STATE                                      |
|  > claude-code [backend/enrich]   | > fema-integration [in-progress] vishal    |
|    Goal: FEMA flood zone API      | > entity-commit-fix [done] vishal          |
|    15 min ago                     | > dashboard-premium [unblocked] farouk     |
|  > claude-ai [hiring]             | > sov-editor [planned] unassigned          |
|    Goal: Draft GeoAI JD           |                                            |
|    5 min ago                      |                                            |
|                                   |                                            |
|  RECENT DECISIONS                 | RECENT EVENTS                              |
|  [D15] Enterprise LLMs for norm   | entity-commit-service updated (2m ago)     |
|  [D14] DevOps shared              | Decision D16 logged (15m ago)              |
|  [D12] Geospatial is core         | Session abc ended (1h ago)                 |
|                                   |                                            |
|  BLOCKERS                         | SEARCH: [________________________]         |
|  (none)                           |                                            |
+--[ q:quit s:search n:new d:decide e:end h:handoff /:filter Tab:switch ]-------+
```

**Keybindings:** j/k navigate, Enter drill in, s search, n new session, d decide, e end session, h handoff, q quit, / filter by scope, Tab switch panels.

---

## 15. Briefing Generation

### 15.1 Template Mode (default)

Structured SQL queries with scope resolution, formatted into the briefing template.

**Queries:**
1. Active decisions in scope chain (limit 10, newest first)
2. State items in scope chain where status IN ('in-progress', 'blocked', 'needs-review')
3. Last 5 completed sessions in scope chain
4. Entities with recent observations (last 7 days) in scope chain (limit 15)
5. Blocked state items
6. Events since last briefing generation for this scope

**Briefing size target:** Under 2,000 tokens. Truncate oldest sessions and lowest-confidence observations first.

### 15.2 LLM Mode (optional)

Passes raw query results to an LLM for natural language summarization.

**Supported providers:** Ollama (local, default), Anthropic, OpenAI.

**Prompt:**
```
You are a project context generator. Given raw data about a software project,
generate a concise briefing for a developer starting a new AI coding session.

Focus on: what's relevant to their stated goal, active decisions, in-progress work
that might affect them, and blockers.

Keep under 1500 tokens. Be specific, not generic.

Goal: {goal}
Scope: {scope}
Data: {serialized results}
```

---

## 16. Graph Traversal and Analysis

### 16.1 Traversal Algorithms

**BFS (default):** Breadth-first from root entity to configurable depth.
**DFS:** Depth-first for following dependency chains.
**Shortest Path:** BFS-based shortest path between two entities. Returns (entity, relation, entity) triples.

All traversals respect bitemporal validity (only follow relations where valid_to IS NULL).

### 16.2 Community Detection

Uses the Leiden algorithm (improvement over Louvain) for community detection on the entity-relation graph. Communities are groups of entities that are more densely connected to each other than to the rest of the graph.

Rust implementation: use `petgraph` for graph representation, implement Leiden or use a binding. Communities are stored as a computed property (not persisted in the database) and regenerated on `grafiki analyze`.

Each community is auto-labeled by its most connected entity (god node) or by the dominant entity_type. Example communities:
- "auth-service cluster" (auth-service, workos-sdk, jwt, session-management)
- "enrichment pipeline" (fema-api, attom, hazardhub, normalization-service)
- "frontend" (sov-editor, dashboard, properties-page, tanstack-query)

### 16.3 Analysis Pipeline

Inspired by Graphify's `detect() → extract() → build_graph() → cluster() → analyze() → report() → export()` pipeline.

Grafiki's analysis pipeline (runs on `grafiki analyze`):

```
load_graph() → cluster() → identify_god_nodes() → find_surprising_connections() → find_orphans() → generate_suggestions() → report()
```

**load_graph():** Read all entities, observations, and relations from SQLite into a petgraph DiGraph.

**cluster():** Run Leiden community detection. Assign each entity to a community.

**identify_god_nodes():** Rank entities by degree (in-degree + out-degree). Top 10 are god nodes.

**find_surprising_connections():** Find relations that cross community boundaries. Score by:
- Cross-community bonus: 2x weight
- Cross-entity-type bonus: 1.5x (e.g., person -> service is more surprising than service -> service)
- Low-confidence penalty: reduce score for AMBIGUOUS edges

**find_orphans():** Entities with 0 or 1 connections. Flag as potentially missing knowledge.

**generate_suggestions():** Based on graph structure, generate questions like:
- "What happens if {god_node} goes down? ({N} entities depend on it)"
- "How does data flow from {community_A} to {community_B}?"
- "{entity} has {N} blockers, is it the critical path?"

**report():** Format results into GRAPH_REPORT.md.

### 16.4 Interactive Visualization (vis.js)

`grafiki graph --full --format html` generates a self-contained HTML file with an embedded vis.js force-directed graph.

The HTML file includes:
- vis.js loaded from CDN (with local fallback embedded)
- All graph data serialized as JSON inside the file
- Force-directed physics layout
- Node sizing by degree
- Node coloring by entity_type
- Edge styling by confidence (solid/dashed/dotted)
- Community grouping with colored backgrounds
- Click-to-inspect: clicking a node shows its observations in a side panel
- Search bar: filter nodes by name
- Filter toggles: show/hide entity types and communities
- Zoom and pan controls
- Dark mode support

File saved to `~/.grafiki/views/<project>-graph.html` and auto-opened in the default browser.

### 16.5 Wiki Export

`grafiki export --wiki --output ./grafiki-wiki/` generates an agent-navigable markdown vault:

```
grafiki-wiki/
    index.md                    -- entry point: project overview, link to all communities and god nodes
    communities/
        auth-system.md          -- all entities in the auth community, their observations, relations
        enrichment-pipeline.md  -- enrichment community
        frontend.md             -- frontend community
    entities/
        auth-service.md         -- deep dive on auth-service: all observations, relations, decisions
        sov.md                  -- deep dive on SOV
        farouk.md               -- deep dive on Farouk
    GRAPH_REPORT.md             -- god nodes, surprising connections, suggested queries
```

Any AI agent pointed at `index.md` can navigate the entire knowledge base by following links. This is how you give a new Claude Code session complete project understanding without consuming context window tokens on raw database queries.

### 16.6 PreToolUse Hook Integration

`grafiki hook install --tool claude-code` installs a hook that fires before file search operations. When the knowledge graph exists, Claude sees:

```
grafiki: Knowledge graph exists with 47 entities, 83 relations across 5 communities.
God nodes: sov (14), auth-service (9), farouk (8).
Read ~/.grafiki/reports/open-insurance-analysis.md for structure before searching raw files.
```

This makes Claude navigate via the knowledge graph instead of grepping through every file, saving tokens and producing better answers.

### 16.7 Edge Confidence Tagging

Relations have a confidence field, but also a source_type tag:

- **EXTRACTED**: Directly stated by the developer via CLI or confirmed in a session. High confidence.
- **INFERRED**: AI agent added this observation during a coding session. Medium confidence.
- **AMBIGUOUS**: Auto-detected or uncertain. Needs human review.

The `grafiki analyze` command surfaces AMBIGUOUS edges for review. The vis.js visualization uses solid lines for EXTRACTED, dashed for INFERRED, and dotted for AMBIGUOUS.

---

## 17. Concurrency Model

**Connection pool:** `r2d2` pool with:
- 4 read-only connections
- 1 write connection with mutex

**WAL mode** allows concurrent readers while a single writer holds the lock. The write connection uses a mutex (`tokio::sync::Mutex`) to serialize all writes.

**CLI fallback:** When daemon is not running, CLI opens a direct connection (WAL mode still allows concurrent access from other processes).

**Embedding queue:** Embedding generation is async. A `tokio::mpsc` channel receives embedding requests, and a background task processes them, writing results back to the vec tables.

---

## 18. Error Handling

```rust
#[derive(thiserror::Error, Debug)]
pub enum GrafikiError {
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("Entity not found: {0}")]
    EntityNotFound(String),

    #[error("Session not found: {0}")]
    SessionNotFound(String),

    #[error("Decision not found: {0}")]
    DecisionNotFound(String),

    #[error("State key not found: {0}")]
    StateNotFound(String),

    #[error("Context key not found: {0}")]
    ContextNotFound(String),

    #[error("No active session. Run 'grafiki start' first.")]
    NoActiveSession,

    #[error("Project not initialized. Run 'grafiki init' first.")]
    ProjectNotInitialized,

    #[error("Invalid scope format: {0}")]
    InvalidScope(String),

    #[error("Embedding error: {0}")]
    Embedding(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("HTTP client error: {0}")]
    HttpClient(String),

    #[error("LLM provider error: {0}")]
    Llm(String),

    #[error("MCP transport error: {0}")]
    Mcp(String),

    #[error("Pool error: {0}")]
    Pool(String),
}
```

CLI: human-readable to stderr, non-zero exit codes.
HTTP: proper status codes with JSON error bodies `{ "error": "...", "code": "..." }`.
MCP: JSON-RPC 2.0 error responses.

---

## 19. Project Structure

```
grafiki/
    Cargo.toml
    build.rs                            -- embed migrations, model metadata
    src/
        main.rs
        lib.rs

        db/
            mod.rs                      -- pool setup, connection management
            schema.rs                   -- table creation SQL
            migrations.rs               -- embedded migration runner
            pool.rs                     -- r2d2 pool configuration

        models/
            mod.rs
            entity.rs
            observation.rs
            relation.rs
            decision.rs
            session.rs
            state.rs
            context.rs
            event.rs
            briefing.rs

        store/
            mod.rs                      -- Store trait
            entities.rs
            observations.rs
            relations.rs
            decisions.rs
            sessions.rs
            state.rs
            context.rs
            events.rs
            search.rs                   -- FTS5 + sqlite-vec + RRF fusion
            briefing.rs
            graph.rs                    -- BFS, DFS, shortest path
            scope.rs

        analysis/
            mod.rs                      -- analysis pipeline orchestration
            community.rs                -- Leiden community detection via petgraph
            god_nodes.rs                -- highest-degree entity identification
            connections.rs              -- surprising cross-community connections
            orphans.rs                  -- orphan entity detection
            suggestions.rs              -- auto-generated query suggestions
            report.rs                   -- GRAPH_REPORT.md generation

        visualize/
            mod.rs
            visjs.rs                    -- vis.js HTML generation (self-contained)
            wiki.rs                     -- agent-navigable markdown wiki export
            graphml.rs                  -- GraphML export for Gephi/yEd
            dot.rs                      -- Graphviz DOT export

        hooks/
            mod.rs
            claude_code.rs              -- PreToolUse hook + CLAUDE.md integration
            cursor.rs                   -- .cursorrules integration
            windsurf.rs                 -- .windsurfrules integration
            codex.rs                    -- AGENTS.md + UserPromptSubmit hook

        cli/
            mod.rs
            init.rs
            start.rs
            end.rs
            handoff.rs
            status.rs
            decide.rs
            search.rs
            save.rs
            context.rs
            log.rs
            serve.rs
            graph.rs
            analyze.rs
            report.rs
            events.rs
            embed.rs
            hook.rs
            tui_cmd.rs
            install.rs
            export.rs
            completions.rs

        mcp/
            mod.rs
            server.rs                   -- rmcp ServerHandler implementation
            tools.rs                    -- tool definitions via rmcp macros
            instructions.rs

        api/
            mod.rs                      -- axum router
            sessions.rs
            entities.rs
            decisions.rs
            state.rs
            context.rs
            search.rs
            events.rs
            system.rs

        tui/
            mod.rs
            app.rs
            ui.rs
            input.rs
            panels/
                sessions.rs
                state.rs
                decisions.rs
                events.rs
                search.rs

        embed/
            mod.rs                      -- embedding engine abstraction
            local.rs                    -- candle + all-MiniLM-L6-v2
            remote.rs                   -- ollama / openai / anthropic
            queue.rs                    -- async embedding queue

        config.rs
        error.rs
        scope.rs
        ulid.rs                         -- ULID generation utilities

    tests/
        common/mod.rs                   -- test helpers, temp DB creation
        integration/
            cli_tests.rs
            mcp_tests.rs
            api_tests.rs
            scope_tests.rs
            search_tests.rs
            handoff_tests.rs
            graph_tests.rs
            events_tests.rs
            embedding_tests.rs
        property/
            store_props.rs              -- proptest property-based tests
            scope_props.rs
            search_props.rs
```

---

## 20. Testing Strategy

### Unit Tests
Every store function, scope resolution, briefing template, RRF fusion, graph traversal.

### Integration Tests
CLI commands end-to-end against temp SQLite databases. MCP simulated stdio. Axum test client.

### Property-Based Tests (proptest)
- Any entity saved can be retrieved
- Scope resolution is monotonic (deeper scope always sees more)
- Search results are deterministic for the same query
- Handoff creates valid parent-child chain
- Events are append-only and monotonically ordered by ULID
- RRF fusion never drops results present in either ranking

### Benchmarks
- Briefing generation time (target: < 50ms for 1000 observations)
- Search latency: FTS5 (target: < 5ms), semantic (target: < 20ms), hybrid (target: < 30ms)
- Embedding throughput (target: > 100 embeddings/sec local)

---

## 21. Configuration

```toml
# ~/.grafiki/config.toml

default_project = "open-insurance"

[briefing]
mode = "template"                       # template | llm
max_recent_sessions = 5
max_observations = 20
max_tokens = 2000

[search]
default_mode = "hybrid"                 # keyword | semantic | hybrid
rrf_k = 60

[embedding]
engine = "local"                        # local | ollama | openai | anthropic
model = "all-MiniLM-L6-v2"             # for local
dimensions = 384
ollama_url = "http://localhost:11434"
api_key = ""

[llm]
provider = "ollama"                     # for LLM briefing mode
model = "llama3.2"
base_url = "http://localhost:11434"
api_key = ""

[server]
port = 9700
host = "127.0.0.1"

[session]
default_type = "claude-code"
auto_copy_briefing = true

[display]
color = true
format = "plain"
```

---

## 22. Project Detection

Priority:
1. `--project` flag
2. `.grafiki` file in cwd or parent directories
3. Git remote name from `.git/config`
4. Directory name

Multiple projects supported. Each gets its own `.db` file.

---

## 23. Installation

```bash
# macOS
brew install grafiki

# Direct download
curl -fsSL https://grafiki.dev/install.sh | sh

# From source
cargo install grafiki

# From repo
git clone https://github.com/<org>/grafiki
cd grafiki
cargo build --release
```

**Targets:** aarch64-apple-darwin, x86_64-apple-darwin, x86_64-unknown-linux-gnu, aarch64-unknown-linux-gnu, x86_64-pc-windows-msvc.

CI via GitHub Actions with `cross` for all targets. Binaries attached to GitHub releases.

---

## 24. File Layout

```
~/.grafiki/
    config.toml
    grafiki.pid
    grafiki.log
    models/                              -- cached embedding models
        all-MiniLM-L6-v2/
    open-insurance.db
    the-garden.db
    stash.db
```

Project root:
```
open-insurance-web/
    .grafiki                               -- contains "open-insurance"
    CLAUDE.md                            -- includes grafiki instructions
```

---

## 25. Connector Architecture

Connectors are background workers that watch external services and automatically extract knowledge into the graph. Each connector runs as a separate async task within the `grafiki serve` daemon.

### 25.1 Connector Trait

Every connector implements the same interface:

```rust
#[async_trait]
pub trait Connector: Send + Sync {
    fn name(&self) -> &str;
    fn source_type(&self) -> &str;                       // "slack", "gmail", "calendar", etc.
    async fn authenticate(&mut self) -> Result<()>;
    async fn poll(&self) -> Result<Vec<RawEvent>>;       // fetch new events since last poll
    async fn extract(&self, event: RawEvent) -> Result<Vec<KnowledgeItem>>;  // LLM extraction
    fn poll_interval(&self) -> Duration;
}

pub enum KnowledgeItem {
    Entity { name: String, entity_type: String, scope: String, observations: Vec<String> },
    Decision { title: String, reasoning: String, tags: Vec<String> },
    StateChange { key: String, status: String, details: String },
    Relation { from: String, to: String, relation: String },
    Observation { entity_id: String, content: String, category: String },
}
```

Each connector polls its service at a configured interval, extracts raw events, passes them through an LLM extraction pipeline (local Ollama or cloud API), and writes the resulting knowledge items to the graph. The `source` field on every observation records the origin: `"slack:C04ABCD1234:1711234567"`, `"gmail:thread-abc123"`, `"granola:meeting-xyz"`.

### 25.2 Extraction Pipeline

Raw events from connectors go through a 3-stage extraction:

1. **Filter**: Skip noise (emoji reactions, "ok" messages, bot messages, calendar declines)
2. **Classify**: Is this a decision, action item, context update, status change, or noise?
3. **Extract**: Pull structured knowledge items using LLM with connector-specific prompts

The extraction prompt is tuned per connector. Slack extraction focuses on decisions and action items. Gmail extraction focuses on commitments and external stakeholder context. Meeting extraction focuses on outcomes and follow-ups.

### 25.3 Deduplication

When a connector extracts knowledge that already exists in the graph (same entity, similar observation), it deduplicates using:
- Exact match on entity ID + observation content hash
- Semantic similarity via sqlite-vec (if embedding similarity > 0.92, skip)
- Temporal proximity (same entity, same category, within 1 hour = likely duplicate)

---

## 26. Connectors

### 26.1 Slack Connector

```toml
[connectors.slack]
enabled = true
token = ""                              # Slack Bot User OAuth Token (xoxb-)
channels = ["general", "engineering", "product"]  # channels to watch, or "*" for all
poll_interval_seconds = 30
extract_decisions = true
extract_action_items = true
extract_context_changes = true
ignore_bots = true
ignore_threads_shorter_than = 2         # skip single messages without replies
```

**What it extracts:**
- Decisions: "we decided to...", "let's go with...", "the plan is..."
- Action items: "@farouk will fix...", "TODO:", "needs to be done by..."
- Context changes: "geospatial is now core", "we got funding", "Zach wants..."
- Entity mentions: people, projects, tools, services mentioned in context

**Auth:** Slack Bot Token with channels:history, channels:read, users:read scopes. Alternatively, use the existing Slack MCP connection if available.

### 26.2 Gmail Connector

```toml
[connectors.gmail]
enabled = true
credentials_path = ""                   # Google OAuth credentials JSON
labels = ["INBOX", "SENT"]              # which labels to watch
poll_interval_seconds = 120
extract_commitments = true
extract_vendor_context = true
ignore_newsletters = true
ignore_automated = true                 # skip CI notifications, calendar invites, etc.
```

**What it extracts:**
- Commitments: "I'll send the proposal by Friday", "we agreed to..."
- Vendor context: communications with external services, API providers, partners
- Stakeholder updates: progress reports, status emails, feedback
- Entity creation: new people, companies, projects mentioned in email

### 26.3 Google Calendar Connector

```toml
[connectors.calendar]
enabled = true
credentials_path = ""                   # shares OAuth with Gmail
calendars = ["primary"]
poll_interval_seconds = 300
link_to_granola = true                  # match calendar events to Granola transcripts
```

**What it extracts:**
- Meeting entities: who attended, when, topic
- Links events to Granola transcripts when available (matched by time + title)
- Pre-meeting context: "you're meeting with Cape Analytics in 2 hours, here's what you know about them"
- Post-meeting follow-ups: if Granola extracted action items, check if they've been addressed

### 26.4 Granola Connector

```toml
[connectors.granola]
enabled = true
poll_interval_seconds = 300
extract_decisions = true
extract_action_items = true
extract_key_insights = true
```

**What it extracts:**
- Decisions made during meetings
- Action items assigned to people
- Key insights and context from discussions
- Entity relationships (who said what about which project)
- Links to calendar events for temporal context

**Auth:** Uses the Granola MCP server already connected.

### 26.5 Linear Connector

```toml
[connectors.linear]
enabled = true
api_key = ""                            # Linear API key
teams = ["engineering"]                 # which teams to watch
poll_interval_seconds = 60
sync_issue_status = true
sync_comments = true
sync_project_updates = true
```

**What it extracts:**
- State changes: issue moved from "In Progress" to "Done"
- Blockers: issue marked as blocked, with reason
- Assignments: who owns what
- Project progress: milestone completion, sprint progress
- Comment context: decisions and discussions in issue comments
- Relations: issue dependencies, parent/child relationships

### 26.6 GitHub Connector

```toml
[connectors.github]
enabled = true
token = ""                              # GitHub Personal Access Token
repos = ["OpenInsured/portal-FE", "OpenInsured/portal-be"]
poll_interval_seconds = 120
sync_prs = true
sync_commits = true
sync_issues = true
sync_reviews = true
```

**What it extracts:**
- PR summaries: what changed, which files, who reviewed
- Commit messages as progress observations
- Code review decisions: approved, requested changes, and why
- Issue status changes
- Entity updates: files changed map to file entities in the graph
- Relations: PRs that modify related files create dependency edges

---

## 27. Connector CLI Commands

```
grafiki connect status                          -- show all connector statuses
grafiki connect enable <connector>              -- enable a connector
grafiki connect disable <connector>             -- disable a connector
grafiki connect test <connector>                -- test authentication and fetch a sample
grafiki connect sync <connector>                -- force immediate sync
grafiki connect history <connector> [--last N]  -- show recent extraction activity
```

---

## 28. Updated Cargo Workspace

```
grafiki/
    Cargo.toml                          -- workspace root
    crates/
        grafiki-core/                   -- db, models, store, search, scope, graph
        grafiki-embed/                  -- candle embeddings, sqlite-vec
        grafiki-analysis/               -- community detection, god nodes, report generation
        grafiki-visualize/              -- vis.js, wiki, graphml, dot export
        grafiki-cli/                    -- clap CLI commands
        grafiki-mcp/                    -- rmcp MCP server
        grafiki-api/                    -- axum HTTP API
        grafiki-tui/                    -- ratatui TUI dashboard
        grafiki-hooks/                  -- Claude Code, Cursor, Windsurf, Codex hooks
        grafiki-connect/                -- connector trait + shared extraction pipeline
        grafiki-connect-slack/          -- Slack connector
        grafiki-connect-gmail/          -- Gmail connector
        grafiki-connect-calendar/       -- Google Calendar connector
        grafiki-connect-granola/        -- Granola connector
        grafiki-connect-linear/         -- Linear connector
        grafiki-connect-github/         -- GitHub connector
```

Each connector crate depends on `grafiki-core` and `grafiki-connect`. The main binary selects connectors via Cargo features:

```toml
[features]
default = ["all-connectors"]
all-connectors = ["slack", "gmail", "calendar", "granola", "linear", "github"]
slack = ["grafiki-connect-slack"]
gmail = ["grafiki-connect-gmail"]
calendar = ["grafiki-connect-calendar"]
granola = ["grafiki-connect-granola"]
linear = ["grafiki-connect-linear"]
github = ["grafiki-connect-github"]
```

This means users can compile with only the connectors they need: `cargo install grafiki --features "slack,github"`.

---

## 29. Updated Configuration

```toml
# ~/.grafiki/config.toml

default_project = "open-insurance"

[briefing]
mode = "template"
max_recent_sessions = 5
max_observations = 20
max_tokens = 2000

[search]
default_mode = "hybrid"
rrf_k = 60

[embedding]
engine = "local"
model = "all-MiniLM-L6-v2"
dimensions = 384

[extraction]
provider = "ollama"                     # LLM for connector extraction
model = "llama3.2"
base_url = "http://localhost:11434"

[server]
port = 9700
host = "127.0.0.1"

[session]
default_type = "claude-code"
auto_copy_briefing = true

[connectors.slack]
enabled = true
token = ""
channels = ["*"]
poll_interval_seconds = 30

[connectors.gmail]
enabled = true
credentials_path = "~/.grafiki/google-credentials.json"
labels = ["INBOX", "SENT"]
poll_interval_seconds = 120

[connectors.calendar]
enabled = true
calendars = ["primary"]
poll_interval_seconds = 300
link_to_granola = true

[connectors.granola]
enabled = true
poll_interval_seconds = 300

[connectors.linear]
enabled = true
api_key = ""
teams = ["engineering"]
poll_interval_seconds = 60

[connectors.github]
enabled = true
token = ""
repos = ["OpenInsured/portal-FE", "OpenInsured/portal-be"]
poll_interval_seconds = 120

[display]
color = true
format = "plain"

[privacy]
redact_emails = false                   # redact email addresses in observations
redact_names = false                    # redact names in observations
exclude_channels = []                   # Slack channels to never extract from
exclude_labels = ["SPAM", "PROMOTIONS"] # Gmail labels to skip
```

---

## 30. Updated File Layout

```
~/.grafiki/
    config.toml
    grafiki.pid
    grafiki.log
    google-credentials.json              -- OAuth credentials for Gmail + Calendar
    models/
        all-MiniLM-L6-v2/
    views/
        open-insurance-graph.html        -- vis.js visualization
    reports/
        open-insurance-analysis.md       -- latest GRAPH_REPORT
    wiki/
        open-insurance/                  -- wiki export
            index.md
            communities/
            entities/
    open-insurance.db
    the-garden.db
```
