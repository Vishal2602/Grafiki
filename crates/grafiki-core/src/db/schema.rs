use rusqlite::Connection;

use crate::error::GrafikiError;
use crate::Result;

pub const INITIAL_SCHEMA_VERSION: i64 = 1;
/// The newest schema version this build knows how to produce. Bump this and add
/// a `Migration` entry whenever the schema changes.
pub const LATEST_SCHEMA_VERSION: i64 = 1;

struct Migration {
    version: i64,
    description: &'static str,
    sql: &'static str,
}

const MIGRATIONS: &[Migration] = &[Migration {
    version: 1,
    description: "initial core schema with sessions, graph, decisions, state, context, events, and FTS",
    sql: INITIAL_SCHEMA,
}];

/// Bring a database up to LATEST_SCHEMA_VERSION by applying every migration step
/// newer than its current version, each in its own transaction. Runs on every
/// open, but the already-current path is a cheap version check. Refuses to touch
/// a database created by a newer Grafiki build.
pub fn initialize_schema(connection: &mut Connection) -> Result<()> {
    let current = current_schema_version(connection)?;
    if current > LATEST_SCHEMA_VERSION {
        return Err(GrafikiError::SchemaVersionTooNew {
            found: current,
            supported: LATEST_SCHEMA_VERSION,
        });
    }
    if current == LATEST_SCHEMA_VERSION {
        return Ok(());
    }

    for migration in MIGRATIONS.iter().filter(|m| m.version > current) {
        let transaction = connection.transaction()?;
        transaction.execute_batch(migration.sql)?;
        transaction.execute(
            "
            INSERT OR IGNORE INTO schema_version (version, description)
            VALUES (?1, ?2)
            ",
            (migration.version, migration.description),
        )?;
        transaction.commit()?;
    }
    Ok(())
}

/// Current schema version, or 0 if the database has not been initialized yet.
fn current_schema_version(connection: &Connection) -> Result<i64> {
    let has_table: i64 = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'schema_version')",
        [],
        |row| row.get(0),
    )?;
    if has_table == 0 {
        return Ok(0);
    }
    let version: i64 = connection.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM schema_version",
        [],
        |row| row.get(0),
    )?;
    Ok(version)
}

const INITIAL_SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS schema_version (
    version     INTEGER PRIMARY KEY,
    applied_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    description TEXT NOT NULL
) STRICT;

CREATE TABLE IF NOT EXISTS entities (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    entity_type TEXT NOT NULL
        CHECK (entity_type IN (
            'person', 'service', 'file', 'module', 'concept',
            'api', 'tool', 'library', 'config', 'endpoint'
        )),
    scope       TEXT NOT NULL DEFAULT '',
    metadata    TEXT,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
) STRICT;

CREATE INDEX IF NOT EXISTS idx_entities_type ON entities(entity_type);
CREATE INDEX IF NOT EXISTS idx_entities_scope ON entities(scope);

CREATE TABLE IF NOT EXISTS observations (
    id          TEXT PRIMARY KEY,
    entity_id   TEXT NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    content     TEXT NOT NULL,
    category    TEXT NOT NULL DEFAULT 'general'
        CHECK (category IN (
            'general', 'architecture', 'decision', 'blocker',
            'pattern', 'progress', 'gotcha', 'learned',
            'preference', 'convention', 'dependency', 'risk'
        )),
    source      TEXT,
    confidence  REAL NOT NULL DEFAULT 1.0
        CHECK (confidence >= 0.0 AND confidence <= 1.0),
    valid_from  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    valid_to    TEXT,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
) STRICT;

CREATE INDEX IF NOT EXISTS idx_obs_entity ON observations(entity_id);
CREATE INDEX IF NOT EXISTS idx_obs_category ON observations(category);
CREATE INDEX IF NOT EXISTS idx_obs_valid ON observations(valid_from, valid_to);
CREATE INDEX IF NOT EXISTS idx_obs_source ON observations(source);

