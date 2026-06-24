use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::memory::{
    ingest_capture_event, propose_capture_candidates, start_capture_session, stop_capture_session,
    CaptureCandidateReport, IngestCaptureEventOptions, ProposeCaptureCandidatesOptions,
    StartCaptureOptions, StopCaptureOptions,
};
use crate::{GrafikiError, Result};

#[derive(Debug, Clone)]
pub struct ImportAgentTranscriptsOptions {
    pub project_name: Option<String>,
    pub start_dir: PathBuf,
    pub grafiki_home: Option<PathBuf>,
    pub agent: String,
    pub input: Option<PathBuf>,
    pub scope: String,
    pub limit: usize,
    pub summarize: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TranscriptImportSource {
    pub path: String,
    pub events: usize,
    pub skipped: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentTranscriptImportReport {
    pub agent: String,
    pub scope: String,
    pub capture_id: String,
    pub files_scanned: usize,
    pub files_imported: usize,
    pub events_imported: usize,
    pub sources: Vec<TranscriptImportSource>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub candidates: Option<CaptureCandidateReport>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedTranscriptEvent {
    role: String,
    title: String,
    text: String,
    timestamp: Option<String>,
    kind: String,
}

pub fn import_agent_transcripts(
    options: ImportAgentTranscriptsOptions,
) -> Result<AgentTranscriptImportReport> {
    let agent = normalize_agent(&options.agent)?;
    let limit = options.limit.clamp(1, 1000);
    let files = transcript_input_files(&agent, options.input.as_deref(), &options.start_dir)?;

    let capture = start_capture_session(StartCaptureOptions {
        project_name: options.project_name.clone(),
        start_dir: options.start_dir.clone(),
        grafiki_home: options.grafiki_home.clone(),
        scope: options.scope.clone(),
        source_app: Some(agent.clone()),
        consent_profile: Some("transcript-import".to_owned()),
        redaction_profile: Some("default".to_owned()),
    })?;
    let capture_id = capture.capture.id;

    let mut remaining = limit;
    let mut files_scanned = 0usize;
    let mut files_imported = 0usize;
    let mut events_imported = 0usize;
    let mut sources = Vec::new();

    for file in files {
        if remaining == 0 {
            break;
        }
        files_scanned += 1;
        let parsed = match parse_transcript_file(&agent, &file, remaining) {
            Ok(events) => events,
            Err(error) => {
                sources.push(TranscriptImportSource {
                    path: file.display().to_string(),
                    events: 0,
                    skipped: Some(error.to_string()),
                });
                continue;
            }
        };
        if parsed.is_empty() {
            sources.push(TranscriptImportSource {
                path: file.display().to_string(),
                events: 0,
                skipped: Some("no importable transcript messages found".to_owned()),
            });
            continue;
        }

        let mut imported_for_file = 0usize;
        for (index, event) in parsed.into_iter().enumerate() {
            ingest_capture_event(IngestCaptureEventOptions {
                project_name: options.project_name.clone(),
                start_dir: options.start_dir.clone(),
                grafiki_home: options.grafiki_home.clone(),
                capture_id: Some(capture_id.clone()),
                scope: options.scope.clone(),
                source_type: "transcript".to_owned(),
                source: Some(format!("{}:{}", agent, file.display())),
                title: Some(event.title.clone()),
                text: Some(event.text.clone()),
                payload: None,
                metadata: Some(serde_json::json!({
                    "agent": agent.as_str(),
                    "role": event.role.as_str(),
                    "kind": event.kind.as_str(),
                    "path": file.display().to_string(),
                    "index": index,
                })),
                privacy_level: Some("internal".to_owned()),
                redacted: false,
                captured_at: event.timestamp,
            })?;
            imported_for_file += 1;
            events_imported += 1;
            remaining = remaining.saturating_sub(1);
            if remaining == 0 {
                break;
            }
        }

        files_imported += 1;
        sources.push(TranscriptImportSource {
            path: file.display().to_string(),
            events: imported_for_file,
            skipped: None,
        });
    }

    stop_capture_session(StopCaptureOptions {
        project_name: options.project_name.clone(),
        start_dir: options.start_dir.clone(),
        grafiki_home: options.grafiki_home.clone(),
        capture_id: capture_id.clone(),
    })?;

    let candidates = if options.summarize && events_imported > 0 {
        Some(propose_capture_candidates(
            ProposeCaptureCandidatesOptions {
                project_name: options.project_name,
                start_dir: options.start_dir,
                grafiki_home: options.grafiki_home,
                capture_id: Some(capture_id.clone()),
                scope: options.scope.clone(),
                limit: events_imported.min(100),
            },
        )?)
    } else {
        None
    };

    let message = if events_imported == 0 {
        format!("No {agent} transcript events were imported.")
    } else {
        format!("Imported {events_imported} {agent} transcript events into raw capture.")
    };

    Ok(AgentTranscriptImportReport {
        agent,
        scope: options.scope,
        capture_id,
        files_scanned,
        files_imported,
        events_imported,
        sources,
        candidates,
        message,
    })
}

fn normalize_agent(raw: &str) -> Result<String> {
    let normalized = raw.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "codex" | "openai-codex" | "openai_codex" => Ok("codex".to_owned()),
        "claude" | "claude-code" | "claude_code" => Ok("claude-code".to_owned()),
        "cursor" => Ok("cursor".to_owned()),
        "generic" | "jsonl" | "json" | "markdown" | "md" | "text" | "txt" => {
            Ok("generic".to_owned())
        }
        _ => Err(GrafikiError::InvalidCandidate(format!(
            "unsupported transcript agent: {raw}"
        ))),
    }
}

fn transcript_input_files(
    agent: &str,
    input: Option<&Path>,
    start_dir: &Path,
) -> Result<Vec<PathBuf>> {
    let roots = match input {
        Some(path) => vec![path.to_path_buf()],
        None => default_transcript_roots(agent, start_dir),
    };

    let mut files = Vec::new();
    for root in roots {
        collect_transcript_files(&root, agent, &mut files, 0)?;
    }
    files.sort_by(|left, right| {
        file_modified(right)
            .cmp(&file_modified(left))
            .then_with(|| left.cmp(right))
    });
    files.truncate(120);
    Ok(files)
}

fn default_transcript_roots(agent: &str, start_dir: &Path) -> Vec<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    match (agent, home) {
        ("codex", Some(home)) => vec![home.join(".codex").join("sessions")],
        ("claude-code", Some(home)) => vec![home.join(".claude").join("projects")],
        ("cursor", Some(home)) => vec![
            start_dir.join(".cursor"),
            home.join("Library")
                .join("Application Support")
                .join("Cursor")
                .join("User")
                .join("workspaceStorage"),
        ],
        _ => vec![start_dir.to_path_buf()],
    }
}

fn collect_transcript_files(
    path: &Path,
    agent: &str,
    files: &mut Vec<PathBuf>,
    depth: usize,
) -> Result<()> {
    if files.len() >= 120 || depth > 8 || !path.exists() {
        return Ok(());
    }
    if path.is_file() {
        if is_transcript_file(path, agent) {
            files.push(path.to_path_buf());
        }
        return Ok(());
    }
    if should_skip_dir(path) {
        return Ok(());
    }

    let mut entries = fs::read_dir(path)?.collect::<std::result::Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| entry.path());
    for entry in entries {
        if files.len() >= 120 {
            break;
        }
        collect_transcript_files(&entry.path(), agent, files, depth + 1)?;
    }
    Ok(())
}

