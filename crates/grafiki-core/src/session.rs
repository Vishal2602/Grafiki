use std::path::PathBuf;

use rusqlite::{params, params_from_iter, Row, Transaction};
use serde::{Deserialize, Serialize};

use crate::db::{open_project_database, schema::initialize_schema};
use crate::project::{resolve_project, ProjectResolveOptions};
use crate::scope::Scope;
use crate::ulid::new_ulid;
use crate::{GrafikiError, Result};

#[derive(Debug, Clone)]
pub struct StartSessionOptions {
    pub project_name: Option<String>,
    pub start_dir: PathBuf,
    pub grafiki_home: Option<PathBuf>,
    pub session_type: String,
    pub goal: String,
    pub scope: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StartSessionReport {
    pub session_id: String,
    pub project: String,
    pub project_dir: PathBuf,
    pub db_path: PathBuf,
    pub session_type: String,
    pub goal: String,
    pub scope: String,
    pub scope_chain: Vec<String>,
    pub briefing: String,
}

pub fn start_session(options: StartSessionOptions) -> Result<StartSessionReport> {
    let session_type = validate_session_type(options.session_type.trim())?;
    let goal = options.goal.trim().to_owned();
    let scope = Scope::new(options.scope)?;
    let scope_chain = scope.chain().into_vec();
    let project = resolve_project(ProjectResolveOptions {
        project_name: options.project_name,
        start_dir: options.start_dir,
        grafiki_home: options.grafiki_home,
    })?;

    let mut connection = open_project_database(&project.db_path)?;
    initialize_schema(&mut connection)?;

    let session_id = new_ulid();
    let event_id = new_ulid();
    let tx = connection.transaction()?;

    tx.execute(
        "
        INSERT INTO sessions (id, session_type, project, scope, goal)
        VALUES (?1, ?2, ?3, ?4, ?5)
        ",
        params![
            session_id,
            session_type,
            project.project,
            scope.as_str(),
            goal
        ],
    )?;

    tx.execute(
        "
        INSERT INTO events (id, event_type, source_session, target_type, target_id, scope, summary)
        VALUES (?1, 'session_started', ?2, 'session', ?2, ?3, ?4)
        ",
        params![
            event_id,
            session_id,
            scope.as_str(),
            format!("Started {session_type} session: {goal}")
        ],
    )?;

    let briefing = generate_template_briefing(
        &tx,
        &project.project,
        &session_id,
        &session_type,
        &goal,
        scope.as_str(),
        &scope_chain,
    )?;

    tx.commit()?;

    Ok(StartSessionReport {
        session_id,
        project: project.project,
        project_dir: project.project_dir,
        db_path: project.db_path,
        session_type,
        goal,
        scope: scope.as_str().to_owned(),
        scope_chain,
        briefing,
    })
}

fn validate_session_type(raw: &str) -> Result<String> {
    const SESSION_TYPES: &[&str] = &[
        "claude-code",
        "claude-ai",
        "co-work",
        "cursor",
        "copilot",
        "windsurf",
        "cline",
        "codex",
        "aider",
        "other",
    ];

    if SESSION_TYPES.contains(&raw) {
        Ok(raw.to_owned())
    } else {
        Err(GrafikiError::InvalidSessionType(raw.to_owned()))
    }
}

fn generate_template_briefing(
    connection: &Transaction<'_>,
    project: &str,
    session_id: &str,
    session_type: &str,
    goal: &str,
    scope: &str,
    scope_chain: &[String],
) -> Result<String> {
    let decisions = query_active_decisions(connection, scope_chain)?;
    let state_items = query_active_state(connection, scope_chain)?;
    let recent_sessions = query_recent_sessions(connection, scope_chain, session_id)?;
    let observations = query_recent_observations(connection, scope_chain)?;
    let events = query_recent_events(connection, scope_chain)?;

    let mut briefing = String::new();
    briefing.push_str("# Grafiki Briefing\n\n");
    briefing.push_str(&format!("- Project: {project}\n"));
    briefing.push_str(&format!("- Session: {session_id}\n"));
    briefing.push_str(&format!("- Tool: {session_type}\n"));
    briefing.push_str(&format!(
        "- Scope: {}\n",
        if scope.is_empty() { "global" } else { scope }
    ));
    briefing.push_str(&format!("- Goal: {goal}\n\n"));

    push_section(&mut briefing, "Active Decisions", &decisions);
    push_section(&mut briefing, "Active Work", &state_items);
    push_section(&mut briefing, "Recent Sessions", &recent_sessions);
    push_section(&mut briefing, "Recent Observations", &observations);
    push_section(&mut briefing, "Recent Events", &events);

    briefing
        .push_str("\nUse this briefing as the project memory starting point for the session.\n");
    Ok(briefing)
}

fn push_section(briefing: &mut String, title: &str, items: &[String]) {
    briefing.push_str(&format!("## {title}\n"));

    if items.is_empty() {
        briefing.push_str("- None yet.\n\n");
        return;
    }

    for item in items {
        briefing.push_str("- ");
        briefing.push_str(item);
        briefing.push('\n');
    }
    briefing.push('\n');
}

fn query_active_decisions(
    connection: &Transaction<'_>,
    scope_chain: &[String],
) -> Result<Vec<String>> {
    query_scoped_rows(
        connection,
        "
        SELECT title, scope
        FROM decisions
        WHERE status = 'active' AND scope IN ({scopes})
        ORDER BY created_at DESC
        LIMIT 10
        ",
        scope_chain,
        |row| {
            let title: String = row.get(0)?;
            let scope: String = row.get(1)?;
            Ok(format!("{title} [{}]", display_scope(&scope)))
        },
    )
}

fn query_active_state(connection: &Transaction<'_>, scope_chain: &[String]) -> Result<Vec<String>> {
    query_scoped_rows(
        connection,
        "
        SELECT key, title, status, priority, owner, scope
        FROM state
        WHERE status IN ('in-progress', 'blocked', 'needs-review') AND scope IN ({scopes})
        ORDER BY updated_at DESC
        LIMIT 10
        ",
        scope_chain,
        |row| {
            let key: String = row.get(0)?;
            let title: String = row.get(1)?;
            let status: String = row.get(2)?;
            let priority: String = row.get(3)?;
            let owner: Option<String> = row.get(4)?;
            let scope: String = row.get(5)?;
            let owner = owner.unwrap_or_else(|| "unassigned".to_owned());
            Ok(format!(
                "{key}: {title} ({status}, {priority}, {owner}) [{}]",
                display_scope(&scope)
            ))
        },
    )
}

fn query_recent_sessions(
    connection: &Transaction<'_>,
    scope_chain: &[String],
    current_session_id: &str,
) -> Result<Vec<String>> {
    let sql = scoped_query(
        "
        SELECT session_type, goal, summary, scope
        FROM sessions
        WHERE status IN ('completed', 'handed-off') AND id != ? AND scope IN ({scopes})
        ORDER BY ended_at DESC, started_at DESC
        LIMIT 5
        ",
        scope_chain.len(),
    );
    let mut statement = connection.prepare(&sql)?;
    let params = std::iter::once(current_session_id).chain(scope_chain.iter().map(String::as_str));
    let rows = statement.query_map(params_from_iter(params), |row| {
        let session_type: String = row.get(0)?;
        let goal: Option<String> = row.get(1)?;
        let summary: Option<String> = row.get(2)?;
        let scope: String = row.get(3)?;
        let headline = summary.or(goal).unwrap_or_else(|| "No summary".to_owned());
        Ok(format!(
            "{session_type}: {headline} [{}]",
            display_scope(&scope)
        ))
    })?;

    collect_rows(rows)
}

fn query_recent_observations(
    connection: &Transaction<'_>,
    scope_chain: &[String],
) -> Result<Vec<String>> {
    query_scoped_rows(
        connection,
        "
        SELECT e.id, o.category, o.content, e.scope
        FROM observations o
        JOIN entities e ON e.id = o.entity_id
        WHERE o.valid_to IS NULL AND e.scope IN ({scopes})
        ORDER BY o.created_at DESC
        LIMIT 15
        ",
        scope_chain,
        |row| {
            let entity_id: String = row.get(0)?;
            let category: String = row.get(1)?;
            let content: String = row.get(2)?;
            let scope: String = row.get(3)?;
            Ok(format!(
                "{entity_id} ({category}): {content} [{}]",
                display_scope(&scope)
            ))
        },
    )
}

fn query_recent_events(
    connection: &Transaction<'_>,
    scope_chain: &[String],
) -> Result<Vec<String>> {
    query_scoped_rows(
        connection,
        "
        SELECT summary, scope
        FROM events
        WHERE scope IN ({scopes})
        ORDER BY created_at DESC
        LIMIT 5
        ",
        scope_chain,
        |row| {
            let summary: String = row.get(0)?;
            let scope: String = row.get(1)?;
            Ok(format!("{summary} [{}]", display_scope(&scope)))
        },
    )
}

fn query_scoped_rows<T, F>(
    connection: &Transaction<'_>,
    template: &str,
    scope_chain: &[String],
    mapper: F,
) -> Result<Vec<T>>
where
    F: FnMut(&Row<'_>) -> rusqlite::Result<T>,
{
    let sql = scoped_query(template, scope_chain.len());
    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map(params_from_iter(scope_chain.iter()), mapper)?;

    collect_rows(rows)
}

fn collect_rows<T>(
    rows: rusqlite::MappedRows<'_, impl FnMut(&Row<'_>) -> rusqlite::Result<T>>,
) -> Result<Vec<T>> {
    let mut items = Vec::new();
    for row in rows {
        items.push(row?);
    }
    Ok(items)
}

fn scoped_query(template: &str, scope_count: usize) -> String {
    template.replace("{scopes}", &placeholders(scope_count))
}

fn placeholders(count: usize) -> String {
    std::iter::repeat_n("?", count)
        .collect::<Vec<_>>()
        .join(", ")
}

fn display_scope(scope: &str) -> &str {
    if scope.is_empty() {
        "global"
    } else {
        scope
    }
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use crate::db::schema::initialize_schema;
    use crate::project::{init_project, InitOptions};

    use super::{start_session, StartSessionOptions};

    #[test]
    fn start_session_creates_session_and_event() {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().join("home");
        let project_dir = temp.path().join("example-project");

        init_project(InitOptions {
            project_name: None,
            project_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
        })
        .unwrap();

        let report = start_session(StartSessionOptions {
            project_name: None,
            start_dir: project_dir,
            grafiki_home: Some(home.clone()),
            session_type: "codex".to_owned(),
            goal: "Implement session start".to_owned(),
            scope: "example-project/backend".to_owned(),
        })
        .unwrap();

        let connection = Connection::open(home.join("example-project.db")).unwrap();
        let session_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sessions WHERE id = ?1",
                [&report.session_id],
                |row| row.get(0),
            )
            .unwrap();
        let event_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM events WHERE event_type = 'session_started' AND source_session = ?1",
                [&report.session_id],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(session_count, 1);
        assert_eq!(event_count, 1);
        assert!(report.briefing.contains("Implement session start"));
    }

    #[test]
    fn start_session_briefing_uses_scope_chain() {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().join("home");
        let project_dir = temp.path().join("example-project");

        init_project(InitOptions {
            project_name: None,
            project_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
        })
        .unwrap();

        let mut connection = Connection::open(home.join("example-project.db")).unwrap();
        initialize_schema(&mut connection).unwrap();
        connection
            .execute(
                "INSERT INTO decisions (id, title, scope) VALUES (?1, ?2, ?3)",
                [
                    "01K00000000000000000000001",
                    "Use SQLite WAL",
                    "example-project/backend",
                ],
            )
            .unwrap();

        let report = start_session(StartSessionOptions {
            project_name: None,
            start_dir: project_dir,
            grafiki_home: Some(home),
            session_type: "codex".to_owned(),
            goal: "Work on backend".to_owned(),
            scope: "example-project/backend/api".to_owned(),
        })
        .unwrap();

        assert!(report.briefing.contains("Use SQLite WAL"));
        assert_eq!(
            report.scope_chain,
            vec![
                "",
                "example-project",
                "example-project/backend",
                "example-project/backend/api"
            ]
        );
    }
}