CREATE TABLE IF NOT EXISTS relations (
    id          TEXT PRIMARY KEY,
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
    confidence  REAL NOT NULL DEFAULT 1.0
        CHECK (confidence >= 0.0 AND confidence <= 1.0),
    source_type TEXT NOT NULL DEFAULT 'EXTRACTED'
        CHECK (source_type IN ('EXTRACTED', 'INFERRED', 'AMBIGUOUS')),
    source      TEXT,
    metadata    TEXT,
    valid_from  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    valid_to    TEXT,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),

    UNIQUE(from_entity, to_entity, relation)
) STRICT;

CREATE INDEX IF NOT EXISTS idx_rel_from ON relations(from_entity);
CREATE INDEX IF NOT EXISTS idx_rel_to ON relations(to_entity);
CREATE INDEX IF NOT EXISTS idx_rel_type ON relations(relation);
CREATE INDEX IF NOT EXISTS idx_rel_source_type ON relations(source_type);

CREATE TABLE IF NOT EXISTS decisions (
    id              TEXT PRIMARY KEY,
    title           TEXT NOT NULL,
    reasoning       TEXT,
    alternatives    TEXT,
    scope           TEXT NOT NULL DEFAULT '',
    status          TEXT NOT NULL DEFAULT 'active'
        CHECK (status IN ('active', 'superseded', 'revisit', 'revoked')),
    superseded_by   TEXT REFERENCES decisions(id),
    decided_in      TEXT,
    tags            TEXT,
    created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
) STRICT;

CREATE INDEX IF NOT EXISTS idx_dec_status ON decisions(status);
CREATE INDEX IF NOT EXISTS idx_dec_scope ON decisions(scope);

CREATE TABLE IF NOT EXISTS sessions (
    id              TEXT PRIMARY KEY,
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
    accomplishments TEXT,
    remaining       TEXT,
    files_changed   TEXT,
    decisions_made  TEXT,
    entities_touched TEXT,
    handoff_context TEXT,
    parent_session  TEXT REFERENCES sessions(id),
    child_session   TEXT,
    started_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    ended_at        TEXT
) STRICT;

CREATE INDEX IF NOT EXISTS idx_sess_status ON sessions(status);
CREATE INDEX IF NOT EXISTS idx_sess_project ON sessions(project);
CREATE INDEX IF NOT EXISTS idx_sess_type ON sessions(session_type);
CREATE INDEX IF NOT EXISTS idx_sess_parent ON sessions(parent_session);
CREATE INDEX IF NOT EXISTS idx_sess_started ON sessions(started_at);

CREATE TABLE IF NOT EXISTS state (
    id          TEXT PRIMARY KEY,
    key         TEXT NOT NULL UNIQUE,
    title       TEXT NOT NULL,
    status      TEXT NOT NULL
        CHECK (status IN (
            'planned', 'in-progress', 'blocked', 'needs-review',
            'done', 'abandoned'
        )),
    owner       TEXT,
    details     TEXT,
    blockers    TEXT,
    depends_on  TEXT,
    scope       TEXT NOT NULL DEFAULT '',
    priority    TEXT NOT NULL DEFAULT 'medium'
        CHECK (priority IN ('critical', 'high', 'medium', 'low')),
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
) STRICT;

CREATE INDEX IF NOT EXISTS idx_state_status ON state(status);
CREATE INDEX IF NOT EXISTS idx_state_priority ON state(priority);
CREATE INDEX IF NOT EXISTS idx_state_scope ON state(scope);

CREATE TABLE IF NOT EXISTS context (
    id          TEXT PRIMARY KEY,
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
    checksum    TEXT NOT NULL,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
) STRICT;

CREATE INDEX IF NOT EXISTS idx_ctx_category ON context(category);
CREATE INDEX IF NOT EXISTS idx_ctx_scope ON context(scope);

CREATE TABLE IF NOT EXISTS embedding_jobs (
    id           TEXT PRIMARY KEY,
    record_type  TEXT NOT NULL
        CHECK (record_type IN ('entity', 'observation', 'decision', 'context')),
    record_id    TEXT NOT NULL,
    scope        TEXT NOT NULL DEFAULT '',
    content_hash TEXT NOT NULL,
    status       TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'embedded', 'failed', 'skipped')),
    attempts     INTEGER NOT NULL DEFAULT 0
        CHECK (attempts >= 0),
    error        TEXT,
    created_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),

    UNIQUE(record_type, record_id, content_hash)
) STRICT;