fn should_skip_dir(path: &Path) -> bool {
    matches!(
        path.file_name().and_then(|name| name.to_str()),
        Some(".git" | "node_modules" | "target" | "CacheStorage" | "cache")
    )
}

fn is_transcript_file(path: &Path, agent: &str) -> bool {
    let Some(extension) = path.extension().and_then(|value| value.to_str()) else {
        return false;
    };
    let extension = extension.to_ascii_lowercase();
    match agent {
        "codex" | "claude-code" => extension == "jsonl",
        "cursor" => matches!(extension.as_str(), "jsonl" | "json" | "md" | "txt"),
        _ => matches!(extension.as_str(), "jsonl" | "json" | "md" | "txt"),
    }
}

fn file_modified(path: &Path) -> std::time::SystemTime {
    path.metadata()
        .and_then(|metadata| metadata.modified())
        .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
}

fn parse_transcript_file(
    agent: &str,
    path: &Path,
    limit: usize,
) -> Result<Vec<ParsedTranscriptEvent>> {
    let raw = fs::read_to_string(path)?;
    if raw.trim().is_empty() {
        return Ok(Vec::new());
    }

    match path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "jsonl" => parse_jsonl_transcript(agent, &raw, limit),
        "json" => parse_json_transcript(agent, &raw, limit),
        "md" | "txt" => Ok(parse_text_transcript(agent, &raw, limit)),
        _ => Ok(Vec::new()),
    }
}

