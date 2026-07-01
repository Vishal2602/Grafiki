use std::{
    collections::HashMap,
    env, fs,
    path::{Path, PathBuf},
    process::{Command as ProcessCommand, Stdio},
    sync::Mutex,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use tauri::{AppHandle, Manager, RunEvent, State};

use grafiki_core::{
    add_context, approve_candidate, bulk_review_candidates, chat, chat_with_provider,
    delete_context, delete_decision, delete_entity, delete_observation, delete_relation,
    delete_state, edit_candidate, export_memory, generate_report, get_capture_status, get_context,
    get_embedding_status, get_graph, get_status, handoff_session, import_agent_transcripts,
    import_memory, ingest_capture_event, init_project, list_agent_queries, list_candidates,
    list_capture_events, list_context, list_decisions, list_events, list_relations, list_sessions,
    list_state, load_capture_config, log_decision, process_embedding_jobs, propose_candidate,
    propose_capture_candidates, reject_candidate, resolve_project, save_entity, search_memory,
    start_capture_session, start_session, stop_capture_session, update_capture_config,
    update_context, update_decision, update_entity, update_observation, update_relation,
    update_session, upsert_state, AddContextOptions, AgentQueryLogItem,
    AgentTranscriptImportReport, ApproveCandidateOptions, BulkCandidateReviewOptions,
    BulkCandidateReviewReport, CandidateMutationReport, CandidateOrder, CaptureCandidateReport,
    CaptureConfigOptions, CaptureConfigReport, CaptureEvent, CaptureEventReport,
    CaptureSessionReport, CaptureSourceUpdates, CaptureStatusOptions, CaptureStatusReport,
    ChatOptions, ChatReply, ContextListOptions, ContextSummary, DecisionItem, DecisionListOptions,
    DeleteContextOptions, DeleteDecisionOptions, DeleteEntityOptions, DeleteObservationOptions,
    DeleteRelationOptions, DeleteStateOptions, EditCandidateOptions, EmbeddingStatusOptions,
    EmbeddingStatusReport, EndSessionOptions, EndSessionReport, EventListOptions, EvidenceInput,
    ExportBundle, ExportOptions, ExtractionCandidate, GetContextOptions, GraphOptions,
    GraphRelation, GraphReport, HandoffOptions, HandoffReport, ImportAgentTranscriptsOptions,
    ImportOptions, ImportReport, IngestCaptureEventOptions, InitOptions, InitReport,
    ListAgentQueriesOptions, ListCandidatesOptions, ListCaptureEventsOptions, LogDecisionOptions,
    OllamaProvider, ProcessEmbeddingsOptions, ProcessEmbeddingsReport, ProjectReport,
    ProjectReportOptions, ProjectResolveOptions, ProposeCandidateOptions,
    ProposeCaptureCandidatesOptions, RejectCandidateOptions, RelationListOptions,
    SaveEntityOptions, SearchMemoryOptions, SearchMode, SearchReport, SessionLogItem,
    SessionLogOptions, StartCaptureOptions, StartSessionOptions, StartSessionReport, StateItem,
    StateListOptions, StatusOptions, StatusReport, StopCaptureOptions, UpdateCaptureConfigOptions,
    UpdateDecisionOptions, UpdateEntityOptions, UpdateObservationOptions, UpdateRelationOptions,
    UpdateSessionOptions, UpsertStateOptions,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
struct ProjectMeta {
    project: String,
    project_dir: String,
    db_path: String,
    marker_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct ProjectSnapshot {
    start_dir: String,
    scope: String,
    memory_available: bool,
    project: Option<ProjectMeta>,
    status: Option<StatusReport>,
    report: Option<ProjectReport>,
    embedding: Option<EmbeddingStatusReport>,
    error: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SnapshotRequest {
    start_dir: Option<String>,
    scope: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InitProjectRequest {
    project_dir: String,
    project_name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChatRequest {
    start_dir: Option<String>,
    question: String,
    scope: Option<String>,
    limit: Option<usize>,
    model: Option<String>,
    ollama_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SearchRequest {
    start_dir: Option<String>,
    scope: Option<String>,
    query: String,
    record_type: Option<String>,
    mode: Option<String>,
    limit: Option<usize>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphRequest {
    start_dir: Option<String>,
    entity_id: String,
    depth: Option<usize>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DetailRequest {
    start_dir: Option<String>,
    record_type: String,
    id: String,
    scope: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListRecordsRequest {
    start_dir: Option<String>,
    scope: Option<String>,
    status: Option<String>,
    category: Option<String>,
    relation: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListCandidatesRequest {
    start_dir: Option<String>,
    scope: Option<String>,
    status: Option<String>,
    limit: Option<usize>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AgentActivityRequest {
    start_dir: Option<String>,
    scope: Option<String>,
    limit: Option<usize>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CandidateReviewRequest {
    start_dir: Option<String>,
    id: String,
    rationale: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CandidateEditRequest {
    start_dir: Option<String>,
    id: String,
    record_type: Option<String>,
    payload: Option<serde_json::Value>,
    scope: Option<String>,
    confidence: Option<f64>,
    rationale: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CandidateBulkReviewRequest {
    start_dir: Option<String>,
    action: String,
    ids: Vec<String>,
    rationale: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeleteRecordRequest {
    start_dir: Option<String>,
    record_type: String,
    id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExportFileRequest {
    start_dir: Option<String>,
    scope: Option<String>,
    output_path: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ImportFileRequest {
    start_dir: Option<String>,
    input_path: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProcessEmbeddingsRequest {
    start_dir: Option<String>,
    scope: Option<String>,
    limit: Option<usize>,
    rebuild: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DaemonControlRequest {
    start_dir: Option<String>,
    host: Option<String>,
    port: Option<u16>,
    token: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct CliDaemonStatus {
    project: String,
    running: bool,
    pid: Option<u32>,
    host: Option<String>,
    port: Option<u16>,
    pid_path: PathBuf,
    log_path: PathBuf,
}

#[derive(Debug, Clone, Deserialize)]
struct CliDaemonStart {
    project: String,
    running: bool,
    already_running: bool,
    pid: u32,
    host: String,
    port: u16,
    pid_path: PathBuf,
    log_path: PathBuf,
}

#[derive(Debug, Clone, Deserialize)]
struct CliDaemonStop {
    project: String,
    stopped: bool,
    pid: Option<u32>,
    pid_path: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
struct DaemonStatusResponse {
    project: String,
    running: bool,
    pid: Option<u32>,
    host: Option<String>,
    port: Option<u16>,
    url: Option<String>,
    pid_path: String,
    log_path: String,
    cli_path: Option<String>,
    cli_available: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    token: Option<String>,
    message: String,
}

#[derive(Debug, Clone, Serialize)]
struct DaemonStartResponse {
    project: String,
    running: bool,
    already_running: bool,
    pid: u32,
    host: String,
    port: u16,
    url: String,
    pid_path: String,
    log_path: String,
    cli_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    token: Option<String>,
    message: String,
}

#[derive(Debug, Clone, Serialize)]
struct DaemonStopResponse {
    project: String,
    stopped: bool,
    pid: Option<u32>,
    pid_path: String,
    cli_path: String,
    message: String,
}

#[derive(Debug, Clone, Serialize)]
struct DetailMetadata {
    label: String,
    value: String,
}

#[derive(Debug, Clone, Serialize)]
struct RelatedMemoryRecord {
    record_type: String,
    id: String,
    title: String,
    relation: String,
}

#[derive(Debug, Clone, Serialize)]
struct DetailEvent {
    id: String,
    event_type: String,
    summary: String,
    created_at: String,
}

#[derive(Debug, Clone, Serialize)]
struct MemoryRecordDetail {
    record_type: String,
    id: String,
    title: String,
    scope: String,
    body: String,
    metadata: Vec<DetailMetadata>,
    related: Vec<RelatedMemoryRecord>,
    events: Vec<DetailEvent>,
    focus_entity_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CaptureRequest {
    start_dir: Option<String>,
    capture_type: String,
    title: String,
    scope: Option<String>,
    content: Option<String>,
    key: Option<String>,
    entity_type: Option<String>,
    category: Option<String>,
    status: Option<String>,
    priority: Option<String>,
    relation_target: Option<String>,
    relation_type: Option<String>,
    tags: Option<String>,
    alternatives: Option<String>,
    supersedes: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AutoCaptureRequest {
    start_dir: Option<String>,
    scope: Option<String>,
    source: Option<String>,
    limit: Option<usize>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CaptureControlRequest {
    start_dir: Option<String>,
    scope: Option<String>,
    source_app: Option<String>,
    consent_profile: Option<String>,
    redaction_profile: Option<String>,
    capture_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawCaptureEventRequest {
    start_dir: Option<String>,
    capture_id: Option<String>,
    scope: Option<String>,
    source_type: String,
    source: Option<String>,
    title: Option<String>,
    text: Option<String>,
    payload: Option<serde_json::Value>,
    metadata: Option<serde_json::Value>,
    privacy_level: Option<String>,
    redacted: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CaptureListRequest {
    start_dir: Option<String>,
    capture_id: Option<String>,
    scope: Option<String>,
    source_type: Option<String>,
    limit: Option<usize>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CaptureConfigRequest {
    start_dir: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CaptureConfigUpdateRequest {
    start_dir: Option<String>,
    git: Option<bool>,
    transcripts: Option<bool>,
    terminal: Option<bool>,
    files: Option<bool>,
    ide: Option<bool>,
    screen: Option<bool>,
    browser: Option<bool>,
    audio: Option<bool>,
    system: Option<bool>,
    add_blocked_paths: Option<Vec<String>>,
    remove_blocked_paths: Option<Vec<String>>,
    add_blocked_apps: Option<Vec<String>>,
    remove_blocked_apps: Option<Vec<String>>,
    redaction_profile: Option<String>,
    terminal_output: Option<String>,
    screen_policy: Option<String>,
    browser_policy: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ScreenCaptureRequest {
    start_dir: Option<String>,
    capture_id: Option<String>,
    scope: Option<String>,
    source: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TranscriptImportRequest {
    start_dir: Option<String>,
    scope: Option<String>,
    agent: String,
    input: Option<String>,
    limit: Option<usize>,
    summarize: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateRecordRequest {
    start_dir: Option<String>,
    record_type: String,
    id: String,
    title: Option<String>,
    scope: Option<String>,
    content: Option<String>,
    category: Option<String>,
    entity_type: Option<String>,
    status: Option<String>,
    priority: Option<String>,
    relation: Option<String>,
    weight: Option<f64>,
    confidence: Option<f64>,
    source_type: Option<String>,
    source: Option<String>,
    session_type: Option<String>,
    goal: Option<String>,
    summary: Option<String>,
    accomplishments: Option<String>,
    remaining: Option<String>,
    files_changed: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct CaptureResponse {
    record_type: String,
    id: String,
    title: String,
    scope: String,
    message: String,
}

#[derive(Debug, Clone, Serialize)]
struct AutoCaptureResponse {
    scope: String,
    source: String,
    path: String,
    git_root: Option<String>,
    changed_files: Vec<String>,
    staged_files: Vec<String>,
    unstaged_files: Vec<String>,
    untracked_files: Vec<String>,
    diff_stat: String,
    last_commit: Option<String>,
    candidates: Vec<CandidateMutationReport>,
    message: String,
}

#[derive(Debug, Default)]
struct GitStatusSummary {
    changed_files: Vec<String>,
    staged_files: Vec<String>,
    unstaged_files: Vec<String>,
    untracked_files: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct DeleteRecordResponse {
    record_type: String,
    id: String,
    title: String,
    scope: String,
    message: String,
}

#[derive(Debug, Clone, Serialize)]
struct UpdateRecordResponse {
    record_type: String,
    id: String,
    title: String,
    scope: String,
    message: String,
}

#[derive(Debug, Clone, Serialize)]
struct ExportFileResponse {
    output_path: String,
    records: usize,
    message: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StartSessionRequest {
    start_dir: Option<String>,
    session_type: String,
    goal: String,
    scope: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EndSessionRequest {
    start_dir: Option<String>,
    session_id: Option<String>,
    status: String,
    summary: Option<String>,
    accomplishments: Option<String>,
    remaining: Option<String>,
    files_changed: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HandoffSessionRequest {
    start_dir: Option<String>,
    session_id: Option<String>,
}

#[tauri::command(async)]
fn get_project_snapshot(request: Option<SnapshotRequest>) -> ProjectSnapshot {
    let request = request.unwrap_or(SnapshotRequest {
        start_dir: None,
        scope: None,
    });
    let start_dir = resolve_start_dir(request.start_dir);
    let scope = request.scope.unwrap_or_default();

    let project = match resolve_project(ProjectResolveOptions {
        project_name: None,
        start_dir: start_dir.clone(),
        grafiki_home: None,
    }) {
        Ok(project) => project,
        Err(error) => {
            return ProjectSnapshot {
                start_dir: display_path(&start_dir),
                scope,
                memory_available: false,
                project: None,
                status: None,
                report: None,
                embedding: None,
                error: Some(error.to_string()),
            };
        }
    };

    let status = get_status(StatusOptions {
        project_name: Some(project.project.clone()),
        start_dir: project.project_dir.clone(),
        grafiki_home: None,
        scope: scope.clone(),
    });
    let report = generate_report(ProjectReportOptions {
        project_name: Some(project.project.clone()),
        start_dir: project.project_dir.clone(),
        grafiki_home: None,
        scope: scope.clone(),
    });
    let embedding = get_embedding_status(EmbeddingStatusOptions {
        project_name: Some(project.project.clone()),
        start_dir: project.project_dir.clone(),
        grafiki_home: None,
        scope: scope.clone(),
    });

    let mut errors = Vec::new();
    let status = match status {
        Ok(report) => Some(report),
        Err(error) => {
            errors.push(error.to_string());
            None
        }
    };
    let report = match report {
        Ok(report) => Some(report),
        Err(error) => {
            errors.push(error.to_string());
            None
        }
    };
    let embedding = match embedding {
        Ok(report) => Some(report),
        Err(error) => {
            errors.push(error.to_string());
            None
        }
    };

    ProjectSnapshot {
        start_dir: display_path(&start_dir),
        scope,
        memory_available: true,
        project: Some(ProjectMeta {
            project: project.project,
            project_dir: display_path(&project.project_dir),
            db_path: display_path(&project.db_path),
            marker_path: project.marker_path.as_ref().map(|path| display_path(path)),
        }),
        status,
        report,
        embedding,
        error: if errors.is_empty() {
            None
        } else {
            Some(errors.join("; "))
        },
    }
}

#[tauri::command]
fn initialize_project(request: InitProjectRequest) -> Result<InitReport, String> {
    init_project(InitOptions {
        project_name: clean_optional(request.project_name),
        project_dir: PathBuf::from(request.project_dir),
        grafiki_home: None,
    })
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn search_project_memory(request: SearchRequest) -> Result<SearchReport, String> {
    let start_dir = resolve_start_dir(request.start_dir);
    let mode = SearchMode::parse(request.mode.as_deref().unwrap_or("hybrid"))
        .map_err(|error| error.to_string())?;

    search_memory(SearchMemoryOptions {
        project_name: None,
        start_dir,
        grafiki_home: None,
        query: request.query,
        record_type: request.record_type.unwrap_or_else(|| "all".to_owned()),
        mode,
        scope: request.scope.unwrap_or_default(),
        limit: request.limit.unwrap_or(20),
        temporal_weight: 0.0,
    })
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn chat_with_memory(request: ChatRequest) -> Result<ChatReply, String> {
    let options = ChatOptions {
        project_name: None,
        start_dir: resolve_start_dir(request.start_dir),
        grafiki_home: None,
        question: request.question,
        scope: request.scope.unwrap_or_default(),
        limit: request.limit.unwrap_or(8),
        temporal_weight: 0.0,
    };
    // With a local model, phrase a conversational answer via Ollama; if it's
    // unreachable, fall back to the deterministic extractive answer so the user
    // still gets their memory (retrieval already succeeded).
    match clean_optional(request.model) {
        Some(model) => {
            let provider = OllamaProvider::new(clean_optional(request.ollama_url), Some(model));
            match chat_with_provider(options.clone(), &provider) {
                Ok(reply) => Ok(reply),
                Err(_) => chat(options).map_err(|error| error.to_string()),
            }
        }
        None => chat(options).map_err(|error| error.to_string()),
    }
}

#[tauri::command]
fn start_grafiki_session(request: StartSessionRequest) -> Result<StartSessionReport, String> {
    start_session(StartSessionOptions {
        project_name: None,
        start_dir: resolve_start_dir(request.start_dir),
        grafiki_home: None,
        session_type: required("Session type", &request.session_type)?,
        goal: required("Goal", &request.goal)?,
        scope: clean_optional(request.scope).unwrap_or_default(),
    })
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn end_grafiki_session(request: EndSessionRequest) -> Result<EndSessionReport, String> {
    grafiki_core::end_session(EndSessionOptions {
        project_name: None,
        start_dir: resolve_start_dir(request.start_dir),
        grafiki_home: None,
        session_id: clean_optional(request.session_id),
        status: required("Status", &request.status)?,
        summary: clean_optional(request.summary),
        accomplishments: split_csv(request.accomplishments),
        remaining: split_csv(request.remaining),
        files_changed: split_csv(request.files_changed),
    })
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn get_memory_graph(request: GraphRequest) -> Result<GraphReport, String> {
    get_graph(GraphOptions {
        project_name: None,
        start_dir: resolve_start_dir(request.start_dir),
        grafiki_home: None,
        entity_id: request.entity_id,
        depth: request.depth.unwrap_or(2),
    })
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn handoff_grafiki_session(request: HandoffSessionRequest) -> Result<HandoffReport, String> {
    handoff_session(HandoffOptions {
        project_name: None,
        start_dir: resolve_start_dir(request.start_dir),
        grafiki_home: None,
        session_id: request.session_id,
    })
    .map_err(|error| error.to_string())
}

#[tauri::command(async)]
fn get_memory_record(request: DetailRequest) -> Result<MemoryRecordDetail, String> {
    let start_dir = resolve_start_dir(request.start_dir);
    let scope = clean_optional(request.scope).unwrap_or_default();
    let record_type = normalize_record_type(&request.record_type);
    let id = request.id.trim().to_owned();
    if id.is_empty() {
        return Err("Record id is required.".to_owned());
    }

    let bundle = export_memory(ExportOptions {
        project_name: None,
        start_dir: start_dir.clone(),
        grafiki_home: None,
        scope: scope.clone(),
    })
    .map_err(|error| error.to_string())?;
    let events = detail_events(&start_dir, &scope, &record_type, &id);

    match record_type.as_str() {
        "entity" => {
            let entity = bundle
                .entities
                .iter()
                .find(|entity| entity.id == id)
                .ok_or_else(|| format!("Entity not found: {id}"))?;
            let related = related_for_entity(&bundle, &entity.id);
            Ok(MemoryRecordDetail {
                record_type,
                id: entity.id.clone(),
                title: entity.name.clone(),
                scope: entity.scope.clone(),
                body: format!(
                    "{} entity in scope {}",
                    entity.entity_type,
                    display_scope(&entity.scope)
                ),
                metadata: vec![
                    metadata("entity type", &entity.entity_type),
                    metadata("scope", display_scope(&entity.scope)),
                    metadata("relations", related.len().to_string()),
                ],
                related,
                events,
                focus_entity_id: Some(entity.id.clone()),
            })
        }
        "observation" => {
            let observation = bundle
                .observations
                .iter()
                .find(|observation| observation.id == id)
                .ok_or_else(|| format!("Observation not found: {id}"))?;
            let entity_title = bundle
                .entities
                .iter()
                .find(|entity| entity.id == observation.entity_id)
                .map(|entity| entity.name.clone())
                .unwrap_or_else(|| observation.entity_id.clone());
            Ok(MemoryRecordDetail {
                record_type,
                id: observation.id.clone(),
                title: entity_title.clone(),
                scope: observation.scope.clone(),
                body: observation.content.clone(),
                metadata: vec![
                    metadata("entity", &observation.entity_id),
                    metadata("category", &observation.category),
                    metadata("confidence", format!("{:.2}", observation.confidence)),
                    metadata("scope", display_scope(&observation.scope)),
                ],
                related: vec![RelatedMemoryRecord {
                    record_type: "entity".to_owned(),
                    id: observation.entity_id.clone(),
                    title: entity_title,
                    relation: "attached_to".to_owned(),
                }],
                events,
                focus_entity_id: Some(observation.entity_id.clone()),
            })
        }
        "decision" => {
            let decision = bundle
                .decisions
                .iter()
                .find(|decision| decision.id == id)
                .ok_or_else(|| format!("Decision not found: {id}"))?;
            Ok(MemoryRecordDetail {
                record_type,
                id: decision.id.clone(),
                title: decision.title.clone(),
                scope: decision.scope.clone(),
                body: decision
                    .reasoning
                    .clone()
                    .unwrap_or_else(|| "No reasoning recorded yet.".to_owned()),
                metadata: vec![
                    metadata("status", &decision.status),
                    metadata("scope", display_scope(&decision.scope)),
                ],
                related: Vec::new(),
                events,
                focus_entity_id: None,
            })
        }
        "context" => {
            let document = get_context(GetContextOptions {
                project_name: None,
                start_dir,
                grafiki_home: None,
                key: id,
            })
            .map_err(|error| error.to_string())?;
            Ok(MemoryRecordDetail {
                record_type,
                id: document.key.clone(),
                title: document.title.clone(),
                scope: document.scope.clone(),
                body: document.content.clone(),
                metadata: vec![
                    metadata("category", &document.category),
                    metadata("version", document.version.to_string()),
                    metadata("scope", display_scope(&document.scope)),
                ],
                related: Vec::new(),
                events,
                focus_entity_id: None,
            })
        }
        "state" => {
            let state = bundle
                .state
                .iter()
                .find(|state| state.key == id)
                .ok_or_else(|| format!("State item not found: {id}"))?;
            Ok(MemoryRecordDetail {
                record_type,
                id: state.key.clone(),
                title: state.title.clone(),
                scope: state.scope.clone(),
                body: state
                    .owner
                    .as_ref()
                    .map(|owner| format!("Owned by {owner}."))
                    .unwrap_or_else(|| "No owner recorded.".to_owned()),
                metadata: vec![
                    metadata("status", &state.status),
                    metadata("priority", &state.priority),
                    metadata("owner", state.owner.as_deref().unwrap_or("unassigned")),
                    metadata("scope", display_scope(&state.scope)),
                ],
                related: Vec::new(),
                events,
                focus_entity_id: None,
            })
        }
        "relation" => {
            let relation = bundle
                .relations
                .iter()
                .find(|relation| relation.id == id)
                .ok_or_else(|| format!("Relation not found: {id}"))?;
            let from_title = entity_title(&bundle, &relation.from_entity);
            let to_title = entity_title(&bundle, &relation.to_entity);
            Ok(MemoryRecordDetail {
                record_type,
                id: relation.id.clone(),
                title: format!("{from_title} {} {to_title}", relation.relation),
                scope: String::new(),
                body: format!(
                    "{} {} {}",
                    relation.from_entity, relation.relation, relation.to_entity
                ),
                metadata: vec![
                    metadata("relation", &relation.relation),
                    metadata("weight", format!("{:.2}", relation.weight)),
                    metadata("confidence", format!("{:.2}", relation.confidence)),
                    metadata("source type", &relation.source_type),
                    metadata("source", relation.source.as_deref().unwrap_or("")),
                ],
                related: vec![
                    RelatedMemoryRecord {
                        record_type: "entity".to_owned(),
                        id: relation.from_entity.clone(),
                        title: from_title,
                        relation: "from".to_owned(),
                    },
                    RelatedMemoryRecord {
                        record_type: "entity".to_owned(),
                        id: relation.to_entity.clone(),
                        title: to_title,
                        relation: "to".to_owned(),
                    },
                ],
                events,
                focus_entity_id: Some(relation.from_entity.clone()),
            })
        }
        "session" => {
            let session = bundle
                .sessions
                .iter()
                .find(|session| session.id == id)
                .ok_or_else(|| format!("Session not found: {id}"))?;
            let mut metadata_items = vec![
                metadata("type", &session.session_type),
                metadata("status", &session.status),
                metadata("started", &session.started_at),
                metadata("ended", session.ended_at.as_deref().unwrap_or("active")),
                metadata("scope", display_scope(&session.scope)),
                metadata("accomplishments", session.accomplishments.join(", ")),
                metadata("remaining", session.remaining.join(", ")),
                metadata("files changed", session.files_changed.join(", ")),
                metadata("decisions made", session.decisions_made.join(", ")),
                metadata("entities touched", session.entities_touched.join(", ")),
            ];
            if let Some(parent_session) = &session.parent_session {
                metadata_items.push(metadata("parent session", parent_session));
            }
            if let Some(child_session) = &session.child_session {
                metadata_items.push(metadata("child session", child_session));
            }
            if let Some(handoff_context) = &session.handoff_context {
                metadata_items.push(metadata("handoff context", handoff_context));
            }

            let mut related = Vec::new();
            if let Some(parent_session) = &session.parent_session {
                related.push(RelatedMemoryRecord {
                    record_type: "session".to_owned(),
                    id: parent_session.clone(),
                    title: parent_session.clone(),
                    relation: "parent".to_owned(),
                });
            }
            if let Some(child_session) = &session.child_session {
                related.push(RelatedMemoryRecord {
                    record_type: "session".to_owned(),
                    id: child_session.clone(),
                    title: child_session.clone(),
                    relation: "child".to_owned(),
                });
            }

            Ok(MemoryRecordDetail {
                record_type,
                id: session.id.clone(),
                title: session.goal.clone().unwrap_or_else(|| session.id.clone()),
                scope: session.scope.clone(),
                body: session
                    .summary
                    .clone()
                    .unwrap_or_else(|| "No session summary recorded yet.".to_owned()),
                metadata: metadata_items,
                related,
                events,
                focus_entity_id: None,
            })
        }
        _ => Err(format!("Unsupported record type: {}", request.record_type)),
    }
}

#[tauri::command]
fn list_project_context(
    request: Option<ListRecordsRequest>,
) -> Result<Vec<ContextSummary>, String> {
    let request = request.unwrap_or(ListRecordsRequest {
        start_dir: None,
        scope: None,
        status: None,
        category: None,
        relation: None,
    });

    list_context(ContextListOptions {
        project_name: None,
        start_dir: resolve_start_dir(request.start_dir),
        grafiki_home: None,
        category: clean_optional(request.category),
        scope: clean_optional(request.scope).unwrap_or_default(),
    })
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_project_state(request: Option<ListRecordsRequest>) -> Result<Vec<StateItem>, String> {
    let request = request.unwrap_or(ListRecordsRequest {
        start_dir: None,
        scope: None,
        status: None,
        category: None,
        relation: None,
    });

    list_state(StateListOptions {
        project_name: None,
        start_dir: resolve_start_dir(request.start_dir),
        grafiki_home: None,
        status: clean_optional(request.status),
        scope: clean_optional(request.scope).unwrap_or_default(),
    })
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_project_sessions(
    request: Option<ListRecordsRequest>,
) -> Result<Vec<SessionLogItem>, String> {
    let request = request.unwrap_or(ListRecordsRequest {
        start_dir: None,
        scope: None,
        status: None,
        category: None,
        relation: None,
    });

    list_sessions(SessionLogOptions {
        project_name: None,
        start_dir: resolve_start_dir(request.start_dir),
        grafiki_home: None,
        session_type: None,
        scope: clean_optional(request.scope).unwrap_or_default(),
        limit: 100,
    })
    .map(|report| report.sessions)
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_project_decisions(
    request: Option<ListRecordsRequest>,
) -> Result<Vec<DecisionItem>, String> {
    let request = request.unwrap_or(ListRecordsRequest {
        start_dir: None,
        scope: None,
        status: None,
        category: None,
        relation: None,
    });

    list_decisions(DecisionListOptions {
        project_name: None,
        start_dir: resolve_start_dir(request.start_dir),
        grafiki_home: None,
        status: clean_optional(request.status),
        scope: clean_optional(request.scope).unwrap_or_default(),
    })
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_project_relations(
    request: Option<ListRecordsRequest>,
) -> Result<Vec<GraphRelation>, String> {
    let request = request.unwrap_or(ListRecordsRequest {
        start_dir: None,
        scope: None,
        status: None,
        category: None,
        relation: None,
    });

    list_relations(RelationListOptions {
        project_name: None,
        start_dir: resolve_start_dir(request.start_dir),
        grafiki_home: None,
        scope: clean_optional(request.scope).unwrap_or_default(),
        relation: clean_optional(request.relation),
    })
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_memory_candidates(
    request: Option<ListCandidatesRequest>,
) -> Result<Vec<ExtractionCandidate>, String> {
    let request = request.unwrap_or(ListCandidatesRequest {
        start_dir: None,
        scope: None,
        status: None,
        limit: None,
    });

    list_candidates(ListCandidatesOptions {
        project_name: None,
        start_dir: resolve_start_dir(request.start_dir),
        grafiki_home: None,
        status: clean_optional(request.status),
        scope: clean_optional(request.scope).unwrap_or_default(),
        limit: request.limit.unwrap_or(50),
        order: CandidateOrder::Recent,
    })
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_agent_activity(
    request: Option<AgentActivityRequest>,
) -> Result<Vec<AgentQueryLogItem>, String> {
    let request = request.unwrap_or(AgentActivityRequest {
        start_dir: None,
        scope: None,
        limit: None,
    });

    list_agent_queries(ListAgentQueriesOptions {
        project_name: None,
        start_dir: resolve_start_dir(request.start_dir),
        grafiki_home: None,
        scope: clean_optional(request.scope).unwrap_or_default(),
        limit: request.limit.unwrap_or(50),
    })
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn approve_memory_candidate(
    request: CandidateReviewRequest,
) -> Result<CandidateMutationReport, String> {
    approve_candidate(ApproveCandidateOptions {
        project_name: None,
        start_dir: resolve_start_dir(request.start_dir),
        grafiki_home: None,
        id: required("Candidate id", &request.id)?,
    })
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn edit_memory_candidate(request: CandidateEditRequest) -> Result<CandidateMutationReport, String> {
    edit_candidate(EditCandidateOptions {
        project_name: None,
        start_dir: resolve_start_dir(request.start_dir),
        grafiki_home: None,
        id: required("Candidate id", &request.id)?,
        record_type: clean_optional(request.record_type),
        payload: request.payload,
        scope: clean_optional(request.scope),
        confidence: request.confidence,
        rationale: clean_optional(request.rationale),
    })
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn bulk_review_memory_candidates(
    request: CandidateBulkReviewRequest,
) -> Result<BulkCandidateReviewReport, String> {
    bulk_review_candidates(BulkCandidateReviewOptions {
        project_name: None,
        start_dir: resolve_start_dir(request.start_dir),
        grafiki_home: None,
        action: request.action,
        ids: request.ids,
        rationale: clean_optional(request.rationale),
    })
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn reject_memory_candidate(
    request: CandidateReviewRequest,
) -> Result<CandidateMutationReport, String> {
    reject_candidate(RejectCandidateOptions {
        project_name: None,
        start_dir: resolve_start_dir(request.start_dir),
        grafiki_home: None,
        id: required("Candidate id", &request.id)?,
        rationale: clean_optional(request.rationale),
    })
    .map_err(|error| error.to_string())
}

#[tauri::command(async)]
fn auto_capture_memory(request: AutoCaptureRequest) -> Result<AutoCaptureResponse, String> {
    let start_dir = resolve_start_dir(request.start_dir);
    let scope = clean_optional(request.scope).unwrap_or_default();
    let source = clean_optional(request.source).unwrap_or_else(|| "desktop".to_owned());
    auto_capture_working_tree(start_dir, scope, source, request.limit.unwrap_or(25))
}

#[tauri::command]
fn start_automatic_capture(request: CaptureControlRequest) -> Result<CaptureSessionReport, String> {
    start_capture_session(StartCaptureOptions {
        project_name: None,
        start_dir: resolve_start_dir(request.start_dir),
        grafiki_home: None,
        scope: clean_optional(request.scope).unwrap_or_default(),
        source_app: clean_optional(request.source_app)
            .or_else(|| Some("grafiki-desktop".to_owned())),
        consent_profile: clean_optional(request.consent_profile),
        redaction_profile: clean_optional(request.redaction_profile),
    })
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn stop_automatic_capture(request: CaptureControlRequest) -> Result<CaptureSessionReport, String> {
    stop_capture_session(StopCaptureOptions {
        project_name: None,
        start_dir: resolve_start_dir(request.start_dir),
        grafiki_home: None,
        capture_id: request
            .capture_id
            .as_deref()
            .map(|value| required("Capture id", value))
            .transpose()?
            .ok_or_else(|| "Capture id is required.".to_owned())?,
    })
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn ingest_raw_capture_event(request: RawCaptureEventRequest) -> Result<CaptureEventReport, String> {
    ingest_capture_event(IngestCaptureEventOptions {
        project_name: None,
        start_dir: resolve_start_dir(request.start_dir),
        grafiki_home: None,
        capture_id: clean_optional(request.capture_id),
        scope: clean_optional(request.scope).unwrap_or_default(),
        source_type: request.source_type,
        source: clean_optional(request.source),
        title: clean_optional(request.title),
        text: clean_optional(request.text),
        payload: request.payload,
        metadata: request.metadata,
        privacy_level: clean_optional(request.privacy_level),
        redacted: request.redacted.unwrap_or(false),
        captured_at: None,
    })
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn get_automatic_capture_status(
    request: Option<CaptureListRequest>,
) -> Result<CaptureStatusReport, String> {
    let request = request.unwrap_or(CaptureListRequest {
        start_dir: None,
        capture_id: None,
        scope: None,
        source_type: None,
        limit: None,
    });
    get_capture_status(CaptureStatusOptions {
        project_name: None,
        start_dir: resolve_start_dir(request.start_dir),
        grafiki_home: None,
        scope: clean_optional(request.scope).unwrap_or_default(),
    })
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_raw_capture_events(
    request: Option<CaptureListRequest>,
) -> Result<Vec<CaptureEvent>, String> {
    let request = request.unwrap_or(CaptureListRequest {
        start_dir: None,
        capture_id: None,
        scope: None,
        source_type: None,
        limit: None,
    });
    list_capture_events(ListCaptureEventsOptions {
        project_name: None,
        start_dir: resolve_start_dir(request.start_dir),
        grafiki_home: None,
        capture_id: clean_optional(request.capture_id),
        source_type: clean_optional(request.source_type),
        scope: clean_optional(request.scope).unwrap_or_default(),
        limit: request.limit.unwrap_or(50),
    })
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn get_capture_config(
    request: Option<CaptureConfigRequest>,
) -> Result<CaptureConfigReport, String> {
    let request = request.unwrap_or(CaptureConfigRequest { start_dir: None });
    load_capture_config(CaptureConfigOptions {
        project_name: None,
        start_dir: resolve_start_dir(request.start_dir),
        grafiki_home: None,
    })
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn update_capture_config_settings(
    request: CaptureConfigUpdateRequest,
) -> Result<CaptureConfigReport, String> {
    update_capture_config(UpdateCaptureConfigOptions {
        project_name: None,
        start_dir: resolve_start_dir(request.start_dir),
        grafiki_home: None,
        sources: CaptureSourceUpdates {
            git: request.git,
            transcripts: request.transcripts,
            terminal: request.terminal,
            files: request.files,
            ide: request.ide,
            screen: request.screen,
            browser: request.browser,
            audio: request.audio,
            system: request.system,
        },
        add_blocked_paths: request.add_blocked_paths.unwrap_or_default(),
        remove_blocked_paths: request.remove_blocked_paths.unwrap_or_default(),
        add_blocked_apps: request.add_blocked_apps.unwrap_or_default(),
        remove_blocked_apps: request.remove_blocked_apps.unwrap_or_default(),
        redaction_profile: clean_optional(request.redaction_profile),
        terminal_output: clean_optional(request.terminal_output),
        screen_policy: clean_optional(request.screen_policy),
        browser_policy: clean_optional(request.browser_policy),
    })
    .map_err(|error| error.to_string())
}

#[tauri::command(async)]
fn summarize_automatic_capture(
    request: Option<CaptureListRequest>,
) -> Result<CaptureCandidateReport, String> {
    let request = request.unwrap_or(CaptureListRequest {
        start_dir: None,
        capture_id: None,
        scope: None,
        source_type: None,
        limit: None,
    });
    propose_capture_candidates(ProposeCaptureCandidatesOptions {
        project_name: None,
        start_dir: resolve_start_dir(request.start_dir),
        grafiki_home: None,
        capture_id: clean_optional(request.capture_id),
        scope: clean_optional(request.scope).unwrap_or_default(),
        limit: request.limit.unwrap_or(80),
    })
    .map_err(|error| error.to_string())
}

#[tauri::command(async)]
fn import_agent_transcripts_from_disk(
    request: TranscriptImportRequest,
) -> Result<AgentTranscriptImportReport, String> {
    import_agent_transcripts(ImportAgentTranscriptsOptions {
        project_name: None,
        start_dir: resolve_start_dir(request.start_dir),
        grafiki_home: None,
        agent: request.agent,
        input: clean_optional(request.input).map(PathBuf::from),
        scope: clean_optional(request.scope).unwrap_or_default(),
        limit: request.limit.unwrap_or(200),
        summarize: request.summarize.unwrap_or(false),
    })
    .map_err(|error| error.to_string())
}

#[tauri::command(async)]
fn capture_screen_snapshot(request: ScreenCaptureRequest) -> Result<CaptureEventReport, String> {
    let start_dir = resolve_start_dir(request.start_dir);
    let scope = clean_optional(request.scope).unwrap_or_default();
    let captures_dir = grafiki_capture_dir()?;
    fs::create_dir_all(&captures_dir).map_err(|error| error.to_string())?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0));
    // Include sub-second nanos so two captures in the same second don't collide.
    let path = captures_dir.join(format!(
        "screen-{}-{:09}.png",
        stamp.as_secs(),
        stamp.subsec_nanos()
    ));
    let output = ProcessCommand::new("screencapture")
        .arg("-x")
        .arg(&path)
        .stdin(Stdio::null())
        .output()
        .map_err(|error| format!("Failed to start screencapture: {error}"))?;
    if !output.status.success() {
        let message = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        return Err(if message.is_empty() {
            "screencapture failed. Check macOS Screen Recording permission.".to_owned()
        } else {
            message
        });
    }
    // Bound how many screenshots accumulate on disk.
    prune_screenshots(&captures_dir, 50);

    ingest_capture_event(IngestCaptureEventOptions {
        project_name: None,
        start_dir,
        grafiki_home: None,
        capture_id: clean_optional(request.capture_id),
        scope,
        source_type: "screen".to_owned(),
        source: clean_optional(request.source).or_else(|| Some("macos-screencapture".to_owned())),
        title: Some("Screen snapshot".to_owned()),
        text: Some(format!("Screen snapshot captured at {}", path.display())),
        payload: Some(serde_json::json!({
            "image_path": path.display().to_string(),
            "kind": "screen_snapshot"
        })),
        metadata: Some(serde_json::json!({
            "capture_command": "screencapture -x",
            "local_file": true
        })),
        privacy_level: Some("sensitive".to_owned()),
        redacted: false,
        captured_at: None,
    })
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn delete_memory_record(request: DeleteRecordRequest) -> Result<DeleteRecordResponse, String> {
    let start_dir = resolve_start_dir(request.start_dir);
    let record_type = normalize_record_type(&request.record_type);
    let id = required("Record id", &request.id)?;

    match record_type.as_str() {
        "context" => {
            let report = delete_context(DeleteContextOptions {
                project_name: None,
                start_dir,
                grafiki_home: None,
                key: id.clone(),
            })
            .map_err(|error| error.to_string())?;

            Ok(DeleteRecordResponse {
                record_type,
                id: report.key,
                title: report.title,
                scope: report.scope,
                message: "Context deleted.".to_owned(),
            })
        }
        "state" => {
            let report = delete_state(DeleteStateOptions {
                project_name: None,
                start_dir,
                grafiki_home: None,
                key: id.clone(),
            })
            .map_err(|error| error.to_string())?;

            Ok(DeleteRecordResponse {
                record_type,
                id: report.key,
                title: report.title,
                scope: report.scope,
                message: "State item deleted.".to_owned(),
            })
        }
        "decision" => {
            let report = delete_decision(DeleteDecisionOptions {
                project_name: None,
                start_dir,
                grafiki_home: None,
                id: id.clone(),
            })
            .map_err(|error| error.to_string())?;

            Ok(DeleteRecordResponse {
                record_type,
                id: report.id,
                title: report.title,
                scope: report.scope,
                message: "Decision deleted.".to_owned(),
            })
        }
        "entity" => {
            let report = delete_entity(DeleteEntityOptions {
                project_name: None,
                start_dir,
                grafiki_home: None,
                id: id.clone(),
            })
            .map_err(|error| error.to_string())?;

            Ok(DeleteRecordResponse {
                record_type,
                id: report.id,
                title: report.name,
                scope: report.scope,
                message: "Entity deleted.".to_owned(),
            })
        }
        "observation" => {
            let report = delete_observation(DeleteObservationOptions {
                project_name: None,
                start_dir,
                grafiki_home: None,
                id: id.clone(),
            })
            .map_err(|error| error.to_string())?;

            Ok(DeleteRecordResponse {
                record_type,
                id: report.id,
                title: report.entity_name,
                scope: report.scope,
                message: "Observation invalidated.".to_owned(),
            })
        }
        "relation" => {
            let report = delete_relation(DeleteRelationOptions {
                project_name: None,
                start_dir,
                grafiki_home: None,
                id: id.clone(),
            })
            .map_err(|error| error.to_string())?;

            Ok(DeleteRecordResponse {
                record_type,
                id: report.id,
                title: format!(
                    "{} {} {}",
                    report.from_entity, report.relation, report.to_entity
                ),
                scope: String::new(),
                message: "Relation removed.".to_owned(),
            })
        }
        // Sessions are intentionally non-deletable (matches the CLI/MCP contract);
        // the UI hides the Delete affordance for sessions, so this is a backstop.
        "session" => Err("Session records can be updated but not deleted.".to_owned()),
        other => Err(format!("Unsupported memory record type: {other}")),
    }
}

#[tauri::command]
fn update_memory_record(request: UpdateRecordRequest) -> Result<UpdateRecordResponse, String> {
    let start_dir = resolve_start_dir(request.start_dir);
    let record_type = normalize_record_type(&request.record_type);
    let id = required("Record id", &request.id)?;

    match record_type.as_str() {
        "context" => {
            let report = update_context(grafiki_core::UpdateContextOptions {
                project_name: None,
                start_dir,
                grafiki_home: None,
                key: id,
                title: clean_optional(request.title),
                category: clean_optional(request.category),
                scope: clean_optional(request.scope),
                content: clean_optional(request.content),
            })
            .map_err(|error| error.to_string())?;

            Ok(UpdateRecordResponse {
                record_type,
                id: report.key,
                title: report.title,
                scope: report.scope,
                message: "Context updated.".to_owned(),
            })
        }
        "state" => {
            let title = required("Title", &request.title.unwrap_or_default())?;
            let report = upsert_state(UpsertStateOptions {
                project_name: None,
                start_dir,
                grafiki_home: None,
                key: id,
                title,
                status: clean_optional(request.status).unwrap_or_else(|| "in-progress".to_owned()),
                owner: None,
                details: clean_optional(request.content),
                blockers: Vec::new(),
                depends_on: Vec::new(),
                scope: clean_optional(request.scope).unwrap_or_default(),
                priority: clean_optional(request.priority).unwrap_or_else(|| "medium".to_owned()),
            })
            .map_err(|error| error.to_string())?;

            Ok(UpdateRecordResponse {
                record_type,
                id: report.key,
                title: report.title,
                scope: report.scope,
                message: "State item updated.".to_owned(),
            })
        }
        "decision" => {
            let report = update_decision(UpdateDecisionOptions {
                project_name: None,
                start_dir,
                grafiki_home: None,
                id,
                title: clean_optional(request.title),
                reasoning: clean_optional(request.content),
                scope: clean_optional(request.scope),
                status: clean_optional(request.status),
            })
            .map_err(|error| error.to_string())?;

            Ok(UpdateRecordResponse {
                record_type,
                id: report.id,
                title: report.title,
                scope: report.scope,
                message: "Decision updated.".to_owned(),
            })
        }
        "entity" => {
            let report = update_entity(UpdateEntityOptions {
                project_name: None,
                start_dir,
                grafiki_home: None,
                id,
                name: clean_optional(request.title),
                entity_type: clean_optional(request.entity_type),
                scope: clean_optional(request.scope),
            })
            .map_err(|error| error.to_string())?;

            Ok(UpdateRecordResponse {
                record_type,
                id: report.id,
                title: report.name,
                scope: report.scope,
                message: "Entity updated.".to_owned(),
            })
        }
        "observation" => {
            let report = update_observation(UpdateObservationOptions {
                project_name: None,
                start_dir,
                grafiki_home: None,
                id,
                content: clean_optional(request.content),
                category: clean_optional(request.category),
            })
            .map_err(|error| error.to_string())?;

            Ok(UpdateRecordResponse {
                record_type,
                id: report.id,
                title: report.entity_name,
                scope: report.scope,
                message: "Observation updated.".to_owned(),
            })
        }
        "relation" => {
            let report = update_relation(UpdateRelationOptions {
                project_name: None,
                start_dir,
                grafiki_home: None,
                id,
                relation: clean_optional(request.relation),
                weight: request.weight,
                confidence: request.confidence,
                source_type: clean_optional(request.source_type),
                source: clean_optional(request.source),
            })
            .map_err(|error| error.to_string())?;

            Ok(UpdateRecordResponse {
                record_type,
                id: report.id,
                title: format!(
                    "{} {} {}",
                    report.from_entity, report.relation, report.to_entity
                ),
                scope: String::new(),
                message: "Relation updated.".to_owned(),
            })
        }
        "session" => {
            let report = update_session(UpdateSessionOptions {
                project_name: None,
                start_dir,
                grafiki_home: None,
                id,
                session_type: clean_optional(request.session_type),
                status: clean_optional(request.status),
                scope: clean_optional(request.scope),
                goal: clean_optional(request.goal.or(request.title)),
                summary: clean_optional(request.summary.or(request.content)),
                accomplishments: split_optional_csv(request.accomplishments),
                remaining: split_optional_csv(request.remaining),
                files_changed: split_optional_csv(request.files_changed),
            })
            .map_err(|error| error.to_string())?;

            Ok(UpdateRecordResponse {
                record_type,
                id: report.id.clone(),
                title: report.goal.unwrap_or(report.id),
                scope: report.scope,
                message: "Session updated.".to_owned(),
            })
        }
        other => Err(format!("Unsupported memory record type: {other}")),
    }
}

#[tauri::command(async)]
fn export_memory_file(request: ExportFileRequest) -> Result<ExportFileResponse, String> {
    let output_path = required("Output path", &request.output_path)?;
    let bundle = export_memory(ExportOptions {
        project_name: None,
        start_dir: resolve_start_dir(request.start_dir),
        grafiki_home: None,
        scope: clean_optional(request.scope).unwrap_or_default(),
    })
    .map_err(|error| error.to_string())?;
    let records = bundle.entities.len()
        + bundle.relations.len()
        + bundle.observations.len()
        + bundle.decisions.len()
        + bundle.state.len()
        + bundle.context.len()
        + bundle.sessions.len();
    let json = serde_json::to_string_pretty(&bundle).map_err(|error| error.to_string())?;
    fs::write(&output_path, json).map_err(|error| error.to_string())?;

    Ok(ExportFileResponse {
        output_path,
        records,
        message: format!("Exported {records} records."),
    })
}

#[tauri::command(async)]
fn import_memory_file(request: ImportFileRequest) -> Result<ImportReport, String> {
    let input_path = required("Input path", &request.input_path)?;
    let content = fs::read_to_string(&input_path).map_err(|error| error.to_string())?;
    let bundle: ExportBundle = serde_json::from_str(&content).map_err(|error| error.to_string())?;

    import_memory(ImportOptions {
        project_name: None,
        start_dir: resolve_start_dir(request.start_dir),
        grafiki_home: None,
        bundle,
    })
    .map_err(|error| error.to_string())
}

#[tauri::command(async)]
fn process_project_embeddings(
    request: Option<ProcessEmbeddingsRequest>,
) -> Result<ProcessEmbeddingsReport, String> {
    let request = request.unwrap_or(ProcessEmbeddingsRequest {
        start_dir: None,
        scope: None,
        limit: None,
        rebuild: None,
    });

    process_embedding_jobs(ProcessEmbeddingsOptions {
        project_name: None,
        start_dir: resolve_start_dir(request.start_dir),
        grafiki_home: None,
        scope: clean_optional(request.scope).unwrap_or_else(|| "*".to_owned()),
        limit: request.limit.unwrap_or(100),
        rebuild: request.rebuild.unwrap_or(false),
    })
    .map_err(|error| error.to_string())
}

#[tauri::command(async)]
fn get_daemon_status(
    request: Option<DaemonControlRequest>,
    tokens: State<DaemonTokens>,
) -> Result<DaemonStatusResponse, String> {
    let request = request.unwrap_or(DaemonControlRequest {
        start_dir: None,
        host: None,
        port: None,
        token: None,
    });
    let start_dir = resolve_start_dir(request.start_dir);
    let cli_path = find_grafiki_cli();
    let Some(cli_path) = cli_path else {
        return Ok(DaemonStatusResponse {
            project: "Grafiki".to_owned(),
            running: false,
            pid: None,
            host: None,
            port: None,
            url: None,
            pid_path: String::new(),
            log_path: String::new(),
            cli_path: None,
            cli_available: false,
            token: None,
            message: "Grafiki CLI was not found. Rebuild the desktop debug app to refresh daemon support.".to_owned(),
        });
    };

    let output = run_grafiki_cli_json(
        &cli_path,
        &[
            "daemon",
            "status",
            "--path",
            &display_path(&start_dir),
            "--format",
            "json",
        ],
    )?;
    let status: CliDaemonStatus =
        serde_json::from_str(&output).map_err(|error| error.to_string())?;
    // Report the token only if this desktop session started the daemon (we never
    // persist it). A daemon started elsewhere / in a prior session shows none.
    let token = if status.running {
        let key = display_path(&start_dir);
        tokens.0.lock().ok().and_then(|map| map.get(&key).cloned())
    } else {
        None
    };
    Ok(DaemonStatusResponse {
        project: status.project.clone(),
        running: status.running,
        pid: status.pid,
        host: status.host.clone(),
        port: status.port,
        url: daemon_url(status.host.as_deref(), status.port),
        pid_path: display_path(&status.pid_path),
        log_path: display_path(&status.log_path),
        cli_path: Some(display_path(&cli_path)),
        cli_available: true,
        token,
        message: if status.running {
            "Daemon is running.".to_owned()
        } else {
            "Daemon is stopped.".to_owned()
        },
    })
}

#[tauri::command(async)]
fn start_daemon(
    request: DaemonControlRequest,
    tokens: State<DaemonTokens>,
) -> Result<DaemonStartResponse, String> {
    let start_dir = resolve_start_dir(request.start_dir);
    let cli_path = find_grafiki_cli().ok_or_else(|| {
        "Grafiki CLI was not found. Rebuild the desktop debug app first.".to_owned()
    })?;
    let host = clean_optional(request.host).unwrap_or_else(|| "127.0.0.1".to_owned());
    if !is_local_bind_host(&host) {
        return Err("Desktop daemon controls only allow local binds for now.".to_owned());
    }
    let port = request.port.unwrap_or(9700);
    let port_text = port.to_string();
    let start_dir_text = display_path(&start_dir);
    let args = vec![
        "daemon",
        "start",
        "--path",
        start_dir_text.as_str(),
        "--host",
        host.as_str(),
        "--port",
        port_text.as_str(),
        "--format",
        "json",
    ];
    // Use a caller-supplied token verbatim, otherwise mint a strong one so the
    // local API is never unauthenticated by default. Passed via env (not argv) so
    // it does not leak into the process table.
    let token = clean_optional(request.token).unwrap_or_else(generate_daemon_token);

    let output = run_grafiki_cli_json_env(&cli_path, &args, &[("GRAFIKI_HTTP_TOKEN", &token)])?;
    let report: CliDaemonStart =
        serde_json::from_str(&output).map_err(|error| error.to_string())?;

    let key = display_path(&start_dir);
    let effective_token = if report.already_running {
        // The running daemon keeps its original token; only report what we know.
        tokens.0.lock().ok().and_then(|map| map.get(&key).cloned())
    } else {
        if let Ok(mut map) = tokens.0.lock() {
            map.insert(key, token.clone());
        }
        Some(token)
    };

    Ok(DaemonStartResponse {
        project: report.project,
        running: report.running,
        already_running: report.already_running,
        pid: report.pid,
        host: report.host.clone(),
        port: report.port,
        url: format!("http://{}:{}", report.host, report.port),
        pid_path: display_path(&report.pid_path),
        log_path: display_path(&report.log_path),
        cli_path: display_path(&cli_path),
        token: effective_token,
        message: if report.already_running {
            "Daemon was already running.".to_owned()
        } else {
            "Daemon started.".to_owned()
        },
    })
}

#[tauri::command(async)]
fn stop_daemon(
    request: Option<DaemonControlRequest>,
    tokens: State<DaemonTokens>,
) -> Result<DaemonStopResponse, String> {
    let request = request.unwrap_or(DaemonControlRequest {
        start_dir: None,
        host: None,
        port: None,
        token: None,
    });
    let start_dir = resolve_start_dir(request.start_dir);
    if let Ok(mut map) = tokens.0.lock() {
        map.remove(&display_path(&start_dir));
    }
    let cli_path = find_grafiki_cli().ok_or_else(|| {
        "Grafiki CLI was not found. Rebuild the desktop debug app first.".to_owned()
    })?;
    let output = run_grafiki_cli_json(
        &cli_path,
        &[
            "daemon",
            "stop",
            "--path",
            &display_path(&start_dir),
            "--format",
            "json",
        ],
    )?;
    let report: CliDaemonStop = serde_json::from_str(&output).map_err(|error| error.to_string())?;
    Ok(DaemonStopResponse {
        project: report.project,
        stopped: report.stopped,
        pid: report.pid,
        pid_path: display_path(&report.pid_path),
        cli_path: display_path(&cli_path),
        message: if report.stopped {
            "Daemon stopped.".to_owned()
        } else {
            "Daemon was not running.".to_owned()
        },
    })
}

#[tauri::command]
fn capture_memory(request: CaptureRequest) -> Result<CaptureResponse, String> {
    let start_dir = resolve_start_dir(request.start_dir);
    let scope = clean_optional(request.scope).unwrap_or_default();
    let title = required("Title", &request.title)?;
    let content = clean_optional(request.content);
    let capture_type = request.capture_type.trim().to_ascii_lowercase();

    match capture_type.as_str() {
        "decision" => {
            let report = log_decision(LogDecisionOptions {
                project_name: None,
                start_dir,
                grafiki_home: None,
                title: title.clone(),
                reasoning: content,
                alternatives: split_csv(request.alternatives),
                tags: split_csv(request.tags),
                scope,
                supersedes: clean_optional(request.supersedes),
            })
            .map_err(|error| error.to_string())?;

            Ok(CaptureResponse {
                record_type: "decision".to_owned(),
                id: report.decision_id,
                title: report.title,
                scope: report.scope,
                message: "Decision captured.".to_owned(),
            })
        }
        "observation" => {
            let content = content.ok_or_else(|| "Memory text is required.".to_owned())?;
            let response_scope = scope.clone();
            let report = save_entity(SaveEntityOptions {
                project_name: None,
                start_dir,
                grafiki_home: None,
                name: title.clone(),
                entity_type: clean_optional(request.entity_type)
                    .unwrap_or_else(|| "concept".to_owned()),
                observe: Some(content),
                category: clean_optional(request.category).unwrap_or_else(|| "general".to_owned()),
                scope,
                relate: None,
            })
            .map_err(|error| error.to_string())?;

            Ok(CaptureResponse {
                record_type: "observation".to_owned(),
                id: report.observation_id.unwrap_or(report.entity_id),
                title,
                scope: response_scope,
                message: "Observation attached to entity.".to_owned(),
            })
        }
        "state" => {
            let key = clean_optional(request.key).unwrap_or_else(|| slug_key(&title, "state"));
            let report = upsert_state(UpsertStateOptions {
                project_name: None,
                start_dir,
                grafiki_home: None,
                key,
                title: title.clone(),
                status: clean_optional(request.status).unwrap_or_else(|| "in-progress".to_owned()),
                owner: None,
                details: content,
                blockers: Vec::new(),
                depends_on: Vec::new(),
                scope,
                priority: clean_optional(request.priority).unwrap_or_else(|| "medium".to_owned()),
            })
            .map_err(|error| error.to_string())?;

            Ok(CaptureResponse {
                record_type: "state".to_owned(),
                id: report.key,
                title: report.title,
                scope: report.scope,
                message: "State item saved.".to_owned(),
            })
        }
        "context" => {
            let content = content.ok_or_else(|| "Context content is required.".to_owned())?;
            let key = clean_optional(request.key).unwrap_or_else(|| slug_key(&title, "context"));
            let report = add_context(AddContextOptions {
                project_name: None,
                start_dir,
                grafiki_home: None,
                key,
                title: title.clone(),
                category: clean_optional(request.category)
                    .unwrap_or_else(|| "reference".to_owned()),
                scope,
                content,
            })
            .map_err(|error| error.to_string())?;

            Ok(CaptureResponse {
                record_type: "context".to_owned(),
                id: report.key,
                title: report.title,
                scope: report.scope,
                message: "Context document saved.".to_owned(),
            })
        }
        "relation" => {
            let target = clean_optional(request.relation_target)
                .ok_or_else(|| "Target entity id is required for a relation.".to_owned())?;
            let relation =
                clean_optional(request.relation_type).unwrap_or_else(|| "works_with".to_owned());
            let response_scope = scope.clone();
            let report = save_entity(SaveEntityOptions {
                project_name: None,
                start_dir,
                grafiki_home: None,
                name: title.clone(),
                entity_type: clean_optional(request.entity_type)
                    .unwrap_or_else(|| "concept".to_owned()),
                observe: content,
                category: clean_optional(request.category).unwrap_or_else(|| "general".to_owned()),
                scope,
                relate: Some(format!("{target}:{relation}")),
            })
            .map_err(|error| error.to_string())?;

            Ok(CaptureResponse {
                record_type: "relation".to_owned(),
                id: report.relation_id.unwrap_or(report.entity_id),
                title,
                scope: response_scope,
                message: "Relation saved.".to_owned(),
            })
        }
        "handoff" => {
            let report = handoff_session(HandoffOptions {
                project_name: None,
                start_dir,
                grafiki_home: None,
                session_id: None,
            })
            .map_err(|error| error.to_string())?;

            Ok(CaptureResponse {
                record_type: "handoff".to_owned(),
                id: report.child_session_id,
                title,
                scope: report.scope,
                message: "Handoff session created.".to_owned(),
            })
        }
        _ => Err(format!(
            "Unsupported capture type: {}",
            request.capture_type
        )),
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
fn crash_log_dir() -> Option<PathBuf> {
    let home = env::var_os("GRAFIKI_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".grafiki")))?;
    Some(home.join("logs"))
}

/// Append a bounded record of any panic to ~/.grafiki/logs/desktop-crash.log so
/// a crashing desktop build leaves a trace, while keeping the still-default
/// behavior afterward. The payload is truncated to bound accidental data.
fn install_panic_logger() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if let Some(dir) = crash_log_dir() {
            let _ = fs::create_dir_all(&dir);
            let location = info
                .location()
                .map(|loc| format!("{}:{}", loc.file(), loc.line()))
                .unwrap_or_else(|| "unknown".to_owned());
            let payload = info
                .payload()
                .downcast_ref::<&str>()
                .map(|value| value.to_string())
                .or_else(|| info.payload().downcast_ref::<String>().cloned())
                .unwrap_or_default();
            let payload: String = payload.chars().take(500).collect();
            if let Ok(mut file) = fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(dir.join("desktop-crash.log"))
            {
                use std::io::Write;
                let _ = writeln!(file, "[panic] {location} {payload}");
            }
        }
        default_hook(info);
    }));
}

pub fn run() {
    install_panic_logger();
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(DaemonTokens::default())
        .invoke_handler(tauri::generate_handler![
            get_project_snapshot,
            initialize_project,
            search_project_memory,
            chat_with_memory,
            start_grafiki_session,
            end_grafiki_session,
            handoff_grafiki_session,
            get_memory_graph,
            get_memory_record,
            list_project_context,
            list_project_state,
            list_project_sessions,
            list_project_decisions,
            list_project_relations,
            list_memory_candidates,
            list_agent_activity,
            approve_memory_candidate,
            edit_memory_candidate,
            bulk_review_memory_candidates,
            reject_memory_candidate,
            auto_capture_memory,
            start_automatic_capture,
            stop_automatic_capture,
            ingest_raw_capture_event,
            get_automatic_capture_status,
            list_raw_capture_events,
            get_capture_config,
            update_capture_config_settings,
            summarize_automatic_capture,
            import_agent_transcripts_from_disk,
            capture_screen_snapshot,
            delete_memory_record,
            update_memory_record,
            export_memory_file,
            import_memory_file,
            process_project_embeddings,
            get_daemon_status,
            start_daemon,
            stop_daemon,
            capture_memory
        ])
        .build(tauri::generate_context!())
        .expect("failed to build Grafiki desktop")
        .run(|app_handle, event| {
            if let RunEvent::ExitRequested { .. } = event {
                stop_session_daemons(app_handle);
            }
        });
}

/// Best-effort: on quit, stop any daemons this desktop session started so they do
/// not outlive the app. Drains the token map and drops the lock BEFORE the
/// blocking CLI stop calls so quit can never deadlock on the mutex.
fn stop_session_daemons(app: &AppHandle) {
    let dirs: Vec<String> = {
        let tokens = app.state::<DaemonTokens>();
        let Ok(mut map) = tokens.0.lock() else {
            return;
        };
        map.drain().map(|(dir, _token)| dir).collect()
    };
    if dirs.is_empty() {
        return;
    }
    let Some(cli_path) = find_grafiki_cli() else {
        return;
    };
    for dir in dirs {
        let _ = run_grafiki_cli_json(
            &cli_path,
            &["daemon", "stop", "--path", &dir, "--format", "json"],
        );
    }
}

fn resolve_start_dir(raw: Option<String>) -> PathBuf {
    raw.filter(|value| !value.trim().is_empty())
        .map(PathBuf::from)
        .or_else(default_workspace_root)
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."))
}

fn default_workspace_root() -> Option<PathBuf> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let root = manifest_dir.parent()?.parent()?.parent()?.to_path_buf();
    root.exists().then_some(root)
}

fn grafiki_capture_dir() -> Result<PathBuf, String> {
    let home = env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| "Could not determine HOME for capture storage.".to_owned())?;
    Ok(home.join(".grafiki").join("captures"))
}

/// Best-effort: keep at most `keep` screenshot files in `dir`, deleting the
/// oldest beyond that so snapshots don't accumulate unbounded.
fn prune_screenshots(dir: &Path, keep: usize) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut shots: Vec<(SystemTime, PathBuf)> = entries
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .map(|name| name.starts_with("screen-") && name.ends_with(".png"))
                .unwrap_or(false)
        })
        .map(|path| {
            let modified = fs::metadata(&path)
                .and_then(|meta| meta.modified())
                .unwrap_or(UNIX_EPOCH);
            (modified, path)
        })
        .collect();
    if shots.len() <= keep {
        return;
    }
    shots.sort_by_key(|(modified, _)| *modified);
    let remove = shots.len() - keep;
    for (_, path) in shots.into_iter().take(remove) {
        let _ = fs::remove_file(path);
    }
}

fn find_grafiki_cli() -> Option<PathBuf> {
    if let Ok(path) = env::var("GRAFIKI_CLI") {
        let path = PathBuf::from(path);
        if is_executable_file(&path) {
            return Some(path);
        }
    }

    let mut candidates = Vec::new();
    if let Ok(current_exe) = env::current_exe() {
        if let Some(parent) = current_exe.parent() {
            candidates.push(parent.join("grafiki"));
            candidates.push(parent.join("grafiki-cli"));
            if let Ok(entries) = fs::read_dir(parent) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                        continue;
                    };
                    if name.starts_with("grafiki-") && !name.starts_with("grafiki-desktop") {
                        candidates.push(path);
                    }
                }
            }
        }
    }
    if let Some(root) = default_workspace_root() {
        candidates.push(root.join("target/debug/grafiki"));
        candidates.push(root.join("target/release/grafiki"));
    }
    if let Some(paths) = env::var_os("PATH") {
        for path in env::split_paths(&paths) {
            candidates.push(path.join("grafiki"));
        }
    }

    candidates
        .into_iter()
        .find(|candidate| is_executable_file(candidate))
}

fn is_executable_file(path: &Path) -> bool {
    path.is_file()
}

/// Per-session store of the HTTP token the desktop generated for each project's
/// daemon, keyed by resolved start_dir. Lets the status command report the active
/// token without persisting the secret to disk.
#[derive(Default)]
struct DaemonTokens(Mutex<HashMap<String, String>>);

/// A cryptographically strong 256-bit token (64 lowercase hex chars), backed by
/// the OS CSPRNG. Used to authenticate external agents hitting the local daemon.
fn generate_daemon_token() -> String {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).expect("OS RNG unavailable");
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn run_grafiki_cli_json(cli_path: &Path, args: &[&str]) -> Result<String, String> {
    run_grafiki_cli_json_env(cli_path, args, &[])
}

fn run_grafiki_cli_json_env(
    cli_path: &Path,
    args: &[&str],
    envs: &[(&str, &str)],
) -> Result<String, String> {
    let mut command = ProcessCommand::new(cli_path);
    command.args(args).stdin(Stdio::null());
    for (key, value) in envs {
        command.env(key, value);
    }
    let output = command
        .output()
        .map_err(|error| format!("Failed to run {}: {error}", cli_path.display()))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        let message = if !stderr.is_empty() { stderr } else { stdout };
        return Err(if message.is_empty() {
            format!("Grafiki CLI exited with status {}", output.status)
        } else {
            message
        });
    }
    String::from_utf8(output.stdout).map_err(|error| error.to_string())
}

fn daemon_url(host: Option<&str>, port: Option<u16>) -> Option<String> {
    Some(format!("http://{}:{}", host?, port?))
}

fn is_local_bind_host(host: &str) -> bool {
    matches!(host.trim(), "127.0.0.1" | "localhost" | "::1")
}

fn display_path(path: &std::path::Path) -> String {
    path.display().to_string()
}

fn auto_capture_working_tree(
    start_dir: PathBuf,
    scope: String,
    source: String,
    limit: usize,
) -> Result<AutoCaptureResponse, String> {
    let path = display_path(&start_dir);
    let bounded_limit = limit.clamp(1, 200);
    let git_root =
        desktop_git_output(&start_dir, &["rev-parse", "--show-toplevel"]).unwrap_or_default();
    let (status, diff_stat, last_commit, captured_from) = if git_root.is_some() {
        let status_text =
            desktop_git_output(&start_dir, &["status", "--porcelain=v1"])?.unwrap_or_default();
        let last_commit = desktop_git_output(&start_dir, &["log", "-1", "--pretty=format:%h %s"])?
            .filter(|value| !value.trim().is_empty());
        (
            parse_git_status(&status_text),
            desktop_capture_diff_stat(&start_dir)?,
            last_commit,
            "working tree changes",
        )
    } else {
        let recent_files = recent_workspace_files(&start_dir, bounded_limit)?;
        (
            GitStatusSummary {
                changed_files: recent_files.clone(),
                staged_files: Vec::new(),
                unstaged_files: recent_files,
                untracked_files: Vec::new(),
            },
            "No git working tree was detected. Grafiki captured recently modified workspace files instead.".to_owned(),
            None,
            "recent workspace files",
        )
    };

    if status.changed_files.is_empty() {
        return Ok(AutoCaptureResponse {
            scope,
            source,
            path,
            git_root,
            changed_files: status.changed_files,
            staged_files: status.staged_files,
            unstaged_files: status.unstaged_files,
            untracked_files: status.untracked_files,
            diff_stat,
            last_commit,
            candidates: Vec::new(),
            message: format!("No {captured_from} found, so Grafiki did not propose new memory."),
        });
    }

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0))
        .as_secs();
    let summary = auto_capture_summary(
        &source,
        &path,
        git_root.as_deref(),
        last_commit.as_deref(),
        &status,
        &diff_stat,
        bounded_limit,
    );
    let evidence = vec![EvidenceInput {
        source_event_id: None,
        source_type: "git".to_owned(),
        source: git_root.as_ref().cloned().or_else(|| Some(path.clone())),
        title: Some("Git working tree snapshot".to_owned()),
        excerpt: summary.clone(),
        uri: None,
        byte_start: None,
        byte_end: None,
        line_start: None,
        line_end: None,
        captured_at: None,
    }];

    let context_candidate = propose_candidate(ProposeCandidateOptions {
        project_name: None,
        start_dir: start_dir.clone(),
        grafiki_home: None,
        source_type: "desktop:auto-capture".to_owned(),
        source: Some(source.clone()),
        record_type: "context".to_owned(),
        payload: serde_json::json!({
            "key": format!("auto-capture-{timestamp}"),
            "title": "Auto-captured coding session snapshot",
            "category": "audit",
            "content": summary,
        }),
        scope: scope.clone(),
        confidence: 0.72,
        rationale: Some(
            "Generated from git working tree status and diff statistics; user review is required."
                .to_owned(),
        ),
        evidence: evidence.clone(),
    })
    .map_err(|error| error.to_string())?;
    let state_candidate = propose_candidate(ProposeCandidateOptions {
        project_name: None,
        start_dir,
        grafiki_home: None,
        source_type: "desktop:auto-capture".to_owned(),
        source: Some(source.clone()),
        record_type: "state".to_owned(),
        payload: serde_json::json!({
            "key": format!("auto-capture-review-{timestamp}"),
            "title": "Review auto-captured coding session changes",
            "status": "needs-review",
            "priority": "medium",
            "details": summary,
        }),
        scope: scope.clone(),
        confidence: 0.66,
        rationale: Some(
            "Generated from git working tree status and diff statistics; user review is required."
                .to_owned(),
        ),
        evidence,
    })
    .map_err(|error| error.to_string())?;

    Ok(AutoCaptureResponse {
        scope,
        source,
        path,
        git_root,
        changed_files: status.changed_files,
        staged_files: status.staged_files,
        unstaged_files: status.unstaged_files,
        untracked_files: status.untracked_files,
        diff_stat,
        last_commit,
        candidates: vec![context_candidate, state_candidate],
        message: format!("Auto-captured {captured_from} into pending memory candidates."),
    })
}

fn desktop_git_output(path: &Path, args: &[&str]) -> Result<Option<String>, String> {
    let output = ProcessCommand::new("git")
        .arg("-C")
        .arg(path)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .map_err(|error| error.to_string())?;
    if !output.status.success() {
        return Ok(None);
    }
    Ok(Some(
        String::from_utf8_lossy(&output.stdout).trim().to_owned(),
    ))
}

fn desktop_capture_diff_stat(path: &Path) -> Result<String, String> {
    let mut sections = Vec::new();
    if let Some(stat) = desktop_git_output(path, &["diff", "--stat"])? {
        if !stat.trim().is_empty() {
            sections.push(format!("Unstaged diff:\n{stat}"));
        }
    }
    if let Some(stat) = desktop_git_output(path, &["diff", "--cached", "--stat"])? {
        if !stat.trim().is_empty() {
            sections.push(format!("Staged diff:\n{stat}"));
        }
    }
    if sections.is_empty() {
        Ok("No tracked-file diff stat. Changes may be untracked or metadata-only.".to_owned())
    } else {
        Ok(sections.join("\n\n"))
    }
}

#[derive(Debug)]
struct RecentWorkspaceFile {
    path: String,
    modified_secs: u64,
}

fn recent_workspace_files(root: &Path, limit: usize) -> Result<Vec<String>, String> {
    let mut files = Vec::new();
    collect_recent_workspace_files(root, root, &mut files)?;
    files.sort_by(|left, right| {
        right
            .modified_secs
            .cmp(&left.modified_secs)
            .then_with(|| left.path.cmp(&right.path))
    });
    Ok(files
        .into_iter()
        .take(limit)
        .map(|file| file.path)
        .collect())
}

fn collect_recent_workspace_files(
    root: &Path,
    current: &Path,
    files: &mut Vec<RecentWorkspaceFile>,
) -> Result<(), String> {
    if files.len() > 5000 {
        return Ok(());
    }
    let entries = fs::read_dir(current).map_err(|error| error.to_string())?;
    for entry in entries {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        if is_ignored_workspace_path(root, &path) {
            continue;
        }
        let file_type = entry.file_type().map_err(|error| error.to_string())?;
        if file_type.is_dir() {
            collect_recent_workspace_files(root, &path, files)?;
        } else if file_type.is_file() {
            let metadata = entry.metadata().map_err(|error| error.to_string())?;
            let modified_secs = metadata
                .modified()
                .ok()
                .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
                .map(|duration| duration.as_secs())
                .unwrap_or(0);
            let relative = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .display()
                .to_string();
            files.push(RecentWorkspaceFile {
                path: relative,
                modified_secs,
            });
        }
    }
    Ok(())
}

fn is_ignored_workspace_path(root: &Path, path: &Path) -> bool {
    let relative = path.strip_prefix(root).unwrap_or(path);
    relative.components().any(|component| {
        matches!(
            component.as_os_str().to_string_lossy().as_ref(),
            ".git"
                | ".grafiki"
                | ".tauri"
                | "target"
                | "node_modules"
                | "dist"
                | ".DS_Store"
                | "build"
        )
    })
}

fn parse_git_status(status_text: &str) -> GitStatusSummary {
    let mut summary = GitStatusSummary::default();
    for line in status_text.lines() {
        // Porcelain v1 lines are `XY<space>PATH`; bytes 0..3 are ASCII. Guard the
        // slice boundaries defensively so unexpected output can never panic.
        if line.len() < 3 || !line.is_char_boundary(2) || !line.is_char_boundary(3) {
            continue;
        }
        let status = &line[..2];
        let path = normalize_git_status_path(&line[3..]);
        if path.is_empty() {
            continue;
        }
        unique_push(&mut summary.changed_files, path.clone());
        if status == "??" {
            unique_push(&mut summary.untracked_files, path);
            continue;
        }
        let bytes = status.as_bytes();
        if bytes.first().copied().unwrap_or(b' ') != b' ' {
            unique_push(&mut summary.staged_files, path.clone());
        }
        if bytes.get(1).copied().unwrap_or(b' ') != b' ' {
            unique_push(&mut summary.unstaged_files, path);
        }
    }
    summary
}

fn normalize_git_status_path(path: &str) -> String {
    let trimmed = path.trim();
    let renamed = trimmed
        .rsplit_once(" -> ")
        .map(|(_, right)| right)
        .unwrap_or(trimmed);
    renamed.trim_matches('"').to_owned()
}

fn unique_push(items: &mut Vec<String>, item: String) {
    if !items.iter().any(|existing| existing == &item) {
        items.push(item);
    }
}

fn auto_capture_summary(
    source: &str,
    path: &str,
    git_root: Option<&str>,
    last_commit: Option<&str>,
    status: &GitStatusSummary,
    diff_stat: &str,
    limit: usize,
) -> String {
    let mut sections = vec![
        "Auto-captured coding session snapshot.".to_owned(),
        format!("Source: {source}"),
        format!("Path: {path}"),
    ];
    if let Some(root) = git_root {
        sections.push(format!("Git root: {root}"));
    }
    if let Some(commit) = last_commit {
        sections.push(format!("Last commit: {commit}"));
    }
    sections.push(format_file_section(
        "Changed files",
        &status.changed_files,
        limit,
    ));
    sections.push(format_file_section(
        "Staged files",
        &status.staged_files,
        limit,
    ));
    sections.push(format_file_section(
        "Unstaged files",
        &status.unstaged_files,
        limit,
    ));
    sections.push(format_file_section(
        "Untracked files",
        &status.untracked_files,
        limit,
    ));
    sections.push(format!("Diff stat:\n{diff_stat}"));
    sections.join("\n\n")
}

fn format_file_section(title: &str, files: &[String], limit: usize) -> String {
    if files.is_empty() {
        return format!("{title}: none");
    }
    let mut lines = vec![format!("{title} ({}):", files.len())];
    for file in files.iter().take(limit) {
        lines.push(format!("- {file}"));
    }
    if files.len() > limit {
        lines.push(format!("- ... {} more", files.len() - limit));
    }
    lines.join("\n")
}

fn clean_optional(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn required(label: &str, value: &str) -> Result<String, String> {
    let value = value.trim();
    if value.is_empty() {
        Err(format!("{label} is required."))
    } else {
        Ok(value.to_owned())
    }
}

fn split_csv(value: Option<String>) -> Vec<String> {
    value
        .as_deref()
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn split_optional_csv(value: Option<String>) -> Option<Vec<String>> {
    value.map(|value| {
        value
            .split(',')
            .map(str::trim)
            .filter(|item| !item.is_empty())
            .map(ToOwned::to_owned)
            .collect()
    })
}

fn normalize_record_type(record_type: &str) -> String {
    match record_type.trim().to_ascii_lowercase().as_str() {
        "entities" => "entity".to_owned(),
        "observations" => "observation".to_owned(),
        "decisions" => "decision".to_owned(),
        "contexts" => "context".to_owned(),
        "state_item" | "states" => "state".to_owned(),
        "relations" => "relation".to_owned(),
        "sessions" => "session".to_owned(),
        other => other.to_owned(),
    }
}

fn metadata(label: &str, value: impl Into<String>) -> DetailMetadata {
    DetailMetadata {
        label: label.to_owned(),
        value: value.into(),
    }
}

fn display_scope(scope: &str) -> String {
    if scope.is_empty() {
        "global".to_owned()
    } else {
        scope.to_owned()
    }
}

fn detail_events(
    start_dir: &std::path::Path,
    scope: &str,
    record_type: &str,
    id: &str,
) -> Vec<DetailEvent> {
    list_events(EventListOptions {
        project_name: None,
        start_dir: start_dir.to_path_buf(),
        grafiki_home: None,
        scope: scope.to_owned(),
        since: None,
        limit: 100,
    })
    .map(|report| {
        report
            .events
            .into_iter()
            .filter(|event| {
                event.target_id == id
                    || normalize_record_type(&event.target_type) == record_type
                        && event.summary.contains(id)
            })
            .take(8)
            .map(|event| DetailEvent {
                id: event.id,
                event_type: event.event_type,
                summary: event.summary,
                created_at: event.created_at,
            })
            .collect()
    })
    .unwrap_or_default()
}

fn related_for_entity(
    bundle: &grafiki_core::ExportBundle,
    entity_id: &str,
) -> Vec<RelatedMemoryRecord> {
    bundle
        .relations
        .iter()
        .filter_map(|relation| {
            if relation.from_entity == entity_id {
                Some(RelatedMemoryRecord {
                    record_type: "entity".to_owned(),
                    id: relation.to_entity.clone(),
                    title: entity_title(bundle, &relation.to_entity),
                    relation: relation.relation.clone(),
                })
            } else if relation.to_entity == entity_id {
                Some(RelatedMemoryRecord {
                    record_type: "entity".to_owned(),
                    id: relation.from_entity.clone(),
                    title: entity_title(bundle, &relation.from_entity),
                    relation: format!("reverse {}", relation.relation),
                })
            } else {
                None
            }
        })
        .take(12)
        .collect()
}

fn entity_title(bundle: &grafiki_core::ExportBundle, entity_id: &str) -> String {
    bundle
        .entities
        .iter()
        .find(|entity| entity.id == entity_id)
        .map(|entity| entity.name.clone())
        .unwrap_or_else(|| entity_id.to_owned())
}

fn slug_key(title: &str, prefix: &str) -> String {
    let mut slug = String::new();
    let mut last_was_dash = false;

    for character in title.chars() {
        if character.is_ascii_alphanumeric() {
            slug.push(character.to_ascii_lowercase());
            last_was_dash = false;
        } else if !last_was_dash && !slug.is_empty() {
            slug.push('-');
            last_was_dash = true;
        }
    }

    while slug.ends_with('-') {
        slug.pop();
    }

    if slug.is_empty() {
        prefix.to_owned()
    } else {
        slug
    }
}

#[cfg(test)]
mod tests {
    use super::generate_daemon_token;

    #[test]
    fn daemon_token_is_strong_and_unique() {
        let a = generate_daemon_token();
        let b = generate_daemon_token();
        assert_eq!(a.len(), 64, "256-bit token = 64 hex chars");
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a, b, "tokens must not repeat");
    }
}