CREATE INDEX IF NOT EXISTS idx_embedding_jobs_status ON embedding_jobs(status);
CREATE INDEX IF NOT EXISTS idx_embedding_jobs_scope ON embedding_jobs(scope);
CREATE INDEX IF NOT EXISTS idx_embedding_jobs_record ON embedding_jobs(record_type, record_id);

CREATE TABLE IF NOT EXISTS embedding_vectors (
    record_type  TEXT NOT NULL
        CHECK (record_type IN ('entity', 'observation', 'decision', 'context')),
    record_id    TEXT NOT NULL,
    scope        TEXT NOT NULL DEFAULT '',
    provider     TEXT NOT NULL,
    model        TEXT NOT NULL,
    dimension    INTEGER NOT NULL
        CHECK (dimension > 0),
    content_hash TEXT NOT NULL,
    embedding    TEXT NOT NULL,
    updated_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),

    PRIMARY KEY(record_type, record_id, provider, model)
) STRICT;

CREATE INDEX IF NOT EXISTS idx_embedding_vectors_scope ON embedding_vectors(scope);
CREATE INDEX IF NOT EXISTS idx_embedding_vectors_model ON embedding_vectors(provider, model, dimension);

CREATE TABLE IF NOT EXISTS embedding_metadata (
    record_type  TEXT NOT NULL
        CHECK (record_type IN ('entity', 'observation', 'decision', 'context')),
    record_id    TEXT NOT NULL,
    provider     TEXT NOT NULL,
    model        TEXT NOT NULL,
    dimension    INTEGER NOT NULL
        CHECK (dimension > 0),
    content_hash TEXT NOT NULL,
    embedded_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),

    PRIMARY KEY(record_type, record_id, provider, model)
) STRICT;

CREATE INDEX IF NOT EXISTS idx_embedding_metadata_model ON embedding_metadata(provider, model);

CREATE TABLE IF NOT EXISTS events (
    id          TEXT PRIMARY KEY,
    event_type  TEXT NOT NULL
        CHECK (event_type IN (
            'entity_created', 'entity_updated',
            'observation_added', 'observation_invalidated',
            'relation_created', 'relation_removed',
            'decision_logged', 'decision_superseded',
            'state_changed',
            'session_started', 'session_ended', 'session_handoff',
            'context_added', 'context_updated', 'context_deleted'
        )),
    source_session TEXT,
    target_type TEXT NOT NULL,
    target_id   TEXT NOT NULL,
    scope       TEXT NOT NULL DEFAULT '',
    summary     TEXT NOT NULL,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
) STRICT;

CREATE INDEX IF NOT EXISTS idx_events_type ON events(event_type);
CREATE INDEX IF NOT EXISTS idx_events_scope ON events(scope);
CREATE INDEX IF NOT EXISTS idx_events_created ON events(created_at);
CREATE INDEX IF NOT EXISTS idx_events_session ON events(source_session);

CREATE TABLE IF NOT EXISTS capture_sessions (
    id                TEXT PRIMARY KEY,
    project           TEXT NOT NULL,
    scope             TEXT NOT NULL DEFAULT '',
    status            TEXT NOT NULL DEFAULT 'active'
        CHECK (status IN ('active', 'stopped')),
    source_app        TEXT,
    consent_profile   TEXT NOT NULL DEFAULT 'local-explicit',
    redaction_profile TEXT NOT NULL DEFAULT 'default',
    started_at        TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    ended_at          TEXT
) STRICT;

CREATE INDEX IF NOT EXISTS idx_capture_sessions_project ON capture_sessions(project);
CREATE INDEX IF NOT EXISTS idx_capture_sessions_scope ON capture_sessions(scope);
CREATE INDEX IF NOT EXISTS idx_capture_sessions_status ON capture_sessions(status);
CREATE INDEX IF NOT EXISTS idx_capture_sessions_started ON capture_sessions(started_at);