fn parse_jsonl_transcript(
    agent: &str,
    raw: &str,
    limit: usize,
) -> Result<Vec<ParsedTranscriptEvent>> {
    let mut events = Vec::new();
    for line in raw.lines().filter(|line| !line.trim().is_empty()) {
        if events.len() >= limit {
            break;
        }
        let value: Value = serde_json::from_str(line)?;
        if let Some(event) = parse_json_value_event(agent, &value) {
            events.push(event);
        }
    }
    Ok(events)
}

fn parse_json_transcript(
    agent: &str,
    raw: &str,
    limit: usize,
) -> Result<Vec<ParsedTranscriptEvent>> {
    let value: Value = serde_json::from_str(raw)?;
    let mut events = Vec::new();
    collect_json_events(agent, &value, &mut events, limit);
    Ok(events)
}

fn collect_json_events(
    agent: &str,
    value: &Value,
    events: &mut Vec<ParsedTranscriptEvent>,
    limit: usize,
) {
    if events.len() >= limit {
        return;
    }
    if let Some(event) = parse_json_value_event(agent, value) {
        events.push(event);
        return;
    }
    match value {
        Value::Array(items) => {
            for item in items {
                collect_json_events(agent, item, events, limit);
                if events.len() >= limit {
                    break;
                }
            }
        }
        Value::Object(map) => {
            for key in [
                "messages",
                "conversation",
                "items",
                "events",
                "history",
                "turns",
            ] {
                if let Some(child) = map.get(key) {
                    collect_json_events(agent, child, events, limit);
                }
                if events.len() >= limit {
                    break;
                }
            }
        }
        _ => {}
    }
}

fn parse_text_transcript(agent: &str, raw: &str, limit: usize) -> Vec<ParsedTranscriptEvent> {
    let mut events = Vec::new();
    for (index, chunk) in raw
        .split("\n\n")
        .map(str::trim)
        .filter(|chunk| !chunk.is_empty())
        .enumerate()
    {
        if events.len() >= limit {
            break;
        }
        let (role, text) = split_role_prefix(chunk).unwrap_or(("unknown", chunk));
        events.push(ParsedTranscriptEvent {
            role: role.to_owned(),
            title: format!("{agent} transcript note {}", index + 1),
            text: truncate_event_text(text),
            timestamp: None,
            kind: "text".to_owned(),
        });
    }
    events
}

fn parse_json_value_event(agent: &str, value: &Value) -> Option<ParsedTranscriptEvent> {
    match agent {
        "codex" => parse_codex_event(value),
        "claude-code" => parse_claude_event(value),
        "cursor" => parse_generic_json_event("cursor", value),
        _ => parse_generic_json_event("generic", value),
    }
}