CREATE TABLE IF NOT EXISTS capture_events (
    id              TEXT PRIMARY KEY,
    capture_session TEXT NOT NULL REFERENCES capture_sessions(id) ON DELETE CASCADE,
    source_type     TEXT NOT NULL
        CHECK (source_type IN (
            'transcript', 'screen', 'ide', 'file', 'terminal',
            'browser', 'agent', 'system', 'git'
        )),
    source          TEXT,
    title           TEXT,
    text            TEXT,
    payload         TEXT,
    metadata        TEXT,
    privacy_level   TEXT NOT NULL DEFAULT 'internal'
        CHECK (privacy_level IN ('public', 'internal', 'sensitive', 'secret')),
    redacted        INTEGER NOT NULL DEFAULT 0
        CHECK (redacted IN (0, 1)),
    scope           TEXT NOT NULL DEFAULT '',
    captured_at     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
) STRICT;

CREATE INDEX IF NOT EXISTS idx_capture_events_session ON capture_events(capture_session);
CREATE INDEX IF NOT EXISTS idx_capture_events_source_type ON capture_events(source_type);
CREATE INDEX IF NOT EXISTS idx_capture_events_scope ON capture_events(scope);
CREATE INDEX IF NOT EXISTS idx_capture_events_captured ON capture_events(captured_at);

CREATE TABLE IF NOT EXISTS extraction_candidates (
    id                  TEXT PRIMARY KEY,
    source_type         TEXT NOT NULL,
    source              TEXT,
    proposed_record_type TEXT NOT NULL
        CHECK (proposed_record_type IN ('entity', 'observation', 'decision', 'context', 'state')),
    payload             TEXT NOT NULL,
    scope               TEXT NOT NULL DEFAULT '',
    confidence          REAL NOT NULL DEFAULT 0.5
        CHECK (confidence >= 0.0 AND confidence <= 1.0),
    status              TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'approved', 'rejected')),
    rationale           TEXT,
    trusted_record_type TEXT,
    trusted_record_id   TEXT,
    created_at          TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    reviewed_at         TEXT
) STRICT;

CREATE INDEX IF NOT EXISTS idx_candidates_status ON extraction_candidates(status);
CREATE INDEX IF NOT EXISTS idx_candidates_scope ON extraction_candidates(scope);
CREATE INDEX IF NOT EXISTS idx_candidates_source ON extraction_candidates(source_type, source);
CREATE INDEX IF NOT EXISTS idx_candidates_created ON extraction_candidates(created_at);

CREATE TABLE IF NOT EXISTS evidence_links (
    id                TEXT PRIMARY KEY,
    candidate_id      TEXT REFERENCES extraction_candidates(id) ON DELETE CASCADE,
    trusted_record_type TEXT,
    trusted_record_id TEXT,
    source_event_id   TEXT REFERENCES capture_events(id) ON DELETE SET NULL,
    source_type       TEXT NOT NULL,
    source            TEXT,
    title             TEXT,
    excerpt           TEXT NOT NULL DEFAULT '',
    uri               TEXT,
    byte_start        INTEGER,
    byte_end          INTEGER,
    line_start        INTEGER,
    line_end          INTEGER,
    captured_at       TEXT,
    created_at        TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
) STRICT;

CREATE INDEX IF NOT EXISTS idx_evidence_candidate ON evidence_links(candidate_id);
CREATE INDEX IF NOT EXISTS idx_evidence_trusted ON evidence_links(trusted_record_type, trusted_record_id);
CREATE INDEX IF NOT EXISTS idx_evidence_event ON evidence_links(source_event_id);
CREATE INDEX IF NOT EXISTS idx_evidence_source ON evidence_links(source_type, source);

CREATE TABLE IF NOT EXISTS agent_queries (
    id             TEXT PRIMARY KEY,
    agent          TEXT NOT NULL DEFAULT 'unknown',
    question       TEXT NOT NULL,
    scope          TEXT NOT NULL DEFAULT '',
    returned_ids   TEXT NOT NULL DEFAULT '[]',
    retrieval_mode TEXT NOT NULL DEFAULT 'hybrid',
    fallback       TEXT,
    latency_ms     INTEGER NOT NULL DEFAULT 0,
    created_at     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
) STRICT;

CREATE INDEX IF NOT EXISTS idx_agent_queries_agent ON agent_queries(agent);
CREATE INDEX IF NOT EXISTS idx_agent_queries_scope ON agent_queries(scope);
CREATE INDEX IF NOT EXISTS idx_agent_queries_created ON agent_queries(created_at);

CREATE TABLE IF NOT EXISTS briefing_cache (
    scope           TEXT PRIMARY KEY,
    briefing        TEXT NOT NULL,
    generated_at    TEXT NOT NULL,
    events_cursor   TEXT NOT NULL
) STRICT;

CREATE VIRTUAL TABLE IF NOT EXISTS observations_fts USING fts5(
    content,
    content='observations',
    content_rowid='rowid',
    tokenize='porter unicode61'
);

CREATE VIRTUAL TABLE IF NOT EXISTS decisions_fts USING fts5(
    title,
    reasoning,
    content='decisions',
    content_rowid='rowid',
    tokenize='porter unicode61'
);

CREATE VIRTUAL TABLE IF NOT EXISTS context_fts USING fts5(
    title,
    content,
    content='context',
    content_rowid='rowid',
    tokenize='porter unicode61'
);

CREATE TRIGGER IF NOT EXISTS observations_ai AFTER INSERT ON observations BEGIN
    INSERT INTO observations_fts(rowid, content) VALUES (new.rowid, new.content);
END;

CREATE TRIGGER IF NOT EXISTS observations_ad AFTER DELETE ON observations BEGIN
    INSERT INTO observations_fts(observations_fts, rowid, content)
    VALUES ('delete', old.rowid, old.content);
END;

CREATE TRIGGER IF NOT EXISTS observations_au AFTER UPDATE ON observations BEGIN
    INSERT INTO observations_fts(observations_fts, rowid, content)
    VALUES ('delete', old.rowid, old.content);
    INSERT INTO observations_fts(rowid, content) VALUES (new.rowid, new.content);
END;

CREATE TRIGGER IF NOT EXISTS decisions_ai AFTER INSERT ON decisions BEGIN
    INSERT INTO decisions_fts(rowid, title, reasoning)
    VALUES (new.rowid, new.title, new.reasoning);
END;

CREATE TRIGGER IF NOT EXISTS decisions_ad AFTER DELETE ON decisions BEGIN
    INSERT INTO decisions_fts(decisions_fts, rowid, title, reasoning)
    VALUES ('delete', old.rowid, old.title, old.reasoning);
END;

CREATE TRIGGER IF NOT EXISTS decisions_au AFTER UPDATE ON decisions BEGIN
    INSERT INTO decisions_fts(decisions_fts, rowid, title, reasoning)
    VALUES ('delete', old.rowid, old.title, old.reasoning);
    INSERT INTO decisions_fts(rowid, title, reasoning)
    VALUES (new.rowid, new.title, new.reasoning);
END;

CREATE TRIGGER IF NOT EXISTS context_ai AFTER INSERT ON context BEGIN
    INSERT INTO context_fts(rowid, title, content)
    VALUES (new.rowid, new.title, new.content);
END;

CREATE TRIGGER IF NOT EXISTS context_ad AFTER DELETE ON context BEGIN
    INSERT INTO context_fts(context_fts, rowid, title, content)
    VALUES ('delete', old.rowid, old.title, old.content);
END;

CREATE TRIGGER IF NOT EXISTS context_au AFTER UPDATE ON context BEGIN
    INSERT INTO context_fts(context_fts, rowid, title, content)
    VALUES ('delete', old.rowid, old.title, old.content);
    INSERT INTO context_fts(rowid, title, content)
    VALUES (new.rowid, new.title, new.content);