fn parse_codex_event(value: &Value) -> Option<ParsedTranscriptEvent> {
    let outer_type = value.get("type").and_then(Value::as_str)?;
    let payload = value.get("payload").unwrap_or(value);
    let timestamp = value
        .get("timestamp")
        .and_then(Value::as_str)
        .or_else(|| payload.get("timestamp").and_then(Value::as_str))
        .map(ToOwned::to_owned);

    if outer_type == "event_msg" {
        let event_type = payload.get("type").and_then(Value::as_str)?;
        match event_type {
            "user_message" => {
                let text = payload.get("message").and_then(Value::as_str)?;
                return Some(parsed_event(
                    "user",
                    "Codex user turn",
                    text,
                    timestamp,
                    event_type,
                ));
            }
            "agent_message" => {
                let text = payload.get("message").and_then(Value::as_str)?;
                return Some(parsed_event(
                    "assistant",
                    "Codex assistant turn",
                    text,
                    timestamp,
                    event_type,
                ));
            }
            "task_complete" => {
                if let Some(text) = payload.get("last_agent_message").and_then(Value::as_str) {
                    return Some(parsed_event(
                        "assistant",
                        "Codex task completion",
                        text,
                        timestamp,
                        event_type,
                    ));
                }
            }
            _ => {}
        }
    }

    None
}

fn parse_claude_event(value: &Value) -> Option<ParsedTranscriptEvent> {
    let event_type = value.get("type").and_then(Value::as_str)?;
    if !matches!(event_type, "user" | "assistant") {
        return None;
    }
    let message = value.get("message").unwrap_or(value);
    let role = message
        .get("role")
        .and_then(Value::as_str)
        .unwrap_or(event_type);
    let text = extract_text_from_value(message.get("content")?)?;
    let timestamp = value
        .get("timestamp")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    Some(parsed_event(
        role,
        &format!("Claude Code {role} turn"),
        &text,
        timestamp,
        event_type,
    ))
}

fn parse_generic_json_event(agent: &str, value: &Value) -> Option<ParsedTranscriptEvent> {
    let role = value
        .get("role")
        .or_else(|| value.get("author"))
        .or_else(|| value.get("speaker"))
        .or_else(|| value.get("type"))
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let text_value = value
        .get("content")
        .or_else(|| value.get("text"))
        .or_else(|| value.get("message"))
        .or_else(|| value.get("body"))?;
    let text = extract_text_from_value(text_value)?;
    let timestamp = value
        .get("timestamp")
        .or_else(|| value.get("created_at"))
        .or_else(|| value.get("time"))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    Some(parsed_event(
        role,
        &format!("{agent} {role} turn"),
        &text,
        timestamp,
        "message",
    ))
}

fn parsed_event(
    role: &str,
    title: &str,
    text: &str,
    timestamp: Option<String>,
    kind: &str,
) -> ParsedTranscriptEvent {
    ParsedTranscriptEvent {
        role: normalize_role(role).to_owned(),
        title: title.to_owned(),
        text: truncate_event_text(text),
        timestamp,
        kind: kind.to_owned(),
    }
}

fn normalize_role(role: &str) -> &str {
    match role.trim().to_ascii_lowercase().as_str() {
        "assistant" | "agent" | "model" | "ai" => "assistant",
        "user" | "human" => "user",
        "system" | "developer" | "tool" => "system",
        _ => "unknown",
    }
}

fn extract_text_from_value(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => non_empty(text),
        Value::Array(items) => {
            let parts = items
                .iter()
                .filter_map(extract_text_from_value)
                .collect::<Vec<_>>();
            non_empty(&parts.join("\n"))
        }
        Value::Object(map) => {
            for key in ["text", "content", "message", "input", "output"] {
                if let Some(text) = map.get(key).and_then(extract_text_from_value) {
                    return Some(text);
                }
            }
            None
        }
        _ => None,
    }
}

fn split_role_prefix(chunk: &str) -> Option<(&str, &str)> {
    let (role, text) = chunk.split_once(':')?;
    let role = role.trim();
    if matches!(
        role.to_ascii_lowercase().as_str(),
        "user" | "human" | "assistant" | "agent" | "system"
    ) {
        Some((role, text.trim()))
    } else {
        None
    }
}

fn non_empty(text: &str) -> Option<String> {
    let text = text.trim();
    if text.is_empty() {
        None
    } else {
        Some(text.to_owned())
    }
}

fn truncate_event_text(text: &str) -> String {
    const MAX_CHARS: usize = 12_000;
    let mut output = String::new();
    for (index, character) in text.trim().chars().enumerate() {
        if index >= MAX_CHARS {
            output.push_str("\n[truncated by Grafiki transcript import]");
            break;
        }
        output.push(character);
    }
    output
}

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::memory::{list_capture_events, ListCaptureEventsOptions};
    use crate::project::{init_project, InitOptions};

    use super::{import_agent_transcripts, ImportAgentTranscriptsOptions};

    #[test]
    fn imports_codex_jsonl_transcript_into_capture_events() {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().join("home");
        let project_dir = temp.path().join("project");
        init_project(InitOptions {
            project_name: Some("project".to_owned()),
            project_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
        })
        .unwrap();
        let transcript = temp.path().join("codex.jsonl");
        fs::write(
            &transcript,
            r#"{"timestamp":"2026-05-31T00:00:00Z","type":"event_msg","payload":{"type":"user_message","message":"Why did we choose Postgres?"}}
{"timestamp":"2026-05-31T00:00:02Z","type":"event_msg","payload":{"type":"agent_message","message":"Postgres was chosen for transactional integrity."}}
"#,
        )
        .unwrap();

        let report = import_agent_transcripts(ImportAgentTranscriptsOptions {
            project_name: Some("project".to_owned()),
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            agent: "codex".to_owned(),
            input: Some(transcript),
            scope: "project/core".to_owned(),
            limit: 20,
            summarize: true,
        })
        .unwrap();

        assert_eq!(report.events_imported, 2);
        assert!(report.candidates.is_some());
        let events = list_capture_events(ListCaptureEventsOptions {
            project_name: Some("project".to_owned()),
            start_dir: project_dir,
            grafiki_home: Some(home),
            capture_id: Some(report.capture_id),
            source_type: Some("transcript".to_owned()),
            scope: "project/core".to_owned(),
            limit: 10,
        })
        .unwrap();
        assert_eq!(events.len(), 2);
        assert!(events.iter().any(|event| event
            .text
            .as_deref()
            .unwrap_or_default()
            .contains("transactional integrity")));
    }

    #[test]
    fn imports_claude_jsonl_transcript_into_capture_events() {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().join("home");
        let project_dir = temp.path().join("project");
        init_project(InitOptions {
            project_name: Some("project".to_owned()),
            project_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
        })
        .unwrap();
        let transcript = temp.path().join("claude.jsonl");
        fs::write(
            &transcript,
            r#"{"type":"user","timestamp":"2026-05-31T00:00:00Z","message":{"role":"user","content":"Remember the worker race."}}
{"type":"assistant","timestamp":"2026-05-31T00:00:01Z","message":{"role":"assistant","content":[{"type":"text","text":"The worker race was fixed with a mutex."}]}}
"#,
        )
        .unwrap();

        let report = import_agent_transcripts(ImportAgentTranscriptsOptions {
            project_name: Some("project".to_owned()),
            start_dir: project_dir.clone(),
            grafiki_home: Some(home),
            agent: "claude-code".to_owned(),
            input: Some(transcript),
            scope: "project/core".to_owned(),
            limit: 20,
            summarize: false,
        })
        .unwrap();

        assert_eq!(report.events_imported, 2);
        assert!(report.candidates.is_none());
    }
}