END;
"#;

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use super::initialize_schema;

    #[test]
    fn initializes_schema_version() {
        let mut connection = Connection::open_in_memory().unwrap();

        initialize_schema(&mut connection).unwrap();

        let version: i64 = connection
            .query_row("SELECT version FROM schema_version", [], |row| row.get(0))
            .unwrap();

        assert_eq!(version, 1);
    }

    #[test]
    fn migration_runner_is_idempotent_and_refuses_newer_versions() {
        let mut connection = Connection::open_in_memory().unwrap();
        initialize_schema(&mut connection).unwrap();
        // Re-running is a cheap no-op (already at the latest version).
        initialize_schema(&mut connection).unwrap();
        let version: i64 = connection
            .query_row("SELECT MAX(version) FROM schema_version", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(version, super::LATEST_SCHEMA_VERSION);

        // A database written by a newer build must be refused, not silently used.
        connection
            .execute(
                "INSERT INTO schema_version (version, description) VALUES (?1, 'future')",
                [super::LATEST_SCHEMA_VERSION + 1],
            )
            .unwrap();
        let err = initialize_schema(&mut connection).unwrap_err();
        assert!(matches!(
            err,
            crate::error::GrafikiError::SchemaVersionTooNew { .. }
        ));
    }

    #[test]
    fn creates_relations_with_confidence_and_source_type() {
        let mut connection = Connection::open_in_memory().unwrap();

        initialize_schema(&mut connection).unwrap();

        let column_count: i64 = connection
            .query_row(
                "
                SELECT COUNT(*)
                FROM pragma_table_info('relations')
                WHERE name IN ('confidence', 'source_type', 'source')
                ",
                [],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(column_count, 3);
    }

    #[test]
    fn creates_embedding_job_tables() {
        let mut connection = Connection::open_in_memory().unwrap();

        initialize_schema(&mut connection).unwrap();

        let job_table: i64 = connection
            .query_row(
                "
                SELECT COUNT(*)
                FROM sqlite_schema
                WHERE type = 'table' AND name = 'embedding_jobs'
                ",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let vector_table: i64 = connection
            .query_row(
                "
                SELECT COUNT(*)
                FROM sqlite_schema
                WHERE type = 'table' AND name = 'embedding_vectors'
                ",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let metadata_table: i64 = connection
            .query_row(
                "
                SELECT COUNT(*)
                FROM sqlite_schema
                WHERE type = 'table' AND name = 'embedding_metadata'
                ",
                [],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(job_table, 1);
        assert_eq!(vector_table, 1);
        assert_eq!(metadata_table, 1);
    }

    #[test]
    fn creates_extraction_candidate_review_table() {
        let mut connection = Connection::open_in_memory().unwrap();

        initialize_schema(&mut connection).unwrap();

        let candidate_table: i64 = connection
            .query_row(
                "
                SELECT COUNT(*)
                FROM sqlite_schema
                WHERE type = 'table' AND name = 'extraction_candidates'
                ",
                [],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(candidate_table, 1);
    }

    #[test]
    fn creates_capture_ledger_tables() {
        let mut connection = Connection::open_in_memory().unwrap();

        initialize_schema(&mut connection).unwrap();

        let session_table: i64 = connection
            .query_row(
                "
                SELECT COUNT(*)
                FROM sqlite_schema
                WHERE type = 'table' AND name = 'capture_sessions'
                ",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let event_table: i64 = connection
            .query_row(
                "
                SELECT COUNT(*)
                FROM sqlite_schema
                WHERE type = 'table' AND name = 'capture_events'
                ",
                [],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(session_table, 1);
        assert_eq!(event_table, 1);
    }

    #[test]
    fn fts_triggers_index_inserted_observations() {
        let mut connection = Connection::open_in_memory().unwrap();

        initialize_schema(&mut connection).unwrap();
        connection
            .execute(
                "
                INSERT INTO entities (id, name, entity_type)
                VALUES ('auth-service', 'Auth Service', 'service')
                ",
                [],
            )
            .unwrap();
        connection
            .execute(
                "
                INSERT INTO observations (id, entity_id, content)
                VALUES ('01K00000000000000000000000', 'auth-service', 'JWT refresh uses rotating tokens')
                ",
                [],
            )
            .unwrap();

        let matches: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM observations_fts WHERE observations_fts MATCH 'rotating'",
                [],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(matches, 1);
    }
}
