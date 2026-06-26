// See grafiki-core/src/lib.rs: allow a few intentional, pre-existing API shapes
// so `clippy -D warnings` stays green in CI.
#![allow(
    clippy::too_many_arguments,
    clippy::ptr_arg,
    clippy::large_enum_variant
)]

use std::collections::HashMap;
use std::env;
use std::fs;
use std::fs::OpenOptions;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command as ProcessCommand, Stdio};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use clap::{Parser, Subcommand, ValueEnum};
use grafiki_core::{
    add_context, approve_candidate, ask_memory, bulk_review_candidates, delete_context,
    delete_decision, delete_entity, delete_observation, delete_relation, delete_state,
    edit_candidate, end_session, export_memory, generate_report, get_capture_status, get_context,
    get_embedding_status, get_graph, get_memory_record_detail, get_status, handoff_session,
    import_agent_transcripts, import_memory, ingest_capture_event, init_project,
    list_agent_queries, list_candidates, list_capture_events, list_context, list_events,
    list_sessions, list_state, load_capture_config, log_decision, process_embedding_jobs,
    propose_candidate, propose_capture_candidates, reject_candidate, save_entity, search_memory,
    start_capture_session, start_session, stop_capture_session, update_capture_config,
    update_context, update_decision, update_entity, update_observation, update_relation,
    update_session, upsert_state, AddContextOptions, AgentMemoryBriefing,
    AgentTranscriptImportReport, ApproveCandidateOptions, AskMemoryOptions,
    BulkCandidateReviewOptions, BulkCandidateReviewReport, CandidateMutationReport,
    CaptureCandidateReport, CaptureConfigOptions, CaptureConfigReport, CaptureEvent,
    CaptureEventReport, CaptureSessionReport, CaptureSourceUpdates, CaptureStatusOptions,
    CaptureStatusReport, ContextListOptions, DeleteContextOptions, DeleteDecisionOptions,
    DeleteEntityOptions, DeleteObservationOptions, DeleteRelationOptions, DeleteStateOptions,
    EditCandidateOptions, EmbeddingStatusOptions, EmbeddingStatusReport, EndSessionOptions,
    EventListOptions, EvidenceInput, ExportOptions, GetContextOptions, GetMemoryRecordOptions,
    GraphOptions, HandoffOptions, ImportAgentTranscriptsOptions, ImportOptions,
    IngestCaptureEventOptions, InitOptions, ListAgentQueriesOptions, ListCandidatesOptions,
    ListCaptureEventsOptions, LogDecisionOptions, ProcessEmbeddingsOptions,
    ProcessEmbeddingsReport, ProjectReportOptions, ProjectResolveOptions, ProposeCandidateOptions,
    ProposeCaptureCandidatesOptions, RejectCandidateOptions, SaveEntityOptions, Scope,
    SearchMemoryOptions, SearchMode as CoreSearchMode, SessionLogOptions, StartCaptureOptions,
    StartSessionOptions, StateListOptions, StatusOptions, StopCaptureOptions,
    UpdateCaptureConfigOptions, UpdateContextOptions, UpdateDecisionOptions, UpdateEntityOptions,
    UpdateObservationOptions, UpdateRelationOptions, UpdateSessionOptions, UpsertStateOptions,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Parser)]
#[command(name = "grafiki")]
#[command(about = "Local-first project memory for AI coding sessions")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Initialize Grafiki for a project.
    Init {
        /// Project name. Defaults to the project directory name.
        project_name: Option<String>,

        /// Project directory where the .grafiki marker should be written.
        #[arg(long, default_value = ".")]
        path: PathBuf,

        #[arg(long, value_enum, default_value_t = OutputFormat::Plain)]
        format: OutputFormat,
    },

    /// Start a Grafiki-tracked AI coding session.
    Start {
        /// Explicit project name. Defaults to .grafiki detection, then directory name.
        #[arg(long)]
        project: Option<String>,

        /// Session tool/client type.
        #[arg(long = "type", default_value = "codex")]
        session_type: String,

        /// Session goal.
        #[arg(long)]
        goal: String,

        /// Slash-delimited scope.
        #[arg(long, default_value = "")]
        scope: String,

        /// Directory used for project detection.
        #[arg(long, default_value = ".")]
        path: PathBuf,

        #[arg(long, value_enum, default_value_t = OutputFormat::Plain)]
        format: OutputFormat,
    },

    /// End an active Grafiki session.
    End {
        /// Explicit project name. Defaults to .grafiki detection, then directory name.
        #[arg(long)]
        project: Option<String>,

        /// Session ID. Defaults to the latest active session.
        #[arg(long)]
        session: Option<String>,

        /// End status.
        #[arg(long, default_value = "completed")]
        status: String,

        /// Short session summary.
        #[arg(long)]
        summary: Option<String>,

        /// Comma-separated accomplishments.
        #[arg(long)]
        accomplishments: Option<String>,

        /// Comma-separated remaining items.
        #[arg(long)]
        remaining: Option<String>,

        /// Comma-separated files changed.
        #[arg(long)]
        files: Option<String>,

        /// Directory used for project detection.
        #[arg(long, default_value = ".")]
        path: PathBuf,

        #[arg(long, value_enum, default_value_t = OutputFormat::Plain)]
        format: OutputFormat,
    },

    /// Hand off the current session into a linked child session.
    Handoff {
        /// Explicit project name. Defaults to .grafiki detection, then directory name.
        #[arg(long)]
        project: Option<String>,

        /// Session ID. Defaults to the latest active session.
        #[arg(long)]
        session: Option<String>,

        /// Directory used for project detection.
        #[arg(long, default_value = ".")]
        path: PathBuf,

        #[arg(long, value_enum, default_value_t = OutputFormat::Plain)]
        format: OutputFormat,
    },

    /// Log an architectural or product decision.
    Decide {
        /// Decision title.
        title: String,

        /// Explicit project name. Defaults to .grafiki detection, then directory name.
        #[arg(long)]
        project: Option<String>,

        /// Decision reasoning.
        #[arg(long)]
        reasoning: Option<String>,

        /// Comma-separated alternatives considered.
        #[arg(long)]
        alternatives: Option<String>,

        /// Comma-separated tags.
        #[arg(long)]
        tags: Option<String>,

        /// Slash-delimited scope.
        #[arg(long, default_value = "")]
        scope: String,

        /// Decision ID superseded by this decision.
        #[arg(long)]
        supersedes: Option<String>,

        /// Directory used for project detection.
        #[arg(long, default_value = ".")]
        path: PathBuf,

        #[arg(long, value_enum, default_value_t = OutputFormat::Plain)]
        format: OutputFormat,
    },

    /// Save an entity and optionally attach an observation.
    Save {
        /// Entity display name.
        entity_name: String,

        /// Explicit project name. Defaults to .grafiki detection, then directory name.
        #[arg(long)]
        project: Option<String>,

        /// Entity type.
        #[arg(long = "type")]
        entity_type: String,

        /// Observation text to attach.
        #[arg(long)]
        observe: Option<String>,

        /// Observation category.
        #[arg(long, default_value = "general")]
        category: String,

        /// Slash-delimited scope.
        #[arg(long, default_value = "")]
        scope: String,

        /// Relation in target-id:relation form.
        #[arg(long)]
        relate: Option<String>,

        /// Directory used for project detection.
        #[arg(long, default_value = ".")]
        path: PathBuf,

        #[arg(long, value_enum, default_value_t = OutputFormat::Plain)]
        format: OutputFormat,
    },

    /// Search Grafiki memory.
    Search {
        /// Search query.
        query: String,

        /// Explicit project name. Defaults to .grafiki detection, then directory name.
        #[arg(long)]
        project: Option<String>,

        /// Record type: all, entities, observations, decisions, context.
        #[arg(long = "type", default_value = "all")]
        record_type: String,

        /// Search mode: keyword, semantic, hybrid.
        #[arg(long, value_enum, default_value_t = SearchModeArg::Keyword)]
        mode: SearchModeArg,

        /// Slash-delimited scope.
        #[arg(long, default_value = "")]
        scope: String,

        /// Maximum results.
        #[arg(long, default_value_t = 10)]
        limit: usize,

        /// Directory used for project detection.
        #[arg(long, default_value = ".")]
        path: PathBuf,

        #[arg(long, value_enum, default_value_t = OutputFormat::Plain)]
        format: OutputFormat,
    },

    /// Ask Grafiki for an agent-ready memory briefing.
    Ask {
        /// Question or task the coding agent needs memory for.
        question: String,

        /// Explicit project name. Defaults to .grafiki detection, then directory name.
        #[arg(long)]
        project: Option<String>,

        /// Slash-delimited scope.
        #[arg(long, default_value = "")]
        scope: String,

        /// Maximum relevant memory records.
        #[arg(long, default_value_t = 8)]
        limit: usize,

        /// Calling agent/client name for the audit log.
        #[arg(long, default_value = "cli")]
        agent: String,

        /// Directory used for project detection.
        #[arg(long, default_value = ".")]
        path: PathBuf,

        #[arg(long, value_enum, default_value_t = OutputFormat::Plain)]
        format: OutputFormat,
    },

    /// Show what agents asked Grafiki and what memory was returned.
    AgentActivity {
        /// Explicit project name. Defaults to .grafiki detection, then directory name.
        #[arg(long)]
        project: Option<String>,

        /// Slash-delimited scope.
        #[arg(long, default_value = "")]
        scope: String,

        /// Maximum query log entries.
        #[arg(long, default_value_t = 20)]
        limit: usize,

        /// Directory used for project detection.
        #[arg(long, default_value = ".")]
        path: PathBuf,

        #[arg(long, value_enum, default_value_t = OutputFormat::Plain)]
        format: OutputFormat,
    },

    /// Capture the current git working session as reviewable memory candidates.
    AutoCapture {
        /// Explicit project name. Defaults to .grafiki detection, then directory name.
        #[arg(long)]
        project: Option<String>,

        /// Slash-delimited scope.
        #[arg(long, default_value = "")]
        scope: String,

        /// Directory used for project detection and git inspection.
        #[arg(long, default_value = ".")]
        path: PathBuf,

        /// Human-readable source label for this capture.
        #[arg(long)]
        source: Option<String>,

        /// Maximum changed files to include in the generated summary.
        #[arg(long, default_value_t = 25)]
        limit: usize,

        #[arg(long, value_enum, default_value_t = OutputFormat::Plain)]
        format: OutputFormat,
    },

    /// Manage automatic transcript, screen, IDE, and agent capture.
    Capture {
        #[command(subcommand)]
        command: CaptureCommand,
    },

    /// Traverse the knowledge graph from an entity.
    Graph {
        /// Root entity ID.
        entity_id: String,

        /// Explicit project name. Defaults to .grafiki detection, then directory name.
        #[arg(long)]
        project: Option<String>,

        /// Traversal depth.
        #[arg(long, default_value_t = 2)]
        depth: usize,

        /// Directory used for project detection.
        #[arg(long, default_value = ".")]
        path: PathBuf,

        #[arg(long, value_enum, default_value_t = GraphOutputFormat::Plain)]
        format: GraphOutputFormat,
    },

    /// Generate a lightweight project memory report.
    Report {
        /// Explicit project name. Defaults to .grafiki detection, then directory name.
        #[arg(long)]
        project: Option<String>,

        /// Slash-delimited scope.
        #[arg(long, default_value = "")]
        scope: String,

        /// Directory used for project detection.
        #[arg(long, default_value = ".")]
        path: PathBuf,

        /// Optional output path. Prints to stdout when omitted.
        #[arg(long)]
        output: Option<PathBuf>,

        #[arg(long, value_enum, default_value_t = OutputFormat::Plain)]
        format: OutputFormat,
    },

    /// Analyze the project memory graph and suggest follow-up questions.
    Analyze {
        /// Explicit project name. Defaults to .grafiki detection, then directory name.
        #[arg(long)]
        project: Option<String>,

        /// Slash-delimited scope.
        #[arg(long, default_value = "")]
        scope: String,

        /// Directory used for project detection.
        #[arg(long, default_value = ".")]
        path: PathBuf,

        /// Optional output path. Prints to stdout when omitted.
        #[arg(long)]
        output: Option<PathBuf>,

        #[arg(long, value_enum, default_value_t = OutputFormat::Md)]
        format: OutputFormat,
    },

    /// Export scoped project memory.
    Export {
        /// Explicit project name. Defaults to .grafiki detection, then directory name.
        #[arg(long)]
        project: Option<String>,

        /// Slash-delimited scope.
        #[arg(long, default_value = "")]
        scope: String,

        /// Directory used for project detection.
        #[arg(long, default_value = ".")]
        path: PathBuf,

        /// Optional output path. Prints to stdout when omitted.
        #[arg(long)]
        output: Option<PathBuf>,

        #[arg(long, value_enum, default_value_t = ExportOutputFormat::Json)]
        format: ExportOutputFormat,
    },

    /// Import a Grafiki JSON export into the local project.
    Import {
        /// JSON export file created by `grafiki export --format json`.
        input: PathBuf,

        /// Explicit project name. Defaults to .grafiki detection, then directory name.
        #[arg(long)]
        project: Option<String>,

        /// Directory used for project detection.
        #[arg(long, default_value = ".")]
        path: PathBuf,

        #[arg(long, value_enum, default_value_t = OutputFormat::Plain)]
        format: OutputFormat,
    },

    /// Inspect semantic-search embedding jobs.
    Embeddings {
        #[command(subcommand)]
        command: EmbeddingsCommand,
    },

    /// Run a local HTTP API server.
    Serve {
        /// Explicit project name. Defaults to .grafiki detection, then directory name.
        #[arg(long)]
        project: Option<String>,

        /// Directory used for project detection.
        #[arg(long, default_value = ".")]
        path: PathBuf,

        /// Host interface to bind.
        #[arg(long, default_value = "127.0.0.1")]
        host: String,

        /// TCP port to bind.
        #[arg(long, default_value_t = 9700)]
        port: u16,

        /// Allow binding to a non-local interface.
        #[arg(long)]
        allow_non_local: bool,

        /// Require this token for HTTP API requests. Can also be set with GRAFIKI_HTTP_TOKEN.
        #[arg(long, env = "GRAFIKI_HTTP_TOKEN")]
        token: Option<String>,
    },

    /// Manage the local Grafiki HTTP daemon.
    Daemon {
        #[command(subcommand)]
        command: DaemonCommand,
    },

    /// Run a stdio MCP server.
    Mcp {
        /// Explicit project name. Defaults to .grafiki detection, then directory name.
        #[arg(long)]
        project: Option<String>,

        /// Directory used for project detection.
        #[arg(long, default_value = ".")]
        path: PathBuf,
    },

    /// Show active sessions, work, decisions, and recent events.
    Status {
        /// Explicit project name. Defaults to .grafiki detection, then directory name.
        #[arg(long)]
        project: Option<String>,

        /// Slash-delimited scope.
        #[arg(long, default_value = "")]
        scope: String,

        /// Directory used for project detection.
        #[arg(long, default_value = ".")]
        path: PathBuf,

        #[arg(long, value_enum, default_value_t = OutputFormat::Plain)]
        format: OutputFormat,
    },

    /// Create or update tracked work state.
    State {
        #[command(subcommand)]
        command: StateCommand,
    },

    /// Review untrusted memory candidates before approving them.
    Candidates {
        #[command(subcommand)]
        command: CandidateCommand,
    },

    /// Show recent mutation events.
    Events {
        /// Explicit project name. Defaults to .grafiki detection, then directory name.
        #[arg(long)]
        project: Option<String>,

        /// Slash-delimited scope.
        #[arg(long, default_value = "")]
        scope: String,

        /// Only events after this ULID.
        #[arg(long)]
        since: Option<String>,

        /// Maximum events.
        #[arg(long = "last", default_value_t = 20)]
        limit: usize,

        /// Directory used for project detection.
        #[arg(long, default_value = ".")]
        path: PathBuf,

        #[arg(long, value_enum, default_value_t = OutputFormat::Plain)]
        format: OutputFormat,
    },

    /// Show session history.
    Log {
        /// Explicit project name. Defaults to .grafiki detection, then directory name.
        #[arg(long)]
        project: Option<String>,

        /// Slash-delimited scope.
        #[arg(long, default_value = "")]
        scope: String,

        /// Session type filter.
        #[arg(long = "type")]
        session_type: Option<String>,

        /// Maximum sessions.
        #[arg(long = "last", default_value_t = 20)]
        limit: usize,

        /// Directory used for project detection.
        #[arg(long, default_value = ".")]
        path: PathBuf,

        #[arg(long, value_enum, default_value_t = OutputFormat::Plain)]
        format: OutputFormat,
    },

    /// Manage durable project context documents.
    Context {
        #[command(subcommand)]
        command: ContextCommand,
    },

    /// Development helper for validating scope-chain behavior.
    ScopeChain {
        /// Slash-delimited scope, for example: open-insurance/backend.
        scope: String,

        #[arg(long, value_enum, default_value_t = OutputFormat::Plain)]
        format: OutputFormat,
    },
}

#[derive(Debug, Subcommand)]
enum DaemonCommand {
    /// Start Grafiki HTTP API in the background.
    Start {
        /// Explicit project name. Defaults to .grafiki detection, then directory name.
        #[arg(long)]
        project: Option<String>,

        /// Directory used for project detection.
        #[arg(long, default_value = ".")]
        path: PathBuf,

        /// Host interface to bind.
        #[arg(long, default_value = "127.0.0.1")]
        host: String,

        /// TCP port to bind.
        #[arg(long, default_value_t = 9700)]
        port: u16,

        /// Allow binding to a non-local interface.
        #[arg(long)]
        allow_non_local: bool,

        /// Require this token for HTTP API requests. Can also be set with GRAFIKI_HTTP_TOKEN.
        #[arg(long, env = "GRAFIKI_HTTP_TOKEN")]
        token: Option<String>,

        /// Optional log file. Defaults to ~/.grafiki/daemons/<project>.log.
        #[arg(long)]
        log: Option<PathBuf>,

        #[arg(long, value_enum, default_value_t = OutputFormat::Plain)]
        format: OutputFormat,
    },

    /// Show Grafiki daemon status for this project.
    Status {
        /// Explicit project name. Defaults to .grafiki detection, then directory name.
        #[arg(long)]
        project: Option<String>,

        /// Directory used for project detection.
        #[arg(long, default_value = ".")]
        path: PathBuf,

        #[arg(long, value_enum, default_value_t = OutputFormat::Plain)]
        format: OutputFormat,
    },

    /// Stop Grafiki daemon for this project.
    Stop {
        /// Explicit project name. Defaults to .grafiki detection, then directory name.
        #[arg(long)]
        project: Option<String>,

        /// Directory used for project detection.
        #[arg(long, default_value = ".")]
        path: PathBuf,

        #[arg(long, value_enum, default_value_t = OutputFormat::Plain)]
        format: OutputFormat,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum OutputFormat {
    Plain,
    Md,
    Json,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum GraphOutputFormat {
    Plain,
    Json,
    Dot,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ExportOutputFormat {
    Json,
    Md,
    Wiki,
    Dot,
    #[value(name = "graphml")]
    Graphml,
    Html,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum SearchModeArg {
    Keyword,
    Semantic,
    Hybrid,
}

impl From<SearchModeArg> for CoreSearchMode {
    fn from(value: SearchModeArg) -> Self {
        match value {
            SearchModeArg::Keyword => Self::Keyword,
            SearchModeArg::Semantic => Self::Semantic,
            SearchModeArg::Hybrid => Self::Hybrid,
        }
    }
}

#[derive(Debug, Subcommand)]
enum CaptureCommand {
    /// Show or update workspace capture consent settings.
    Config {
        #[command(subcommand)]
        command: CaptureConfigCommand,
    },

    /// Start an automatic capture session.
    Start {
        #[arg(long)]
        project: Option<String>,

        #[arg(long, default_value = "")]
        scope: String,

        #[arg(long, default_value = ".")]
        path: PathBuf,

        #[arg(long)]
        source_app: Option<String>,

        #[arg(long)]
        consent_profile: Option<String>,

        #[arg(long)]
        redaction_profile: Option<String>,

        #[arg(long, value_enum, default_value_t = OutputFormat::Plain)]
        format: OutputFormat,
    },

    /// Stop an automatic capture session.
    Stop {
        id: String,

        #[arg(long)]
        project: Option<String>,

        #[arg(long, default_value = ".")]
        path: PathBuf,

        #[arg(long, value_enum, default_value_t = OutputFormat::Plain)]
        format: OutputFormat,
    },

    /// Ingest one transcript, screen, IDE, terminal, file, git, or agent event.
    Ingest {
        #[arg(long)]
        project: Option<String>,

        #[arg(long)]
        capture: Option<String>,

        #[arg(long = "type")]
        source_type: String,

        #[arg(long)]
        source: Option<String>,

        #[arg(long)]
        title: Option<String>,

        #[arg(long)]
        text: Option<String>,

        #[arg(long)]
        file: Option<PathBuf>,

        #[arg(long)]
        payload: Option<String>,

        #[arg(long)]
        metadata: Option<String>,

        #[arg(long, default_value = "internal")]
        privacy: String,

        #[arg(long)]
        redacted: bool,

        #[arg(long)]
        captured_at: Option<String>,

        #[arg(long, default_value = "")]
        scope: String,

        #[arg(long, default_value = ".")]
        path: PathBuf,

        #[arg(long, value_enum, default_value_t = OutputFormat::Plain)]
        format: OutputFormat,
    },

    /// Import Codex, Claude Code, Cursor, or generic transcript files as raw capture events.
    ImportTranscripts {
        #[arg(long)]
        project: Option<String>,

        /// Agent transcript format: codex, claude-code, cursor, or generic.
        #[arg(long)]
        agent: String,

        /// Transcript file or directory. Defaults to the known history location for the agent.
        #[arg(long)]
        input: Option<PathBuf>,

        #[arg(long, default_value = "")]
        scope: String,

        #[arg(long, default_value_t = 200)]
        limit: usize,

        /// Immediately summarize imported transcript events into pending review candidates.
        #[arg(long)]
        summarize: bool,

        #[arg(long, default_value = ".")]
        path: PathBuf,

        #[arg(long, value_enum, default_value_t = OutputFormat::Plain)]
        format: OutputFormat,
    },

    /// Print a local zsh hook that records terminal command metadata.
    ShellHook {
        #[arg(long)]
        project: Option<String>,

        #[arg(long, default_value = "")]
        scope: String,

        #[arg(long, default_value = ".")]
        path: PathBuf,

        /// Command used by the hook to call Grafiki.
        #[arg(long, default_value = "grafiki")]
        command: String,
    },

    /// Record one terminal command execution.
    TerminalCommand {
        #[arg(long)]
        project: Option<String>,

        #[arg(long = "cmd", alias = "command")]
        command: String,

        #[arg(long)]
        cwd: Option<PathBuf>,

        #[arg(long)]
        exit_code: Option<i32>,

        #[arg(long)]
        duration_ms: Option<u64>,

        #[arg(long)]
        shell: Option<String>,

        #[arg(long, default_value = "")]
        scope: String,

        #[arg(long, default_value = ".")]
        path: PathBuf,

        #[arg(long, value_enum, default_value_t = OutputFormat::Plain)]
        format: OutputFormat,
    },

    /// Poll recent workspace file changes into raw capture events.
    WatchFiles {
        #[arg(long)]
        project: Option<String>,

        #[arg(long, default_value = "")]
        scope: String,

        #[arg(long, default_value = ".")]
        path: PathBuf,

        #[arg(long, default_value_t = 300)]
        since_seconds: u64,

        #[arg(long, default_value_t = 0)]
        duration_seconds: u64,

        #[arg(long, default_value_t = 1000)]
        interval_ms: u64,

        #[arg(long, default_value_t = 100)]
        limit: usize,

        #[arg(long)]
        summarize: bool,

        #[arg(long, value_enum, default_value_t = OutputFormat::Plain)]
        format: OutputFormat,
    },

    /// Record a raw git working-tree snapshot into capture.
    GitSummary {
        #[arg(long)]
        project: Option<String>,

        #[arg(long, default_value = "")]
        scope: String,

        #[arg(long, default_value = ".")]
        path: PathBuf,

        #[arg(long, default_value = "git")]
        source: String,

        #[arg(long, default_value_t = 80)]
        limit: usize,

        #[arg(long)]
        summarize: bool,

        #[arg(long, value_enum, default_value_t = OutputFormat::Plain)]
        format: OutputFormat,
    },

    /// Show capture status and recent captured events.
    Status {
        #[arg(long)]
        project: Option<String>,

        #[arg(long, default_value = "")]
        scope: String,

        #[arg(long, default_value = ".")]
        path: PathBuf,

        #[arg(long, value_enum, default_value_t = OutputFormat::Plain)]
        format: OutputFormat,
    },

    /// List raw captured events.
    Events {
        #[arg(long)]
        project: Option<String>,

        #[arg(long)]
        capture: Option<String>,

        #[arg(long = "type")]
        source_type: Option<String>,

        #[arg(long, default_value = "")]
        scope: String,

        #[arg(long, default_value_t = 50)]
        limit: usize,

        #[arg(long, default_value = ".")]
        path: PathBuf,

        #[arg(long, value_enum, default_value_t = OutputFormat::Plain)]
        format: OutputFormat,
    },

    /// Summarize captured events into pending memory candidates.
    Summarize {
        #[arg(long)]
        project: Option<String>,

        #[arg(long)]
        capture: Option<String>,

        #[arg(long, default_value = "")]
        scope: String,

        #[arg(long, default_value_t = 80)]
        limit: usize,

        #[arg(long, default_value = ".")]
        path: PathBuf,

        #[arg(long, value_enum, default_value_t = OutputFormat::Plain)]
        format: OutputFormat,
    },
}

#[derive(Debug, Subcommand)]
enum CaptureConfigCommand {
    /// Show workspace capture consent settings.
    Show {
        #[arg(long)]
        project: Option<String>,

        #[arg(long, default_value = ".")]
        path: PathBuf,

        #[arg(long, value_enum, default_value_t = OutputFormat::Plain)]
        format: OutputFormat,
    },

    /// Update workspace capture consent settings.
    Set {
        #[arg(long)]
        project: Option<String>,

        #[arg(long, default_value = ".")]
        path: PathBuf,

        #[arg(long)]
        git: Option<bool>,

        #[arg(long)]
        transcripts: Option<bool>,

        #[arg(long)]
        terminal: Option<bool>,

        #[arg(long)]
        files: Option<bool>,

        #[arg(long)]
        ide: Option<bool>,

        #[arg(long)]
        screen: Option<bool>,

        #[arg(long)]
        browser: Option<bool>,

        #[arg(long)]
        audio: Option<bool>,

        #[arg(long)]
        system: Option<bool>,

        #[arg(long = "add-blocked-path")]
        add_blocked_paths: Vec<String>,

        #[arg(long = "remove-blocked-path")]
        remove_blocked_paths: Vec<String>,

        #[arg(long = "add-blocked-app")]
        add_blocked_apps: Vec<String>,

        #[arg(long = "remove-blocked-app")]
        remove_blocked_apps: Vec<String>,

        #[arg(long)]
        redaction_profile: Option<String>,

        /// off, digest, or full. The shell hook still defaults to off.
        #[arg(long)]
        terminal_output: Option<String>,

        /// off, manual, or allowlist.
        #[arg(long)]
        screen_policy: Option<String>,

        /// off or allowlist.
        #[arg(long)]
        browser_policy: Option<String>,

        #[arg(long, value_enum, default_value_t = OutputFormat::Plain)]
        format: OutputFormat,
    },
}

#[derive(Debug, Subcommand)]
enum EmbeddingsCommand {
    /// Show embedding queue counts and indexed model metadata.
    Status {
        /// Explicit project name. Defaults to .grafiki detection, then directory name.
        #[arg(long)]
        project: Option<String>,

        /// Slash-delimited scope.
        #[arg(long, default_value = "")]
        scope: String,

        /// Directory used for project detection.
        #[arg(long, default_value = ".")]
        path: PathBuf,

        #[arg(long, value_enum, default_value_t = OutputFormat::Plain)]
        format: OutputFormat,
    },

    /// Process pending embedding jobs.
    Process {
        /// Explicit project name. Defaults to .grafiki detection, then directory name.
        #[arg(long)]
        project: Option<String>,

        /// Slash-delimited scope.
        #[arg(long, default_value = "")]
        scope: String,

        /// Maximum jobs to process.
        #[arg(long, default_value_t = 100)]
        limit: usize,

        /// Directory used for project detection.
        #[arg(long, default_value = ".")]
        path: PathBuf,

        #[arg(long, value_enum, default_value_t = OutputFormat::Plain)]
        format: OutputFormat,
    },

    /// Enqueue current records and process embedding jobs.
    Rebuild {
        /// Explicit project name. Defaults to .grafiki detection, then directory name.
        #[arg(long)]
        project: Option<String>,

        /// Slash-delimited scope.
        #[arg(long, default_value = "")]
        scope: String,

        /// Maximum jobs to process.
        #[arg(long, default_value_t = 100)]
        limit: usize,

        /// Directory used for project detection.
        #[arg(long, default_value = ".")]
        path: PathBuf,

        #[arg(long, value_enum, default_value_t = OutputFormat::Plain)]
        format: OutputFormat,
    },
}

#[derive(Debug, Subcommand)]
enum CandidateCommand {
    /// Propose untrusted memory for later review.
    Propose {
        /// Proposed trusted record type: entity, observation, decision, context, or state.
        #[arg(long = "type")]
        record_type: String,

        /// Connector, agent, or import source type.
        #[arg(long, default_value = "agent")]
        source_type: String,

        /// Optional source identifier, such as a URL, issue id, or transcript id.
        #[arg(long)]
        source: Option<String>,

        /// Candidate payload as a JSON object.
        #[arg(long)]
        payload: Option<String>,

        /// Candidate payload JSON file.
        #[arg(long)]
        file: Option<PathBuf>,

        /// Slash-delimited scope.
        #[arg(long, default_value = "")]
        scope: String,

        /// Source confidence from 0.0 to 1.0.
        #[arg(long, default_value_t = 0.5)]
        confidence: f64,

        /// Optional extraction rationale.
        #[arg(long)]
        rationale: Option<String>,

        /// Explicit project name. Defaults to .grafiki detection, then directory name.
        #[arg(long)]
        project: Option<String>,

        /// Directory used for project detection.
        #[arg(long, default_value = ".")]
        path: PathBuf,

        #[arg(long, value_enum, default_value_t = OutputFormat::Plain)]
        format: OutputFormat,
    },

    /// List candidate memory awaiting review.
    List {
        /// Candidate status: pending, approved, rejected, or all.
        #[arg(long, default_value = "pending")]
        status: String,

        /// Slash-delimited scope.
        #[arg(long, default_value = "")]
        scope: String,

        /// Maximum candidates.
        #[arg(long, default_value_t = 20)]
        limit: usize,

        /// Explicit project name. Defaults to .grafiki detection, then directory name.
        #[arg(long)]
        project: Option<String>,

        /// Directory used for project detection.
        #[arg(long, default_value = ".")]
        path: PathBuf,

        #[arg(long, value_enum, default_value_t = OutputFormat::Plain)]
        format: OutputFormat,
    },

    /// Edit a pending candidate before approving it.
    Edit {
        id: String,

        /// Change proposed trusted record type.
        #[arg(long = "type")]
        record_type: Option<String>,

        /// Replacement candidate payload as a JSON object.
        #[arg(long)]
        payload: Option<String>,

        /// Replacement candidate payload JSON file.
        #[arg(long)]
        file: Option<PathBuf>,

        /// Replacement slash-delimited scope.
        #[arg(long)]
        scope: Option<String>,

        /// Replacement source confidence from 0.0 to 1.0.
        #[arg(long)]
        confidence: Option<f64>,

        /// Replacement extraction rationale.
        #[arg(long)]
        rationale: Option<String>,

        #[arg(long)]
        project: Option<String>,

        #[arg(long, default_value = ".")]
        path: PathBuf,

        #[arg(long, value_enum, default_value_t = OutputFormat::Plain)]
        format: OutputFormat,
    },

    /// Approve one candidate into trusted Grafiki memory.
    Approve {
        id: String,

        #[arg(long)]
        project: Option<String>,

        #[arg(long, default_value = ".")]
        path: PathBuf,

        #[arg(long, value_enum, default_value_t = OutputFormat::Plain)]
        format: OutputFormat,
    },

    /// Approve or reject several candidates in one review action.
    Bulk {
        /// Review action: approve or reject.
        action: String,

        /// Candidate ids to review.
        ids: Vec<String>,

        #[arg(long)]
        rationale: Option<String>,

        #[arg(long)]
        project: Option<String>,

        #[arg(long, default_value = ".")]
        path: PathBuf,

        #[arg(long, value_enum, default_value_t = OutputFormat::Plain)]
        format: OutputFormat,
    },

    /// Reject one candidate without trusting it.
    Reject {
        id: String,

        #[arg(long)]
        rationale: Option<String>,

        #[arg(long)]
        project: Option<String>,

        #[arg(long, default_value = ".")]
        path: PathBuf,

        #[arg(long, value_enum, default_value_t = OutputFormat::Plain)]
        format: OutputFormat,
    },
}

#[derive(Debug, Subcommand)]
enum ContextCommand {
    /// Add a context document.
    Add {
        key: String,

        #[arg(long)]
        project: Option<String>,

        #[arg(long)]
        title: String,

        #[arg(long)]
        category: String,

        #[arg(long, default_value = "")]
        scope: String,

        #[arg(long)]
        content: Option<String>,

        #[arg(long)]
        file: Option<PathBuf>,

        #[arg(long, default_value = ".")]
        path: PathBuf,

        #[arg(long, value_enum, default_value_t = OutputFormat::Plain)]
        format: OutputFormat,
    },

    /// Show a context document.
    Show {
        key: String,

        #[arg(long)]
        project: Option<String>,

        #[arg(long, default_value = ".")]
        path: PathBuf,

        #[arg(long, value_enum, default_value_t = OutputFormat::Plain)]
        format: OutputFormat,
    },

    /// List context documents.
    List {
        #[arg(long)]
        project: Option<String>,

        #[arg(long)]
        category: Option<String>,

        #[arg(long, default_value = "")]
        scope: String,

        #[arg(long, default_value = ".")]
        path: PathBuf,

        #[arg(long, value_enum, default_value_t = OutputFormat::Plain)]
        format: OutputFormat,
    },

    /// Update a context document.
    Update {
        key: String,

        #[arg(long)]
        project: Option<String>,

        #[arg(long)]
        title: Option<String>,

        #[arg(long)]
        category: Option<String>,

        #[arg(long)]
        scope: Option<String>,

        #[arg(long)]
        content: Option<String>,

        #[arg(long)]
        file: Option<PathBuf>,

        #[arg(long, default_value = ".")]
        path: PathBuf,

        #[arg(long, value_enum, default_value_t = OutputFormat::Plain)]
        format: OutputFormat,
    },

    /// Delete a context document.
    Delete {
        key: String,

        #[arg(long)]
        project: Option<String>,

        #[arg(long, default_value = ".")]
        path: PathBuf,

        #[arg(long, value_enum, default_value_t = OutputFormat::Plain)]
        format: OutputFormat,
    },
}

#[derive(Debug, Subcommand)]
enum StateCommand {
    /// Create or update a state item.
    Set {
        key: String,

        #[arg(long)]
        project: Option<String>,

        #[arg(long)]
        title: String,

        #[arg(long, default_value = "in-progress")]
        status: String,

        #[arg(long)]
        owner: Option<String>,

        #[arg(long)]
        details: Option<String>,

        #[arg(long)]
        blockers: Option<String>,

        #[arg(long = "depends-on")]
        depends_on: Option<String>,

        #[arg(long, default_value = "")]
        scope: String,

        #[arg(long, default_value = "medium")]
        priority: String,

        #[arg(long, default_value = ".")]
        path: PathBuf,

        #[arg(long, value_enum, default_value_t = OutputFormat::Plain)]
        format: OutputFormat,
    },

    /// List state items.
    List {
        #[arg(long)]
        project: Option<String>,

        #[arg(long)]
        status: Option<String>,

        #[arg(long, default_value = "")]
        scope: String,

        #[arg(long, default_value = ".")]
        path: PathBuf,

        #[arg(long, value_enum, default_value_t = OutputFormat::Plain)]
        format: OutputFormat,
    },

    /// Delete a state item.
    Delete {
        key: String,

        #[arg(long)]
        project: Option<String>,

        #[arg(long, default_value = ".")]
        path: PathBuf,

        #[arg(long, value_enum, default_value_t = OutputFormat::Plain)]
        format: OutputFormat,
    },
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Command::Init {
            project_name,
            path,
            format,
        } => {
            let report = init_project(InitOptions {
                project_name,
                project_dir: path,
                grafiki_home: None,
            })?;

            match format {
                OutputFormat::Plain => {
                    println!("Initialized Grafiki project: {}", report.project);
                    println!("Project directory: {}", report.project_dir.display());
                    println!("Marker: {}", report.marker_path.display());
                    println!("Capture config: {}", report.capture_config_path.display());
                    println!("Database: {}", report.db_path.display());
                    println!("Imported sources: {}", report.imported_files.len());
                    println!("Pending candidates: {}", report.proposed_candidates);
                    println!("Agent setup: {}", report.next_agent_setup);
                }
                OutputFormat::Md => {
                    println!("# Grafiki Project Initialized\n");
                    println!("- Project: {}", report.project);
                    println!("- Project directory: {}", report.project_dir.display());
                    println!("- Marker: {}", report.marker_path.display());
                    println!("- Capture config: {}", report.capture_config_path.display());
                    println!("- Database: {}", report.db_path.display());
                    println!("- Imported sources: {}", report.imported_files.len());
                    println!("- Pending candidates: {}", report.proposed_candidates);
                    println!("- Agent setup: `{}`", report.next_agent_setup);
                }
                OutputFormat::Json => {
                    println!("{}", serde_json::to_string_pretty(&report)?);
                }
            }
        }
        Command::Start {
            project,
            session_type,
            goal,
            scope,
            path,
            format,
        } => {
            let report = start_session(StartSessionOptions {
                project_name: project,
                start_dir: path,
                grafiki_home: None,
                session_type,
                goal,
                scope,
            })?;

            match format {
                OutputFormat::Plain => {
                    println!("Started Grafiki session: {}", report.session_id);
                    println!("Project: {}", report.project);
                    println!("Scope: {}", display_scope(&report.scope));
                    println!("Database: {}", report.db_path.display());
                    println!();
                    println!("{}", report.briefing);
                }
                OutputFormat::Md => {
                    println!("{}", report.briefing);
                }
                OutputFormat::Json => {
                    println!("{}", serde_json::to_string_pretty(&report)?);
                }
            }
        }
        Command::End {
            project,
            session,
            status,
            summary,
            accomplishments,
            remaining,
            files,
            path,
            format,
        } => {
            let report = end_session(EndSessionOptions {
                project_name: project,
                start_dir: path,
                grafiki_home: None,
                session_id: session,
                status,
                summary,
                accomplishments: split_csv(accomplishments),
                remaining: split_csv(remaining),
                files_changed: split_csv(files),
            })?;

            match format {
                OutputFormat::Plain => {
                    println!("Ended Grafiki session: {}", report.session_id);
                    println!("Project: {}", report.project);
                    println!("Status: {}", report.status);
                    if let Some(summary) = report.summary {
                        println!("Summary: {summary}");
                    }
                }
                OutputFormat::Md => {
                    println!("# Grafiki Session Ended\n");
                    println!("- Session: {}", report.session_id);
                    println!("- Project: {}", report.project);
                    println!("- Status: {}", report.status);
                    if let Some(summary) = report.summary {
                        println!("- Summary: {summary}");
                    }
                }
                OutputFormat::Json => {
                    println!("{}", serde_json::to_string_pretty(&report)?);
                }
            }
        }
        Command::Handoff {
            project,
            session,
            path,
            format,
        } => {
            let report = handoff_session(HandoffOptions {
                project_name: project,
                start_dir: path,
                grafiki_home: None,
                session_id: session,
            })?;

            match format {
                OutputFormat::Plain => {
                    println!("Handed off Grafiki session: {}", report.parent_session_id);
                    println!("Child session: {}", report.child_session_id);
                    println!("Project: {}", report.project);
                    println!("Scope: {}", display_scope(&report.scope));
                    println!();
                    println!("{}", report.handoff_context);
                }
                OutputFormat::Md => {
                    println!("{}", report.handoff_context);
                }
                OutputFormat::Json => {
                    println!("{}", serde_json::to_string_pretty(&report)?);
                }
            }
        }
        Command::Decide {
            title,
            project,
            reasoning,
            alternatives,
            tags,
            scope,
            supersedes,
            path,
            format,
        } => {
            let report = log_decision(LogDecisionOptions {
                project_name: project,
                start_dir: path,
                grafiki_home: None,
                title,
                reasoning,
                alternatives: split_csv(alternatives),
                tags: split_csv(tags),
                scope,
                supersedes,
            })?;

            match format {
                OutputFormat::Plain => {
                    println!("Logged decision: {}", report.decision_id);
                    println!("Title: {}", report.title);
                    println!("Scope: {}", display_scope(&report.scope));
                }
                OutputFormat::Md => {
                    println!("# Grafiki Decision\n");
                    println!("- ID: {}", report.decision_id);
                    println!("- Title: {}", report.title);
                    println!("- Scope: {}", display_scope(&report.scope));
                }
                OutputFormat::Json => {
                    println!("{}", serde_json::to_string_pretty(&report)?);
                }
            }
        }
        Command::Save {
            entity_name,
            project,
            entity_type,
            observe,
            category,
            scope,
            relate,
            path,
            format,
        } => {
            let report = save_entity(SaveEntityOptions {
                project_name: project,
                start_dir: path,
                grafiki_home: None,
                name: entity_name,
                entity_type,
                observe,
                category,
                scope,
                relate,
            })?;

            match format {
                OutputFormat::Plain => {
                    println!("Saved entity: {}", report.entity_id);
                    println!("Project: {}", report.project);
                    println!("Created: {}", report.created);
                    if let Some(observation_id) = report.observation_id {
                        println!("Observation: {observation_id}");
                    }
                    if let Some(relation_id) = report.relation_id {
                        println!("Relation: {relation_id}");
                    }
                }
                OutputFormat::Md => {
                    println!("# Grafiki Entity Saved\n");
                    println!("- Entity: {}", report.entity_id);
                    println!("- Project: {}", report.project);
                    println!("- Created: {}", report.created);
                    if let Some(observation_id) = report.observation_id {
                        println!("- Observation: {observation_id}");
                    }
                    if let Some(relation_id) = report.relation_id {
                        println!("- Relation: {relation_id}");
                    }
                }
                OutputFormat::Json => {
                    println!("{}", serde_json::to_string_pretty(&report)?);
                }
            }
        }
        Command::Search {
            query,
            project,
            record_type,
            mode,
            scope,
            limit,
            path,
            format,
        } => {
            let report = search_memory(SearchMemoryOptions {
                project_name: project,
                start_dir: path,
                grafiki_home: None,
                query,
                record_type,
                mode: mode.into(),
                scope,
                limit,
            })?;

            match format {
                OutputFormat::Plain => {
                    if let Some(fallback) = &report.fallback {
                        println!("Note: {fallback}");
                    }
                    if report.results.is_empty() {
                        println!("No results.");
                    } else {
                        for result in &report.results {
                            println!("[{}] {} {}", result.record_type, result.id, result.title);
                            println!("  Scope: {}", display_scope(&result.scope));
                            println!("  {}", result.snippet);
                        }
                    }
                }
                OutputFormat::Md => {
                    println!("# Grafiki Search Results\n");
                    if let Some(fallback) = &report.fallback {
                        println!("> {fallback}\n");
                    }
                    for result in &report.results {
                        println!("## {}: {}", result.record_type, result.title);
                        println!("- ID: {}", result.id);
                        println!("- Scope: {}", display_scope(&result.scope));
                        println!("- Snippet: {}", result.snippet);
                        println!();
                    }
                }
                OutputFormat::Json => {
                    println!("{}", serde_json::to_string_pretty(&report)?);
                }
            }
        }
        Command::Ask {
            question,
            project,
            scope,
            limit,
            agent,
            path,
            format,
        } => {
            let briefing = ask_memory(AskMemoryOptions {
                project_name: project,
                start_dir: path,
                grafiki_home: None,
                question,
                scope,
                limit,
                agent: Some(agent),
            })?;

            match format {
                OutputFormat::Plain => print_ask_plain(&briefing),
                OutputFormat::Md => print_ask_md(&briefing),
                OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&briefing)?),
            }
        }
        Command::AgentActivity {
            project,
            scope,
            limit,
            path,
            format,
        } => {
            let queries = list_agent_queries(ListAgentQueriesOptions {
                project_name: project,
                start_dir: path,
                grafiki_home: None,
                scope,
                limit,
            })?;
            match format {
                OutputFormat::Plain => {
                    if queries.is_empty() {
                        println!("No agent queries recorded.");
                    }
                    for query in &queries {
                        println!(
                            "[{}] {} asked: {}",
                            query.created_at, query.agent, query.question
                        );
                        println!("  Scope: {}", display_scope(&query.scope));
                        println!("  Returned: {}", query.returned_ids.join(", "));
                        if let Some(fallback) = &query.fallback {
                            println!("  Note: {fallback}");
                        }
                    }
                }
                OutputFormat::Md => {
                    println!("# Grafiki Agent Activity\n");
                    for query in &queries {
                        println!("## {} - {}", query.created_at, query.agent);
                        println!("- Scope: {}", display_scope(&query.scope));
                        println!("- Question: {}", query.question);
                        println!("- Returned: {}", query.returned_ids.join(", "));
                        if let Some(fallback) = &query.fallback {
                            println!("- Note: {fallback}");
                        }
                        println!();
                    }
                }
                OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&queries)?),
            }
        }
        Command::AutoCapture {
            project,
            scope,
            path,
            source,
            limit,
            format,
        } => {
            let report = auto_capture(project, path, scope, source, limit)?;
            print_auto_capture(&report, format)?;
        }
        Command::Capture { command } => match command {
            CaptureCommand::Config { command } => match command {
                CaptureConfigCommand::Show {
                    project,
                    path,
                    format,
                } => {
                    let report = load_capture_config(CaptureConfigOptions {
                        project_name: project,
                        start_dir: path,
                        grafiki_home: None,
                    })?;
                    print_capture_config_report(&report, format)?;
                }
                CaptureConfigCommand::Set {
                    project,
                    path,
                    git,
                    transcripts,
                    terminal,
                    files,
                    ide,
                    screen,
                    browser,
                    audio,
                    system,
                    add_blocked_paths,
                    remove_blocked_paths,
                    add_blocked_apps,
                    remove_blocked_apps,
                    redaction_profile,
                    terminal_output,
                    screen_policy,
                    browser_policy,
                    format,
                } => {
                    let report = update_capture_config(UpdateCaptureConfigOptions {
                        project_name: project,
                        start_dir: path,
                        grafiki_home: None,
                        sources: CaptureSourceUpdates {
                            git,
                            transcripts,
                            terminal,
                            files,
                            ide,
                            screen,
                            browser,
                            audio,
                            system,
                        },
                        add_blocked_paths,
                        remove_blocked_paths,
                        add_blocked_apps,
                        remove_blocked_apps,
                        redaction_profile,
                        terminal_output,
                        screen_policy,
                        browser_policy,
                    })?;
                    print_capture_config_report(&report, format)?;
                }
            },
            CaptureCommand::Start {
                project,
                scope,
                path,
                source_app,
                consent_profile,
                redaction_profile,
                format,
            } => {
                let report = start_capture_session(StartCaptureOptions {
                    project_name: project,
                    start_dir: path,
                    grafiki_home: None,
                    scope,
                    source_app,
                    consent_profile,
                    redaction_profile,
                })?;
                print_capture_session_report(&report, format)?;
            }
            CaptureCommand::Stop {
                id,
                project,
                path,
                format,
            } => {
                let report = stop_capture_session(StopCaptureOptions {
                    project_name: project,
                    start_dir: path,
                    grafiki_home: None,
                    capture_id: id,
                })?;
                print_capture_session_report(&report, format)?;
            }
            CaptureCommand::Ingest {
                project,
                capture,
                source_type,
                source,
                title,
                text,
                file,
                payload,
                metadata,
                privacy,
                redacted,
                captured_at,
                scope,
                path,
                format,
            } => {
                let text = match (text, file) {
                    (Some(text), _) => Some(text),
                    (None, Some(file)) => Some(fs::read_to_string(file)?),
                    (None, None) => None,
                };
                let report = ingest_capture_event(IngestCaptureEventOptions {
                    project_name: project,
                    start_dir: path,
                    grafiki_home: None,
                    capture_id: capture,
                    scope,
                    source_type,
                    source,
                    title,
                    text,
                    payload: parse_optional_json_value(payload)?,
                    metadata: parse_optional_json_value(metadata)?,
                    privacy_level: Some(privacy),
                    redacted,
                    captured_at,
                })?;
                print_capture_event_report(&report, format)?;
            }
            CaptureCommand::ImportTranscripts {
                project,
                agent,
                input,
                scope,
                limit,
                summarize,
                path,
                format,
            } => {
                ensure_capture_source_enabled(project.clone(), &path, "transcripts")?;
                let report = import_agent_transcripts(ImportAgentTranscriptsOptions {
                    project_name: project,
                    start_dir: path,
                    grafiki_home: None,
                    agent,
                    input,
                    scope,
                    limit,
                    summarize,
                })?;
                print_agent_transcript_import_report(&report, format)?;
            }
            CaptureCommand::ShellHook {
                project,
                scope,
                path,
                command,
            } => {
                print_shell_hook(project, scope, path, command)?;
            }
            CaptureCommand::TerminalCommand {
                project,
                command,
                cwd,
                exit_code,
                duration_ms,
                shell,
                scope,
                path,
                format,
            } => {
                let report = capture_terminal_command(TerminalCommandCaptureOptions {
                    project_name: project,
                    start_dir: path,
                    scope,
                    command,
                    cwd,
                    exit_code,
                    duration_ms,
                    shell,
                })?;
                print_capture_event_report(&report, format)?;
            }
            CaptureCommand::WatchFiles {
                project,
                scope,
                path,
                since_seconds,
                duration_seconds,
                interval_ms,
                limit,
                summarize,
                format,
            } => {
                let report = watch_files_capture(FileWatchCaptureOptions {
                    project_name: project,
                    start_dir: path,
                    scope,
                    since_seconds,
                    duration_seconds,
                    interval_ms,
                    limit,
                    summarize,
                })?;
                print_file_watch_capture_report(&report, format)?;
            }
            CaptureCommand::GitSummary {
                project,
                scope,
                path,
                source,
                limit,
                summarize,
                format,
            } => {
                let report = capture_git_summary(GitSummaryCaptureOptions {
                    project_name: project,
                    start_dir: path,
                    scope,
                    source,
                    limit,
                    summarize,
                })?;
                print_git_summary_capture_report(&report, format)?;
            }
            CaptureCommand::Status {
                project,
                scope,
                path,
                format,
            } => {
                let report = get_capture_status(CaptureStatusOptions {
                    project_name: project,
                    start_dir: path,
                    grafiki_home: None,
                    scope,
                })?;
                print_capture_status(&report, format)?;
            }
            CaptureCommand::Events {
                project,
                capture,
                source_type,
                scope,
                limit,
                path,
                format,
            } => {
                let events = list_capture_events(ListCaptureEventsOptions {
                    project_name: project,
                    start_dir: path,
                    grafiki_home: None,
                    capture_id: capture,
                    source_type,
                    scope,
                    limit,
                })?;
                print_capture_events(&events, format)?;
            }
            CaptureCommand::Summarize {
                project,
                capture,
                scope,
                limit,
                path,
                format,
            } => {
                let report = propose_capture_candidates(ProposeCaptureCandidatesOptions {
                    project_name: project,
                    start_dir: path,
                    grafiki_home: None,
                    capture_id: capture,
                    scope,
                    limit,
                })?;
                print_capture_candidate_report(&report, format)?;
            }
        },
        Command::Graph {
            entity_id,
            project,
            depth,
            path,
            format,
        } => {
            let report = get_graph(GraphOptions {
                project_name: project,
                start_dir: path,
                grafiki_home: None,
                entity_id,
                depth,
            })?;
            match format {
                GraphOutputFormat::Plain => print_graph_plain(&report),
                GraphOutputFormat::Json => println!("{}", serde_json::to_string_pretty(&report)?),
                GraphOutputFormat::Dot => println!("{}", graph_to_dot(&report)),
            }
        }
        Command::Report {
            project,
            scope,
            path,
            output,
            format,
        } => {
            let report = generate_report(ProjectReportOptions {
                project_name: project,
                start_dir: path,
                grafiki_home: None,
                scope,
            })?;
            let rendered = match format {
                OutputFormat::Plain => project_report_to_plain(&report),
                OutputFormat::Md => project_report_to_markdown(&report),
                OutputFormat::Json => serde_json::to_string_pretty(&report)?,
            };
            write_or_print(output, &rendered)?;
        }
        Command::Analyze {
            project,
            scope,
            path,
            output,
            format,
        } => {
            let report = generate_report(ProjectReportOptions {
                project_name: project,
                start_dir: path,
                grafiki_home: None,
                scope,
            })?;
            let rendered = match format {
                OutputFormat::Plain => project_report_to_plain(&report),
                OutputFormat::Md => project_report_to_markdown(&report),
                OutputFormat::Json => serde_json::to_string_pretty(&report)?,
            };
            write_or_print(output, &rendered)?;
        }
        Command::Export {
            project,
            scope,
            path,
            output,
            format,
        } => {
            let bundle = export_memory(ExportOptions {
                project_name: project,
                start_dir: path,
                grafiki_home: None,
                scope,
            })?;
            match format {
                ExportOutputFormat::Wiki => {
                    let output_dir =
                        output.unwrap_or_else(|| PathBuf::from(format!("{}-wiki", bundle.project)));
                    export_bundle_to_wiki(&bundle, &output_dir)?;
                    println!("Wrote Grafiki wiki export: {}", output_dir.display());
                }
                ExportOutputFormat::Json
                | ExportOutputFormat::Md
                | ExportOutputFormat::Dot
                | ExportOutputFormat::Graphml
                | ExportOutputFormat::Html => {
                    let rendered = match format {
                        ExportOutputFormat::Json => serde_json::to_string_pretty(&bundle)?,
                        ExportOutputFormat::Md => export_bundle_to_markdown(&bundle),
                        ExportOutputFormat::Dot => export_bundle_to_dot(&bundle),
                        ExportOutputFormat::Graphml => export_bundle_to_graphml(&bundle),
                        ExportOutputFormat::Html => export_bundle_to_html(&bundle),
                        ExportOutputFormat::Wiki => unreachable!(),
                    };
                    write_or_print(output, &rendered)?;
                }
            }
        }
        Command::Import {
            input,
            project,
            path,
            format,
        } => {
            let content = fs::read_to_string(input)?;
            let bundle: grafiki_core::ExportBundle = serde_json::from_str(&content)?;
            let report = import_memory(ImportOptions {
                project_name: project,
                start_dir: path,
                grafiki_home: None,
                bundle,
            })?;
            match format {
                OutputFormat::Plain => print_import_report_plain(&report),
                OutputFormat::Md => print_import_report_md(&report),
                OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&report)?),
            }
        }
        Command::Embeddings { command } => match command {
            EmbeddingsCommand::Status {
                project,
                scope,
                path,
                format,
            } => {
                let report = get_embedding_status(EmbeddingStatusOptions {
                    project_name: project,
                    start_dir: path,
                    grafiki_home: None,
                    scope,
                })?;
                print_embedding_status(&report, format)?;
            }
            EmbeddingsCommand::Process {
                project,
                scope,
                limit,
                path,
                format,
            } => {
                let report = process_embedding_jobs(ProcessEmbeddingsOptions {
                    project_name: project,
                    start_dir: path,
                    grafiki_home: None,
                    scope,
                    limit,
                    rebuild: false,
                })?;
                print_process_embeddings_report(&report, format)?;
            }
            EmbeddingsCommand::Rebuild {
                project,
                scope,
                limit,
                path,
                format,
            } => {
                let report = process_embedding_jobs(ProcessEmbeddingsOptions {
                    project_name: project,
                    start_dir: path,
                    grafiki_home: None,
                    scope,
                    limit,
                    rebuild: true,
                })?;
                print_process_embeddings_report(&report, format)?;
            }
        },
        Command::Serve {
            project,
            path,
            host,
            port,
            allow_non_local,
            token,
        } => {
            serve_http(project, path, host, port, allow_non_local, token)?;
        }
        Command::Daemon { command } => match command {
            DaemonCommand::Start {
                project,
                path,
                host,
                port,
                allow_non_local,
                token,
                log,
                format,
            } => {
                let report = daemon_start(project, path, host, port, allow_non_local, token, log)?;
                print_daemon_start_report(&report, format)?;
            }
            DaemonCommand::Status {
                project,
                path,
                format,
            } => {
                let report = daemon_status(project, path)?;
                print_daemon_status_report(&report, format)?;
            }
            DaemonCommand::Stop {
                project,
                path,
                format,
            } => {
                let report = daemon_stop(project, path)?;
                print_daemon_stop_report(&report, format)?;
            }
        },
        Command::Mcp { project, path } => {
            run_mcp(project, path)?;
        }
        Command::Status {
            project,
            scope,
            path,
            format,
        } => {
            let report = get_status(StatusOptions {
                project_name: project,
                start_dir: path,
                grafiki_home: None,
                scope,
            })?;

            match format {
                OutputFormat::Plain => print_status_plain(&report),
                OutputFormat::Md => print_status_md(&report),
                OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&report)?),
            }
        }
        Command::State { command } => match command {
            StateCommand::Set {
                key,
                project,
                title,
                status,
                owner,
                details,
                blockers,
                depends_on,
                scope,
                priority,
                path,
                format,
            } => {
                let report = upsert_state(UpsertStateOptions {
                    project_name: project,
                    start_dir: path,
                    grafiki_home: None,
                    key,
                    title,
                    status,
                    owner,
                    details,
                    blockers: split_csv(blockers),
                    depends_on: split_csv(depends_on),
                    scope,
                    priority,
                })?;
                match format {
                    OutputFormat::Plain => {
                        println!("State updated: {}", report.key);
                        println!("Title: {}", report.title);
                        println!("Status: {}", report.status);
                        println!("Priority: {}", report.priority);
                        println!("Scope: {}", display_scope(&report.scope));
                    }
                    OutputFormat::Md => {
                        println!("# Grafiki State Updated\n");
                        println!("- Key: {}", report.key);
                        println!("- Title: {}", report.title);
                        println!("- Status: {}", report.status);
                        println!("- Priority: {}", report.priority);
                        println!("- Scope: {}", display_scope(&report.scope));
                    }
                    OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&report)?),
                }
            }
            StateCommand::List {
                project,
                status,
                scope,
                path,
                format,
            } => {
                let items = list_state(StateListOptions {
                    project_name: project,
                    start_dir: path,
                    grafiki_home: None,
                    status,
                    scope,
                })?;
                match format {
                    OutputFormat::Plain => {
                        if items.is_empty() {
                            println!("No state items.");
                        } else {
                            for item in items {
                                println!(
                                    "{} {} ({}, {}) [{}]",
                                    item.key,
                                    item.title,
                                    item.status,
                                    item.priority,
                                    display_scope(&item.scope)
                                );
                            }
                        }
                    }
                    OutputFormat::Md => {
                        println!("# Grafiki State\n");
                        if items.is_empty() {
                            println!("- None.");
                        } else {
                            for item in items {
                                println!(
                                    "- `{}`: {} ({}, {}) [{}]",
                                    item.key,
                                    item.title,
                                    item.status,
                                    item.priority,
                                    display_scope(&item.scope)
                                );
                            }
                        }
                    }
                    OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&items)?),
                }
            }
            StateCommand::Delete {
                key,
                project,
                path,
                format,
            } => {
                let report = delete_state(DeleteStateOptions {
                    project_name: project,
                    start_dir: path,
                    grafiki_home: None,
                    key,
                })?;
                match format {
                    OutputFormat::Plain => {
                        println!("State deleted: {}", report.key);
                        println!("Title: {}", report.title);
                        println!("Status: {}", report.status);
                        println!("Scope: {}", display_scope(&report.scope));
                    }
                    OutputFormat::Md => {
                        println!("# Grafiki State Deleted\n");
                        println!("- Key: {}", report.key);
                        println!("- Title: {}", report.title);
                        println!("- Status: {}", report.status);
                        println!("- Scope: {}", display_scope(&report.scope));
                    }
                    OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&report)?),
                }
            }
        },
        Command::Candidates { command } => match command {
            CandidateCommand::Propose {
                record_type,
                source_type,
                source,
                payload,
                file,
                scope,
                confidence,
                rationale,
                project,
                path,
                format,
            } => {
                let payload = read_candidate_payload(payload, file)?;
                let report = propose_candidate(ProposeCandidateOptions {
                    project_name: project,
                    start_dir: path,
                    grafiki_home: None,
                    source_type,
                    source,
                    record_type,
                    payload,
                    scope,
                    confidence,
                    rationale,
                    evidence: Vec::new(),
                })?;
                print_candidate_mutation_report(&report, format)?;
            }
            CandidateCommand::List {
                status,
                scope,
                limit,
                project,
                path,
                format,
            } => {
                let candidates = list_candidates(ListCandidatesOptions {
                    project_name: project,
                    start_dir: path,
                    grafiki_home: None,
                    status: Some(status),
                    scope,
                    limit,
                })?;
                print_candidates(&candidates, format)?;
            }
            CandidateCommand::Edit {
                id,
                record_type,
                payload,
                file,
                scope,
                confidence,
                rationale,
                project,
                path,
                format,
            } => {
                let payload = read_optional_candidate_payload(payload, file)?;
                let report = edit_candidate(EditCandidateOptions {
                    project_name: project,
                    start_dir: path,
                    grafiki_home: None,
                    id,
                    record_type,
                    payload,
                    scope,
                    confidence,
                    rationale,
                })?;
                print_candidate_mutation_report(&report, format)?;
            }
            CandidateCommand::Approve {
                id,
                project,
                path,
                format,
            } => {
                let report = approve_candidate(ApproveCandidateOptions {
                    project_name: project,
                    start_dir: path,
                    grafiki_home: None,
                    id,
                })?;
                print_candidate_mutation_report(&report, format)?;
            }
            CandidateCommand::Bulk {
                action,
                ids,
                rationale,
                project,
                path,
                format,
            } => {
                let report = bulk_review_candidates(BulkCandidateReviewOptions {
                    project_name: project,
                    start_dir: path,
                    grafiki_home: None,
                    action,
                    ids,
                    rationale,
                })?;
                print_bulk_candidate_review_report(&report, format)?;
            }
            CandidateCommand::Reject {
                id,
                rationale,
                project,
                path,
                format,
            } => {
                let report = reject_candidate(RejectCandidateOptions {
                    project_name: project,
                    start_dir: path,
                    grafiki_home: None,
                    id,
                    rationale,
                })?;
                print_candidate_mutation_report(&report, format)?;
            }
        },
        Command::Events {
            project,
            scope,
            since,
            limit,
            path,
            format,
        } => {
            let report = list_events(EventListOptions {
                project_name: project,
                start_dir: path,
                grafiki_home: None,
                scope,
                since,
                limit,
            })?;
            match format {
                OutputFormat::Plain => {
                    if report.events.is_empty() {
                        println!("No events.");
                    } else {
                        for event in report.events {
                            println!("{} {} {}", event.id, event.event_type, event.summary);
                            println!("  Scope: {}", display_scope(&event.scope));
                        }
                    }
                }
                OutputFormat::Md => {
                    println!("# Grafiki Events\n");
                    for event in report.events {
                        println!(
                            "- `{}` {}: {} [{}]",
                            event.id,
                            event.event_type,
                            event.summary,
                            display_scope(&event.scope)
                        );
                    }
                }
                OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&report)?),
            }
        }
        Command::Log {
            project,
            scope,
            session_type,
            limit,
            path,
            format,
        } => {
            let report = list_sessions(SessionLogOptions {
                project_name: project,
                start_dir: path,
                grafiki_home: None,
                scope,
                session_type,
                limit,
            })?;
            match format {
                OutputFormat::Plain => {
                    if report.sessions.is_empty() {
                        println!("No sessions.");
                    } else {
                        for session in report.sessions {
                            println!(
                                "{} {} ({})",
                                session.id, session.session_type, session.status
                            );
                            println!("  Scope: {}", display_scope(&session.scope));
                            if let Some(goal) = session.goal {
                                println!("  Goal: {goal}");
                            }
                            if let Some(summary) = session.summary {
                                println!("  Summary: {summary}");
                            }
                        }
                    }
                }
                OutputFormat::Md => {
                    println!("# Grafiki Session Log\n");
                    for session in report.sessions {
                        println!(
                            "- `{}` {} ({}) [{}]",
                            session.id,
                            session.session_type,
                            session.status,
                            display_scope(&session.scope)
                        );
                    }
                }
                OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&report)?),
            }
        }
        Command::Context { command } => match command {
            ContextCommand::Add {
                key,
                project,
                title,
                category,
                scope,
                content,
                file,
                path,
                format,
            } => {
                let content = read_content(content, file)?;
                let report = add_context(AddContextOptions {
                    project_name: project,
                    start_dir: path,
                    grafiki_home: None,
                    key,
                    title,
                    category,
                    scope,
                    content,
                })?;
                print_context_report(&report, format, "Added")?;
            }
            ContextCommand::Show {
                key,
                project,
                path,
                format,
            } => {
                let document = get_context(GetContextOptions {
                    project_name: project,
                    start_dir: path,
                    grafiki_home: None,
                    key,
                })?;
                match format {
                    OutputFormat::Plain => {
                        println!("{} ({})", document.title, document.key);
                        println!("Category: {}", document.category);
                        println!("Scope: {}", display_scope(&document.scope));
                        println!("Version: {}", document.version);
                        println!();
                        println!("{}", document.content);
                    }
                    OutputFormat::Md => {
                        println!("# {}", document.title);
                        println!();
                        println!("- Key: {}", document.key);
                        println!("- Category: {}", document.category);
                        println!("- Scope: {}", display_scope(&document.scope));
                        println!("- Version: {}", document.version);
                        println!();
                        println!("{}", document.content);
                    }
                    OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&document)?),
                }
            }
            ContextCommand::List {
                project,
                category,
                scope,
                path,
                format,
            } => {
                let summaries = list_context(ContextListOptions {
                    project_name: project,
                    start_dir: path,
                    grafiki_home: None,
                    category,
                    scope,
                })?;
                match format {
                    OutputFormat::Plain => {
                        if summaries.is_empty() {
                            println!("No context documents.");
                        } else {
                            for item in summaries {
                                println!(
                                    "{} {} ({}, v{}) [{}]",
                                    item.key,
                                    item.title,
                                    item.category,
                                    item.version,
                                    display_scope(&item.scope)
                                );
                            }
                        }
                    }
                    OutputFormat::Md => {
                        println!("# Grafiki Context\n");
                        if summaries.is_empty() {
                            println!("- None.");
                        } else {
                            for item in summaries {
                                println!(
                                    "- `{}`: {} ({}, v{}) [{}]",
                                    item.key,
                                    item.title,
                                    item.category,
                                    item.version,
                                    display_scope(&item.scope)
                                );
                            }
                        }
                    }
                    OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&summaries)?),
                }
            }
            ContextCommand::Update {
                key,
                project,
                title,
                category,
                scope,
                content,
                file,
                path,
                format,
            } => {
                let content = read_optional_content(content, file)?;
                let report = update_context(UpdateContextOptions {
                    project_name: project,
                    start_dir: path,
                    grafiki_home: None,
                    key,
                    title,
                    category,
                    scope,
                    content,
                })?;
                print_context_report(&report, format, "Updated")?;
            }
            ContextCommand::Delete {
                key,
                project,
                path,
                format,
            } => {
                let report = delete_context(DeleteContextOptions {
                    project_name: project,
                    start_dir: path,
                    grafiki_home: None,
                    key,
                })?;
                print_context_report(&report, format, "Deleted")?;
            }
        },
        Command::ScopeChain { scope, format } => {
            let chain = Scope::new(scope)?.chain().into_vec();

            match format {
                OutputFormat::Plain => {
                    for scope in chain {
                        println!("{scope}");
                    }
                }
                OutputFormat::Md => {
                    for scope in chain {
                        println!("- {}", display_scope(&scope));
                    }
                }
                OutputFormat::Json => {
                    println!("{}", serde_json::to_string_pretty(&chain)?);
                }
            }
        }
    }

    Ok(())
}

fn display_scope(scope: &str) -> &str {
    if scope.is_empty() {
        "global"
    } else {
        scope
    }
}

fn split_csv(value: Option<String>) -> Vec<String> {
    value
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn read_candidate_payload(
    payload: Option<String>,
    file: Option<PathBuf>,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let raw = match (payload, file) {
        (Some(payload), None) => payload,
        (None, Some(file)) => fs::read_to_string(file)?,
        (Some(_), Some(_)) => return Err("Pass either --payload or --file, not both.".into()),
        (None, None) => {
            return Err("Candidate payload is required. Pass --payload or --file.".into())
        }
    };
    let value: serde_json::Value = serde_json::from_str(&raw)?;
    if !value.is_object() {
        return Err("Candidate payload must be a JSON object.".into());
    }
    Ok(value)
}

fn read_optional_candidate_payload(
    payload: Option<String>,
    file: Option<PathBuf>,
) -> Result<Option<serde_json::Value>, Box<dyn std::error::Error>> {
    if payload.is_none() && file.is_none() {
        return Ok(None);
    }
    read_candidate_payload(payload, file).map(Some)
}

fn parse_optional_json_value(
    raw: Option<String>,
) -> Result<Option<serde_json::Value>, Box<dyn std::error::Error>> {
    raw.map(|raw| serde_json::from_str(&raw))
        .transpose()
        .map_err(Into::into)
}

fn print_candidate_mutation_report(
    report: &grafiki_core::CandidateMutationReport,
    format: OutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    match format {
        OutputFormat::Plain => {
            println!("{}", report.message);
            print_candidate_plain(&report.candidate);
        }
        OutputFormat::Md => {
            println!("# {}\n", report.message);
            print_candidate_md(&report.candidate);
        }
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(report)?),
    }
    Ok(())
}

fn print_bulk_candidate_review_report(
    report: &BulkCandidateReviewReport,
    format: OutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    match format {
        OutputFormat::Plain => {
            println!(
                "Candidate bulk {}: {}/{} succeeded, {} failed",
                report.action, report.succeeded, report.requested, report.failed
            );
            for result in &report.results {
                println!("- {} {}", result.candidate.id, result.candidate.status);
            }
            for error in &report.errors {
                println!("- {} failed: {}", error.id, error.error);
            }
        }
        OutputFormat::Md => {
            println!(
                "# Candidate bulk {}\n\n- Succeeded: {}\n- Failed: {}\n- Requested: {}\n",
                report.action, report.succeeded, report.failed, report.requested
            );
            for result in &report.results {
                println!("- `{}` {}", result.candidate.id, result.candidate.status);
            }
            for error in &report.errors {
                println!("- `{}` failed: {}", error.id, error.error);
            }
        }
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(report)?),
    }
    Ok(())
}

fn print_candidates(
    candidates: &[grafiki_core::ExtractionCandidate],
    format: OutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    match format {
        OutputFormat::Plain => {
            if candidates.is_empty() {
                println!("No candidates.");
            } else {
                for candidate in candidates {
                    print_candidate_plain(candidate);
                }
            }
        }
        OutputFormat::Md => {
            println!("# Grafiki Candidates\n");
            if candidates.is_empty() {
                println!("- None.");
            } else {
                for candidate in candidates {
                    print_candidate_md(candidate);
                }
            }
        }
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(candidates)?),
    }
    Ok(())
}

fn print_candidate_plain(candidate: &grafiki_core::ExtractionCandidate) {
    println!(
        "{} {} {} [{}] confidence {:.2}",
        candidate.id,
        candidate.status,
        candidate.record_type,
        display_scope(&candidate.scope),
        candidate.confidence
    );
    println!("Source: {}", candidate.source_type);
    if let Some(source) = &candidate.source {
        println!("Source id: {source}");
    }
    if let Some(trusted_id) = &candidate.trusted_record_id {
        println!(
            "Trusted record: {} {}",
            candidate.trusted_record_type.as_deref().unwrap_or("memory"),
            trusted_id
        );
    }
    if !candidate.evidence.is_empty() {
        println!("Evidence:");
        for evidence in candidate.evidence.iter().take(3) {
            println!(
                "  - {} {} {}",
                evidence.source_type,
                evidence.title.as_deref().unwrap_or(""),
                evidence.uri.as_deref().unwrap_or("")
            );
        }
    }
}

fn print_candidate_md(candidate: &grafiki_core::ExtractionCandidate) {
    println!(
        "- `{}` {} {} [{}] confidence {:.2} evidence {}",
        candidate.id,
        candidate.status,
        candidate.record_type,
        display_scope(&candidate.scope),
        candidate.confidence,
        candidate.evidence.len()
    );
}

fn print_ask_plain(briefing: &AgentMemoryBriefing) {
    println!("{}", briefing.answer);
    if !briefing.agent_instructions.is_empty() {
        println!();
        print_list("Agent Instructions", &briefing.agent_instructions);
    }
}

fn print_ask_md(briefing: &AgentMemoryBriefing) {
    println!("# Grafiki Agent Briefing\n");
    println!("- Project: {}", briefing.project);
    println!("- Scope: {}", display_scope(&briefing.scope));
    println!("- Question: {}\n", briefing.question);
    println!("```text\n{}\n```\n", briefing.answer);
    if !briefing.relevant_memory.is_empty() {
        println!("## Relevant Memory\n");
        for result in &briefing.relevant_memory {
            println!(
                "- **{}** `{}`: {} - {}",
                result.record_type, result.id, result.title, result.snippet
            );
            for evidence in result.evidence.iter().take(2) {
                println!(
                    "  - Evidence: {} {}",
                    evidence.source_type,
                    evidence.uri.as_deref().unwrap_or("")
                );
            }
        }
        println!();
    }
    if !briefing.agent_instructions.is_empty() {
        println!("## Agent Instructions\n");
        for instruction in &briefing.agent_instructions {
            println!("- {instruction}");
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct AutoCaptureReport {
    project: Option<String>,
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

#[derive(Debug)]
struct TerminalCommandCaptureOptions {
    project_name: Option<String>,
    start_dir: PathBuf,
    scope: String,
    command: String,
    cwd: Option<PathBuf>,
    exit_code: Option<i32>,
    duration_ms: Option<u64>,
    shell: Option<String>,
}

#[derive(Debug)]
struct FileWatchCaptureOptions {
    project_name: Option<String>,
    start_dir: PathBuf,
    scope: String,
    since_seconds: u64,
    duration_seconds: u64,
    interval_ms: u64,
    limit: usize,
    summarize: bool,
}

#[derive(Debug, Clone, Serialize)]
struct FileWatchCaptureReport {
    project: Option<String>,
    scope: String,
    path: String,
    files_seen: usize,
    events: Vec<CaptureEvent>,
    candidates: Option<CaptureCandidateReport>,
    message: String,
}

#[derive(Debug)]
struct GitSummaryCaptureOptions {
    project_name: Option<String>,
    start_dir: PathBuf,
    scope: String,
    source: String,
    limit: usize,
    summarize: bool,
}

#[derive(Debug, Clone, Serialize)]
struct GitSummaryCaptureReport {
    project: Option<String>,
    scope: String,
    path: String,
    git_root: Option<String>,
    branch: Option<String>,
    last_commit: Option<String>,
    changed_files: Vec<String>,
    staged_files: Vec<String>,
    unstaged_files: Vec<String>,
    untracked_files: Vec<String>,
    diff_stat: String,
    event: Option<CaptureEvent>,
    candidates: Option<CaptureCandidateReport>,
    message: String,
}

fn ensure_capture_source_enabled(
    project_name: Option<String>,
    start_dir: &Path,
    source: &str,
) -> Result<CaptureConfigReport, Box<dyn std::error::Error>> {
    let report = load_capture_config(CaptureConfigOptions {
        project_name,
        start_dir: start_dir.to_path_buf(),
        grafiki_home: None,
    })?;
    let enabled = match source {
        "git" => report.config.sources.git,
        "transcripts" | "transcript" => report.config.sources.transcripts,
        "terminal" => report.config.sources.terminal,
        "files" | "file" => report.config.sources.files,
        "ide" => report.config.sources.ide,
        "screen" => report.config.sources.screen,
        "browser" => report.config.sources.browser,
        "audio" => report.config.sources.audio,
        "system" => report.config.sources.system,
        _ => true,
    };
    if !enabled {
        return Err(format!(
            "Grafiki capture source '{source}' is disabled in {}",
            report.config_path.display()
        )
        .into());
    }
    Ok(report)
}

fn is_blocked_capture_path(path: &str, blocked_paths: &[String]) -> bool {
    let normalized = path.trim_start_matches("./").trim_end_matches('/');
    blocked_paths.iter().any(|blocked| {
        let blocked = blocked.trim_start_matches("./").trim_end_matches('/');
        !blocked.is_empty()
            && (normalized == blocked
                || normalized.starts_with(&format!("{blocked}/"))
                || normalized.contains(&format!("/{blocked}/")))
    })
}

fn auto_capture(
    project_name: Option<String>,
    start_dir: PathBuf,
    scope: String,
    source: Option<String>,
    limit: usize,
) -> Result<AutoCaptureReport, Box<dyn std::error::Error>> {
    ensure_capture_source_enabled(project_name.clone(), &start_dir, "git")?;
    let source_label = source.unwrap_or_else(|| "git-working-tree".to_owned());
    let display_path = start_dir.display().to_string();
    let bounded_limit = limit.clamp(1, 200);
    let git_root = git_output(&start_dir, &["rev-parse", "--show-toplevel"]).unwrap_or_default();
    let (status, diff_stat, last_commit, captured_from) = if git_root.is_some() {
        let status_text =
            git_output(&start_dir, &["status", "--porcelain=v1"])?.unwrap_or_default();
        let last_commit = git_output(&start_dir, &["log", "-1", "--pretty=format:%h %s"])?
            .filter(|value| !value.trim().is_empty());
        (
            parse_git_status(&status_text),
            capture_diff_stat(&start_dir)?,
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
        return Ok(AutoCaptureReport {
            project: project_name,
            scope,
            source: source_label,
            path: display_path,
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
        &source_label,
        &display_path,
        git_root.as_deref(),
        last_commit.as_deref(),
        &status,
        &diff_stat,
        bounded_limit,
    );
    let project_for_candidate = project_name.clone();
    let path_for_candidate = start_dir.clone();
    let evidence = vec![EvidenceInput {
        source_event_id: None,
        source_type: "git".to_owned(),
        source: git_root
            .as_ref()
            .cloned()
            .or_else(|| Some(display_path.clone())),
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
        project_name: project_for_candidate.clone(),
        start_dir: path_for_candidate.clone(),
        grafiki_home: None,
        source_type: "agent:auto-capture".to_owned(),
        source: Some(source_label.clone()),
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
    })?;
    let state_candidate = propose_candidate(ProposeCandidateOptions {
        project_name: project_for_candidate,
        start_dir: path_for_candidate,
        grafiki_home: None,
        source_type: "agent:auto-capture".to_owned(),
        source: Some(source_label.clone()),
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
    })?;

    Ok(AutoCaptureReport {
        project: project_name,
        scope,
        source: source_label,
        path: display_path,
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

fn print_shell_hook(
    project_name: Option<String>,
    scope: String,
    path: PathBuf,
    command: String,
) -> Result<(), Box<dyn std::error::Error>> {
    let path = path.canonicalize().unwrap_or(path);
    let mut args = vec![
        "capture".to_owned(),
        "terminal-command".to_owned(),
        "--path".to_owned(),
        shell_quote(&path.display().to_string()),
        "--scope".to_owned(),
        shell_quote(&scope),
    ];
    if let Some(project) = project_name {
        args.push("--project".to_owned());
        args.push(shell_quote(&project));
    }
    let base_args = args.join(" ");
    let command = shell_quote(&command);

    println!(
        r#"# Grafiki zsh terminal capture hook
# Add this block to ~/.zshrc after installing Grafiki.
# It records command, cwd, exit code, and duration metadata only; stdout is not captured.
autoload -Uz add-zsh-hook

__grafiki_preexec() {{
  __GRAFIKI_LAST_COMMAND="$1"
  __GRAFIKI_LAST_CWD="$PWD"
  __GRAFIKI_LAST_START="$SECONDS"
}}

__grafiki_precmd() {{
  local __grafiki_exit_code="$?"
  if [[ -n "${{__GRAFIKI_LAST_COMMAND:-}}" ]]; then
    local __grafiki_duration_ms=""
    if [[ -n "${{__GRAFIKI_LAST_START:-}}" ]]; then
      __grafiki_duration_ms=$(( (SECONDS - __GRAFIKI_LAST_START) * 1000 ))
    fi
    command {command} {base_args} \
      --cmd "$__GRAFIKI_LAST_COMMAND" \
      --cwd "$__GRAFIKI_LAST_CWD" \
      --exit-code "$__grafiki_exit_code" \
      --duration-ms "$__grafiki_duration_ms" \
      --shell zsh \
      --format json >/dev/null 2>&1 || true
    unset __GRAFIKI_LAST_COMMAND __GRAFIKI_LAST_CWD __GRAFIKI_LAST_START
  fi
  return "$__grafiki_exit_code"
}}

add-zsh-hook preexec __grafiki_preexec
add-zsh-hook precmd __grafiki_precmd"#
    );
    Ok(())
}

fn capture_terminal_command(
    options: TerminalCommandCaptureOptions,
) -> Result<CaptureEventReport, Box<dyn std::error::Error>> {
    ensure_capture_source_enabled(options.project_name.clone(), &options.start_dir, "terminal")?;
    let cwd = options
        .cwd
        .unwrap_or_else(|| env::current_dir().unwrap_or_else(|_| options.start_dir.clone()));
    let source = options
        .shell
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("terminal")
        .to_owned();
    let title = format!("Terminal: {}", one_line(&options.command, 96));
    let mut text = vec![
        format!("Command: {}", options.command),
        format!("Cwd: {}", cwd.display()),
    ];
    if let Some(exit_code) = options.exit_code {
        text.push(format!("Exit code: {exit_code}"));
    }
    if let Some(duration_ms) = options.duration_ms {
        text.push(format!("Duration: {duration_ms} ms"));
    }

    let report = ingest_capture_event(IngestCaptureEventOptions {
        project_name: options.project_name,
        start_dir: options.start_dir,
        grafiki_home: None,
        capture_id: None,
        scope: options.scope,
        source_type: "terminal".to_owned(),
        source: Some(source.clone()),
        title: Some(title),
        text: Some(text.join("\n")),
        payload: Some(serde_json::json!({
            "command": options.command,
            "cwd": cwd.display().to_string(),
            "exit_code": options.exit_code,
            "duration_ms": options.duration_ms,
            "shell": source,
            "stdout_captured": false,
        })),
        metadata: Some(serde_json::json!({
            "capture_adapter": "grafiki-shell-hook-v1",
        })),
        privacy_level: Some("internal".to_owned()),
        redacted: false,
        captured_at: None,
    })?;
    Ok(report)
}

fn watch_files_capture(
    options: FileWatchCaptureOptions,
) -> Result<FileWatchCaptureReport, Box<dyn std::error::Error>> {
    let config_report =
        ensure_capture_source_enabled(options.project_name.clone(), &options.start_dir, "files")?;
    let path = options.start_dir.display().to_string();
    let limit = options.limit.clamp(1, 500);
    let interval_ms = options.interval_ms.clamp(100, 60_000);
    let started = current_unix_secs();
    let end_at = started.saturating_add(options.duration_seconds);
    let mut seen = HashMap::<String, u64>::new();
    let mut selected = Vec::<RecentWorkspaceFile>::new();

    loop {
        let since = current_unix_secs().saturating_sub(options.since_seconds);
        for file in recent_workspace_file_records(&options.start_dir, limit * 4)? {
            if is_blocked_capture_path(&file.path, &config_report.config.blocked_paths) {
                continue;
            }
            if file.modified_secs < since {
                continue;
            }
            let previous = seen.insert(file.path.clone(), file.modified_secs);
            if previous == Some(file.modified_secs) {
                continue;
            }
            if !selected.iter().any(|existing| existing.path == file.path) {
                selected.push(file);
            }
            if selected.len() >= limit {
                break;
            }
        }
        if options.duration_seconds == 0 || current_unix_secs() >= end_at || selected.len() >= limit
        {
            break;
        }
        thread::sleep(Duration::from_millis(interval_ms));
    }

    if selected.is_empty() {
        return Ok(FileWatchCaptureReport {
            project: options.project_name,
            scope: options.scope,
            path,
            files_seen: 0,
            events: Vec::new(),
            candidates: None,
            message: "No recent file changes found.".to_owned(),
        });
    }

    let capture = start_capture_session(StartCaptureOptions {
        project_name: options.project_name.clone(),
        start_dir: options.start_dir.clone(),
        grafiki_home: None,
        scope: options.scope.clone(),
        source_app: Some("file-watcher".to_owned()),
        consent_profile: Some("workspace-file-metadata".to_owned()),
        redaction_profile: Some("default".to_owned()),
    })?;
    let capture_id = capture.capture.id;
    let mut events = Vec::new();
    for file in selected.iter().take(limit) {
        let report = ingest_capture_event(IngestCaptureEventOptions {
            project_name: options.project_name.clone(),
            start_dir: options.start_dir.clone(),
            grafiki_home: None,
            capture_id: Some(capture_id.clone()),
            scope: options.scope.clone(),
            source_type: "file".to_owned(),
            source: Some(file.path.clone()),
            title: Some(format!("File changed: {}", file.path)),
            text: Some(format!(
                "File changed: {}\nModified unix: {}\nSize bytes: {}",
                file.path, file.modified_secs, file.size_bytes
            )),
            payload: Some(serde_json::json!({
                "path": file.path,
                "modified_unix": file.modified_secs,
                "size_bytes": file.size_bytes,
            })),
            metadata: Some(serde_json::json!({
                "capture_adapter": "grafiki-file-watch-v1",
                "stdout_captured": false,
            })),
            privacy_level: Some("internal".to_owned()),
            redacted: false,
            captured_at: None,
        })?;
        events.push(report.event);
    }
    stop_capture_session(StopCaptureOptions {
        project_name: options.project_name.clone(),
        start_dir: options.start_dir.clone(),
        grafiki_home: None,
        capture_id: capture_id.clone(),
    })?;

    let candidates = if options.summarize {
        Some(propose_capture_candidates(
            ProposeCaptureCandidatesOptions {
                project_name: options.project_name.clone(),
                start_dir: options.start_dir.clone(),
                grafiki_home: None,
                capture_id: Some(capture_id),
                scope: options.scope.clone(),
                limit: events.len().min(100),
            },
        )?)
    } else {
        None
    };

    Ok(FileWatchCaptureReport {
        project: options.project_name,
        scope: options.scope,
        path,
        files_seen: events.len(),
        events,
        candidates,
        message: "File changes captured into raw events.".to_owned(),
    })
}

fn capture_git_summary(
    options: GitSummaryCaptureOptions,
) -> Result<GitSummaryCaptureReport, Box<dyn std::error::Error>> {
    ensure_capture_source_enabled(options.project_name.clone(), &options.start_dir, "git")?;
    let path = options.start_dir.display().to_string();
    let limit = options.limit.clamp(1, 200);
    let git_root = git_output(&options.start_dir, &["rev-parse", "--show-toplevel"])?;
    if git_root.is_none() {
        return Ok(GitSummaryCaptureReport {
            project: options.project_name,
            scope: options.scope,
            path,
            git_root: None,
            branch: None,
            last_commit: None,
            changed_files: Vec::new(),
            staged_files: Vec::new(),
            unstaged_files: Vec::new(),
            untracked_files: Vec::new(),
            diff_stat: "No git working tree detected.".to_owned(),
            event: None,
            candidates: None,
            message: "No git working tree detected.".to_owned(),
        });
    }

    let status_text =
        git_output(&options.start_dir, &["status", "--porcelain=v1"])?.unwrap_or_default();
    let status = parse_git_status(&status_text);
    let branch = git_output(&options.start_dir, &["rev-parse", "--abbrev-ref", "HEAD"])?
        .filter(|value| !value.trim().is_empty());
    let last_commit = git_output(&options.start_dir, &["log", "-1", "--pretty=format:%h %s"])?
        .filter(|value| !value.trim().is_empty());
    let diff_stat = capture_diff_stat(&options.start_dir)?;
    let summary = auto_capture_summary(
        &options.source,
        &path,
        git_root.as_deref(),
        last_commit.as_deref(),
        &status,
        &diff_stat,
        limit,
    );

    let capture = start_capture_session(StartCaptureOptions {
        project_name: options.project_name.clone(),
        start_dir: options.start_dir.clone(),
        grafiki_home: None,
        scope: options.scope.clone(),
        source_app: Some("git".to_owned()),
        consent_profile: Some("workspace-git-metadata".to_owned()),
        redaction_profile: Some("default".to_owned()),
    })?;
    let capture_id = capture.capture.id;
    let event = ingest_capture_event(IngestCaptureEventOptions {
        project_name: options.project_name.clone(),
        start_dir: options.start_dir.clone(),
        grafiki_home: None,
        capture_id: Some(capture_id.clone()),
        scope: options.scope.clone(),
        source_type: "git".to_owned(),
        source: git_root.clone().or_else(|| Some(path.clone())),
        title: Some("Git working tree snapshot".to_owned()),
        text: Some(summary),
        payload: Some(serde_json::json!({
            "git_root": git_root,
            "branch": branch,
            "last_commit": last_commit,
            "changed_files": status.changed_files,
            "staged_files": status.staged_files,
            "unstaged_files": status.unstaged_files,
            "untracked_files": status.untracked_files,
            "diff_stat": diff_stat,
        })),
        metadata: Some(serde_json::json!({
            "capture_adapter": "grafiki-git-summary-v1",
            "source": options.source,
        })),
        privacy_level: Some("internal".to_owned()),
        redacted: false,
        captured_at: None,
    })?
    .event;
    stop_capture_session(StopCaptureOptions {
        project_name: options.project_name.clone(),
        start_dir: options.start_dir.clone(),
        grafiki_home: None,
        capture_id: capture_id.clone(),
    })?;

    let candidates = if options.summarize {
        Some(propose_capture_candidates(
            ProposeCaptureCandidatesOptions {
                project_name: options.project_name.clone(),
                start_dir: options.start_dir.clone(),
                grafiki_home: None,
                capture_id: Some(capture_id),
                scope: options.scope.clone(),
                limit: 20,
            },
        )?)
    } else {
        None
    };

    let payload = event.payload.clone().unwrap_or_default();
    let status = parse_git_status(&status_text);
    Ok(GitSummaryCaptureReport {
        project: options.project_name,
        scope: options.scope,
        path,
        git_root: payload
            .get("git_root")
            .and_then(serde_json::Value::as_str)
            .map(ToOwned::to_owned),
        branch: payload
            .get("branch")
            .and_then(serde_json::Value::as_str)
            .map(ToOwned::to_owned),
        last_commit: payload
            .get("last_commit")
            .and_then(serde_json::Value::as_str)
            .map(ToOwned::to_owned),
        changed_files: status.changed_files,
        staged_files: status.staged_files,
        unstaged_files: status.unstaged_files,
        untracked_files: status.untracked_files,
        diff_stat: payload
            .get("diff_stat")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        event: Some(event),
        candidates,
        message: "Git working-tree snapshot captured into raw events.".to_owned(),
    })
}

fn print_auto_capture(
    report: &AutoCaptureReport,
    format: OutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    match format {
        OutputFormat::Plain => {
            println!("{}", report.message);
            if let Some(root) = &report.git_root {
                println!("Git root: {root}");
            }
            println!("Changed files: {}", report.changed_files.len());
            for file in report.changed_files.iter().take(25) {
                println!("  - {file}");
            }
            if report.changed_files.len() > 25 {
                println!("  - ... {} more", report.changed_files.len() - 25);
            }
            if !report.candidates.is_empty() {
                println!();
                println!("Pending candidates:");
                for candidate in &report.candidates {
                    println!(
                        "  - {} {}",
                        candidate.candidate.record_type, candidate.candidate.id
                    );
                }
            }
        }
        OutputFormat::Md => {
            println!("# Grafiki Auto-Capture\n");
            println!("{}", report.message);
            println!("\n- Source: {}", report.source);
            println!("- Scope: {}", display_scope(&report.scope));
            println!("- Changed files: {}", report.changed_files.len());
            if !report.changed_files.is_empty() {
                println!("\n## Changed Files\n");
                for file in &report.changed_files {
                    println!("- `{file}`");
                }
            }
            if !report.candidates.is_empty() {
                println!("\n## Pending Candidates\n");
                for candidate in &report.candidates {
                    println!(
                        "- `{}` {}",
                        candidate.candidate.id, candidate.candidate.record_type
                    );
                }
            }
        }
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(report)?),
    }
    Ok(())
}

fn print_capture_session_report(
    report: &CaptureSessionReport,
    format: OutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    match format {
        OutputFormat::Plain => {
            println!("{}", report.message);
            println!("Capture: {}", report.capture.id);
            println!("Status: {}", report.capture.status);
            println!("Scope: {}", display_scope(&report.capture.scope));
        }
        OutputFormat::Md => {
            println!("# Grafiki Capture Session\n");
            println!("- ID: {}", report.capture.id);
            println!("- Status: {}", report.capture.status);
            println!("- Scope: {}", display_scope(&report.capture.scope));
            println!("- Message: {}", report.message);
        }
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(report)?),
    }
    Ok(())
}

fn print_capture_config_report(
    report: &CaptureConfigReport,
    format: OutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    match format {
        OutputFormat::Plain => {
            println!("Grafiki capture config: {}", report.project);
            println!("Config: {}", report.config_path.display());
            println!("Sources:");
            println!("  git: {}", report.config.sources.git);
            println!("  transcripts: {}", report.config.sources.transcripts);
            println!("  terminal: {}", report.config.sources.terminal);
            println!("  files: {}", report.config.sources.files);
            println!("  ide: {}", report.config.sources.ide);
            println!("  screen: {}", report.config.sources.screen);
            println!("  browser: {}", report.config.sources.browser);
            println!("  audio: {}", report.config.sources.audio);
            println!("  system: {}", report.config.sources.system);
            println!("Terminal output: {}", report.config.terminal_output);
            println!("Screen policy: {}", report.config.screen_policy);
            println!("Browser policy: {}", report.config.browser_policy);
            println!("Redaction: {}", report.config.redaction_profile);
            if !report.config.blocked_paths.is_empty() {
                print_list("Blocked paths", &report.config.blocked_paths);
            }
            if !report.config.blocked_apps.is_empty() {
                print_list("Blocked apps", &report.config.blocked_apps);
            }
        }
        OutputFormat::Md => {
            println!("# Grafiki Capture Config\n");
            println!("- Project: {}", report.project);
            println!("- Config: {}", report.config_path.display());
            println!("- Terminal output: {}", report.config.terminal_output);
            println!("- Screen policy: {}", report.config.screen_policy);
            println!("- Browser policy: {}", report.config.browser_policy);
            println!("- Redaction: {}", report.config.redaction_profile);
            println!("\n## Sources\n");
            println!("- git: {}", report.config.sources.git);
            println!("- transcripts: {}", report.config.sources.transcripts);
            println!("- terminal: {}", report.config.sources.terminal);
            println!("- files: {}", report.config.sources.files);
            println!("- ide: {}", report.config.sources.ide);
            println!("- screen: {}", report.config.sources.screen);
            println!("- browser: {}", report.config.sources.browser);
            println!("- audio: {}", report.config.sources.audio);
            println!("- system: {}", report.config.sources.system);
            print_md_list("Blocked paths", &report.config.blocked_paths);
            print_md_list("Blocked apps", &report.config.blocked_apps);
        }
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(report)?),
    }
    Ok(())
}

fn print_capture_event_report(
    report: &CaptureEventReport,
    format: OutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    match format {
        OutputFormat::Plain => {
            println!("{}", report.message);
            println!("Event: {}", report.event.id);
            println!("Capture: {}", report.event.capture_session);
            println!("Type: {}", report.event.source_type);
            println!("Scope: {}", display_scope(&report.event.scope));
        }
        OutputFormat::Md => {
            println!("# Grafiki Capture Event\n");
            println!("- ID: {}", report.event.id);
            println!("- Capture: {}", report.event.capture_session);
            println!("- Type: {}", report.event.source_type);
            println!("- Scope: {}", display_scope(&report.event.scope));
        }
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(report)?),
    }
    Ok(())
}

fn print_agent_transcript_import_report(
    report: &AgentTranscriptImportReport,
    format: OutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    match format {
        OutputFormat::Plain => {
            println!("{}", report.message);
            println!("Agent: {}", report.agent);
            println!("Capture: {}", report.capture_id);
            println!("Scope: {}", display_scope(&report.scope));
            println!("Files scanned: {}", report.files_scanned);
            println!("Files imported: {}", report.files_imported);
            println!("Events imported: {}", report.events_imported);
            if let Some(candidates) = &report.candidates {
                println!("Candidates proposed: {}", candidates.candidates.len());
            }
            for source in &report.sources {
                match &source.skipped {
                    Some(reason) => println!("  - {} skipped: {}", source.path, reason),
                    None => println!("  - {} events: {}", source.path, source.events),
                }
            }
        }
        OutputFormat::Md => {
            println!("# Grafiki Transcript Import\n");
            println!("{}", report.message);
            println!("\n- Agent: {}", report.agent);
            println!("- Capture: {}", report.capture_id);
            println!("- Scope: {}", display_scope(&report.scope));
            println!("- Files scanned: {}", report.files_scanned);
            println!("- Files imported: {}", report.files_imported);
            println!("- Events imported: {}", report.events_imported);
            if let Some(candidates) = &report.candidates {
                println!("- Candidates proposed: {}", candidates.candidates.len());
            }
            if !report.sources.is_empty() {
                println!("\n## Sources\n");
                for source in &report.sources {
                    match &source.skipped {
                        Some(reason) => println!("- `{}` skipped: {}", source.path, reason),
                        None => println!("- `{}` events: {}", source.path, source.events),
                    }
                }
            }
        }
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(report)?),
    }
    Ok(())
}

fn print_capture_status(
    report: &CaptureStatusReport,
    format: OutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    match format {
        OutputFormat::Plain => {
            println!("Project: {}", report.project);
            println!("Scope: {}", display_scope(&report.scope));
            println!("Active capture sessions: {}", report.active_sessions.len());
            println!("Captured events: {}", report.event_count);
            for session in &report.active_sessions {
                println!(
                    "  - {} {}",
                    session.id,
                    session.source_app.as_deref().unwrap_or("")
                );
            }
        }
        OutputFormat::Md => {
            println!("# Grafiki Capture Status\n");
            println!("- Project: {}", report.project);
            println!("- Scope: {}", display_scope(&report.scope));
            println!("- Active sessions: {}", report.active_sessions.len());
            println!("- Events: {}", report.event_count);
        }
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(report)?),
    }
    Ok(())
}

fn print_capture_events(
    events: &[CaptureEvent],
    format: OutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    match format {
        OutputFormat::Plain => {
            if events.is_empty() {
                println!("No capture events.");
            } else {
                for event in events {
                    println!(
                        "{} [{}] {} {}",
                        event.id,
                        event.source_type,
                        display_scope(&event.scope),
                        event.title.as_deref().unwrap_or("")
                    );
                }
            }
        }
        OutputFormat::Md => {
            println!("# Grafiki Capture Events\n");
            if events.is_empty() {
                println!("- None.");
            } else {
                for event in events {
                    println!(
                        "- `{}` [{}] {} {}",
                        event.id,
                        event.source_type,
                        display_scope(&event.scope),
                        event.title.as_deref().unwrap_or("")
                    );
                }
            }
        }
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(events)?),
    }
    Ok(())
}

fn print_capture_candidate_report(
    report: &CaptureCandidateReport,
    format: OutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    match format {
        OutputFormat::Plain => {
            println!("{}", report.message);
            println!("Events summarized: {}", report.events_summarized);
            for candidate in &report.candidates {
                println!(
                    "  - {} {}",
                    candidate.candidate.record_type, candidate.candidate.id
                );
            }
        }
        OutputFormat::Md => {
            println!("# Grafiki Capture Candidates\n");
            println!("- Events summarized: {}", report.events_summarized);
            for candidate in &report.candidates {
                println!(
                    "- `{}` {}",
                    candidate.candidate.id, candidate.candidate.record_type
                );
            }
        }
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(report)?),
    }
    Ok(())
}

fn print_file_watch_capture_report(
    report: &FileWatchCaptureReport,
    format: OutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    match format {
        OutputFormat::Plain => {
            println!("{}", report.message);
            println!("Files captured: {}", report.files_seen);
            for event in report.events.iter().take(25) {
                println!("  - {}", event.source.as_deref().unwrap_or(&event.id));
            }
            if let Some(candidates) = &report.candidates {
                println!("Candidates proposed: {}", candidates.candidates.len());
            }
        }
        OutputFormat::Md => {
            println!("# Grafiki File Watch Capture\n");
            println!("{}", report.message);
            println!("\n- Scope: {}", display_scope(&report.scope));
            println!("- Files captured: {}", report.files_seen);
            if !report.events.is_empty() {
                println!("\n## Files\n");
                for event in &report.events {
                    println!("- `{}`", event.source.as_deref().unwrap_or(&event.id));
                }
            }
        }
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(report)?),
    }
    Ok(())
}

fn print_git_summary_capture_report(
    report: &GitSummaryCaptureReport,
    format: OutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    match format {
        OutputFormat::Plain => {
            println!("{}", report.message);
            if let Some(root) = &report.git_root {
                println!("Git root: {root}");
            }
            if let Some(branch) = &report.branch {
                println!("Branch: {branch}");
            }
            println!("Changed files: {}", report.changed_files.len());
            if let Some(event) = &report.event {
                println!("Event: {}", event.id);
            }
            if let Some(candidates) = &report.candidates {
                println!("Candidates proposed: {}", candidates.candidates.len());
            }
        }
        OutputFormat::Md => {
            println!("# Grafiki Git Capture\n");
            println!("{}", report.message);
            if let Some(root) = &report.git_root {
                println!("\n- Git root: `{root}`");
            }
            if let Some(branch) = &report.branch {
                println!("- Branch: `{branch}`");
            }
            println!("- Changed files: {}", report.changed_files.len());
        }
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(report)?),
    }
    Ok(())
}

fn git_output(path: &Path, args: &[&str]) -> Result<Option<String>, Box<dyn std::error::Error>> {
    let output = ProcessCommand::new("git")
        .arg("-C")
        .arg(path)
        .args(args)
        .output()?;
    if !output.status.success() {
        return Ok(None);
    }
    Ok(Some(
        String::from_utf8_lossy(&output.stdout).trim().to_owned(),
    ))
}

fn capture_diff_stat(path: &Path) -> Result<String, Box<dyn std::error::Error>> {
    let mut sections = Vec::new();
    if let Some(stat) = git_output(path, &["diff", "--stat"])? {
        if !stat.trim().is_empty() {
            sections.push(format!("Unstaged diff:\n{stat}"));
        }
    }
    if let Some(stat) = git_output(path, &["diff", "--cached", "--stat"])? {
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
    size_bytes: u64,
}

fn recent_workspace_files(
    root: &Path,
    limit: usize,
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    Ok(recent_workspace_file_records(root, limit)?
        .into_iter()
        .map(|file| file.path)
        .collect())
}

fn recent_workspace_file_records(
    root: &Path,
    limit: usize,
) -> Result<Vec<RecentWorkspaceFile>, Box<dyn std::error::Error>> {
    let mut files = Vec::new();
    collect_recent_workspace_files(root, root, &mut files)?;
    files.sort_by(|left, right| {
        right
            .modified_secs
            .cmp(&left.modified_secs)
            .then_with(|| left.path.cmp(&right.path))
    });
    files.truncate(limit);
    Ok(files)
}

fn collect_recent_workspace_files(
    root: &Path,
    current: &Path,
    files: &mut Vec<RecentWorkspaceFile>,
) -> Result<(), Box<dyn std::error::Error>> {
    if files.len() > 5000 {
        return Ok(());
    }
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        if is_ignored_workspace_path(root, &path) {
            continue;
        }
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            collect_recent_workspace_files(root, &path, files)?;
        } else if file_type.is_file() {
            let metadata = entry.metadata()?;
            let size_bytes = metadata.len();
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
                size_bytes,
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
        if line.len() < 3 {
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

fn current_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0))
        .as_secs()
}

fn one_line(value: &str, max_chars: usize) -> String {
    let collapsed = value.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut output = String::new();
    for (index, character) in collapsed.chars().enumerate() {
        if index >= max_chars {
            output.push_str("...");
            break;
        }
        output.push(character);
    }
    if output.is_empty() {
        "empty command".to_owned()
    } else {
        output
    }
}

fn shell_quote(value: &str) -> String {
    if value.is_empty() {
        "''".to_owned()
    } else {
        format!("'{}'", value.replace('\'', "'\\''"))
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

fn print_status_plain(report: &grafiki_core::StatusReport) {
    println!("Project: {}", report.project);
    println!("Scope: {}", display_scope(&report.scope));
    print_list("Active Sessions", &report.active_sessions);
    print_list("Active Work", &report.active_state);
    print_list("Recent Decisions", &report.recent_decisions);
    print_list("Recent Events", &report.recent_events);
}

fn print_status_md(report: &grafiki_core::StatusReport) {
    println!("# Grafiki Status\n");
    println!("- Project: {}", report.project);
    println!("- Scope: {}", display_scope(&report.scope));
    print_md_list("Active Sessions", &report.active_sessions);
    print_md_list("Active Work", &report.active_state);
    print_md_list("Recent Decisions", &report.recent_decisions);
    print_md_list("Recent Events", &report.recent_events);
}

fn print_list(title: &str, items: &[String]) {
    println!();
    println!("{title}:");
    if items.is_empty() {
        println!("  None.");
    } else {
        for item in items {
            println!("  - {item}");
        }
    }
}

fn print_md_list(title: &str, items: &[String]) {
    println!();
    println!("## {title}");
    if items.is_empty() {
        println!("- None.");
    } else {
        for item in items {
            println!("- {item}");
        }
    }
}

fn print_graph_plain(report: &grafiki_core::GraphReport) {
    println!("Graph: {}", report.root);
    println!("Project: {}", report.project);
    println!("Depth: {}", report.depth);
    print_graph_entities(report);
    print_graph_relations(report);
}

fn print_graph_entities(report: &grafiki_core::GraphReport) {
    println!();
    println!("Entities:");
    if report.entities.is_empty() {
        println!("  None.");
    } else {
        for entity in &report.entities {
            println!(
                "  - {} {} ({}) [{}]",
                entity.id,
                entity.name,
                entity.entity_type,
                display_scope(&entity.scope)
            );
        }
    }
}

fn print_graph_relations(report: &grafiki_core::GraphReport) {
    println!();
    println!("Relations:");
    if report.relations.is_empty() {
        println!("  None.");
    } else {
        for relation in &report.relations {
            println!(
                "  - {} --{}--> {}",
                relation.from_entity, relation.relation, relation.to_entity
            );
        }
    }
}

fn graph_to_dot(report: &grafiki_core::GraphReport) -> String {
    let mut dot = String::from("digraph grafiki {\n");
    dot.push_str("  rankdir=LR;\n");
    for entity in &report.entities {
        dot.push_str(&format!(
            "  \"{}\" [label=\"{}\\n{}\"];\n",
            escape_dot(&entity.id),
            escape_dot(&entity.name),
            escape_dot(&entity.entity_type)
        ));
    }
    for relation in &report.relations {
        dot.push_str(&format!(
            "  \"{}\" -> \"{}\" [label=\"{}\"];\n",
            escape_dot(&relation.from_entity),
            escape_dot(&relation.to_entity),
            escape_dot(&relation.relation)
        ));
    }
    dot.push_str("}\n");
    dot
}

fn escape_dot(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn project_report_to_plain(report: &grafiki_core::ProjectReport) -> String {
    let mut output = String::new();
    output.push_str(&format!("Grafiki Report: {}\n", report.project));
    output.push_str(&format!("Scope: {}\n", display_scope(&report.scope)));
    output.push_str(&format!("Entities: {}\n", report.entity_count));
    output.push_str(&format!("Relations: {}\n", report.relation_count));
    output.push_str(&format!("Observations: {}\n", report.observation_count));
    output.push_str(&format!("Decisions: {}\n", report.decision_count));
    output.push_str(&format!(
        "Active sessions: {}\n",
        report.active_session_count
    ));
    append_degree_list(&mut output, "God Nodes", &report.god_nodes);
    append_degree_list(&mut output, "Orphan Entities", &report.orphan_entities);
    append_string_list(&mut output, "Suggested Queries", &report.suggested_queries);
    output
}

fn project_report_to_markdown(report: &grafiki_core::ProjectReport) -> String {
    let mut output = String::new();
    output.push_str(&format!("# Grafiki Report: {}\n\n", report.project));
    output.push_str(&format!("- Scope: {}\n", display_scope(&report.scope)));
    output.push_str(&format!("- Entities: {}\n", report.entity_count));
    output.push_str(&format!("- Relations: {}\n", report.relation_count));
    output.push_str(&format!("- Observations: {}\n", report.observation_count));
    output.push_str(&format!("- Decisions: {}\n", report.decision_count));
    output.push_str(&format!(
        "- Active sessions: {}\n",
        report.active_session_count
    ));
    append_degree_markdown_list(&mut output, "God Nodes", &report.god_nodes);
    append_degree_markdown_list(&mut output, "Orphan Entities", &report.orphan_entities);
    append_string_markdown_list(&mut output, "Suggested Queries", &report.suggested_queries);
    output
}

fn append_degree_list(output: &mut String, title: &str, nodes: &[grafiki_core::NodeDegree]) {
    output.push('\n');
    output.push_str(&format!("{title}:\n"));
    if nodes.is_empty() {
        output.push_str("  None.\n");
    } else {
        for node in nodes {
            output.push_str(&format!(
                "  - {} {} ({}) degree {} [{}]\n",
                node.id,
                node.name,
                node.entity_type,
                node.degree,
                display_scope(&node.scope)
            ));
        }
    }
}

fn append_degree_markdown_list(
    output: &mut String,
    title: &str,
    nodes: &[grafiki_core::NodeDegree],
) {
    output.push_str(&format!("\n## {title}\n"));
    if nodes.is_empty() {
        output.push_str("- None.\n");
    } else {
        for node in nodes {
            output.push_str(&format!(
                "- `{}` {} ({}) degree {} [{}]\n",
                node.id,
                node.name,
                node.entity_type,
                node.degree,
                display_scope(&node.scope)
            ));
        }
    }
}

fn append_string_list(output: &mut String, title: &str, items: &[String]) {
    output.push('\n');
    output.push_str(&format!("{title}:\n"));
    if items.is_empty() {
        output.push_str("  None.\n");
    } else {
        for item in items {
            output.push_str(&format!("  - {item}\n"));
        }
    }
}

fn append_string_markdown_list(output: &mut String, title: &str, items: &[String]) {
    output.push_str(&format!("\n## {title}\n"));
    if items.is_empty() {
        output.push_str("- None.\n");
    } else {
        for item in items {
            output.push_str(&format!("- {item}\n"));
        }
    }
}

fn print_import_report_plain(report: &grafiki_core::ImportReport) {
    println!("Imported Grafiki memory into: {}", report.project);
    println!("Source project: {}", report.source_project);
    println!("Entities: {}", report.entities);
    println!("Relations: {}", report.relations);
    println!("Skipped relations: {}", report.skipped_relations);
    println!("Observations: {}", report.observations);
    println!("Skipped observations: {}", report.skipped_observations);
    println!("Decisions: {}", report.decisions);
    println!("State items: {}", report.state);
    println!("Context: {}", report.context);
    println!("Sessions: {}", report.sessions);
}

fn print_import_report_md(report: &grafiki_core::ImportReport) {
    println!("# Grafiki Import");
    println!();
    println!("- Project: {}", report.project);
    println!("- Source project: {}", report.source_project);
    println!("- Entities: {}", report.entities);
    println!("- Relations: {}", report.relations);
    println!("- Skipped relations: {}", report.skipped_relations);
    println!("- Observations: {}", report.observations);
    println!("- Skipped observations: {}", report.skipped_observations);
    println!("- Decisions: {}", report.decisions);
    println!("- State items: {}", report.state);
    println!("- Context: {}", report.context);
    println!("- Sessions: {}", report.sessions);
}

fn print_embedding_status(
    report: &EmbeddingStatusReport,
    format: OutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    match format {
        OutputFormat::Plain => {
            println!("Project: {}", report.project);
            println!("Scope: {}", display_scope(&report.scope));
            print_embedding_runtime_plain(report);
            println!("Pending: {}", report.pending);
            println!("Embedded: {}", report.embedded);
            println!("Failed: {}", report.failed);
            println!("Skipped: {}", report.skipped);
            if report.metadata.is_empty() {
                println!("Metadata: none");
            } else {
                println!("Metadata:");
                for item in &report.metadata {
                    println!(
                        "  - {} / {} / {}d: {} records",
                        item.provider, item.model, item.dimension, item.records
                    );
                }
            }
        }
        OutputFormat::Md => {
            println!("# Grafiki Embeddings\n");
            println!("- Project: {}", report.project);
            println!("- Scope: {}", display_scope(&report.scope));
            println!(
                "- Provider: {} / {} / {}",
                report.runtime.provider,
                report.runtime.model,
                display_optional_dimension(report.runtime.dimension)
            );
            println!(
                "- Requested provider: {}",
                report.runtime.requested_provider
            );
            println!("- Vector backend: {}", report.runtime.vector_backend);
            println!(
                "- Embeddable records: {}",
                report.runtime.embeddable_records
            );
            println!("- Indexed records: {}", report.runtime.indexed_records);
            println!("- Fresh records: {}", report.runtime.fresh_records);
            println!(
                "- Missing or stale records: {}",
                report.runtime.missing_or_stale_records
            );
            if let Some(note) = &report.runtime.note {
                println!("- Provider note: {note}");
            }
            println!("- Pending: {}", report.pending);
            println!("- Embedded: {}", report.embedded);
            println!("- Failed: {}", report.failed);
            println!("- Skipped: {}", report.skipped);
            println!("\n## Metadata");
            if report.metadata.is_empty() {
                println!("- None.");
            } else {
                for item in &report.metadata {
                    println!(
                        "- {} / {} / {}d: {} records",
                        item.provider, item.model, item.dimension, item.records
                    );
                }
            }
        }
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(report)?),
    }
    Ok(())
}

fn print_embedding_runtime_plain(report: &EmbeddingStatusReport) {
    println!(
        "Provider: {} / {} / {}",
        report.runtime.provider,
        report.runtime.model,
        display_optional_dimension(report.runtime.dimension)
    );
    println!("Requested provider: {}", report.runtime.requested_provider);
    println!("Vector backend: {}", report.runtime.vector_backend);
    println!("Embeddable records: {}", report.runtime.embeddable_records);
    println!("Indexed records: {}", report.runtime.indexed_records);
    println!("Fresh records: {}", report.runtime.fresh_records);
    println!(
        "Missing or stale records: {}",
        report.runtime.missing_or_stale_records
    );
    if let Some(note) = &report.runtime.note {
        println!("Provider note: {note}");
    }
}

fn display_optional_dimension(dimension: Option<i64>) -> String {
    dimension
        .map(|dimension| format!("{dimension}d"))
        .unwrap_or_else(|| "unknown dimension".to_owned())
}

fn print_process_embeddings_report(
    report: &ProcessEmbeddingsReport,
    format: OutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    match format {
        OutputFormat::Plain => {
            println!("Project: {}", report.project);
            println!("Scope: {}", display_scope(&report.scope));
            println!(
                "Provider: {} / {} / {}d",
                report.provider, report.model, report.dimension
            );
            println!("Enqueued: {}", report.enqueued);
            println!("Processed: {}", report.processed);
            println!("Skipped: {}", report.skipped);
            println!("Failed: {}", report.failed);
            println!("Pending remaining: {}", report.pending_remaining);
        }
        OutputFormat::Md => {
            println!("# Grafiki Embedding Processing\n");
            println!("- Project: {}", report.project);
            println!("- Scope: {}", display_scope(&report.scope));
            println!(
                "- Provider: {} / {} / {}d",
                report.provider, report.model, report.dimension
            );
            println!("- Enqueued: {}", report.enqueued);
            println!("- Processed: {}", report.processed);
            println!("- Skipped: {}", report.skipped);
            println!("- Failed: {}", report.failed);
            println!("- Pending remaining: {}", report.pending_remaining);
        }
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(report)?),
    }
    Ok(())
}

fn export_bundle_to_markdown(bundle: &grafiki_core::ExportBundle) -> String {
    let mut markdown = String::new();
    markdown.push_str(&format!("# Grafiki Export: {}\n\n", bundle.project));
    markdown.push_str(&format!("- Scope: {}\n", display_scope(&bundle.scope)));
    markdown.push_str(&format!("- Entities: {}\n", bundle.entities.len()));
    markdown.push_str(&format!("- Relations: {}\n", bundle.relations.len()));
    markdown.push_str(&format!("- Observations: {}\n", bundle.observations.len()));
    markdown.push_str(&format!("- Decisions: {}\n", bundle.decisions.len()));
    markdown.push_str(&format!("- State items: {}\n", bundle.state.len()));
    markdown.push_str(&format!("- Context documents: {}\n", bundle.context.len()));
    markdown.push_str(&format!("- Sessions: {}\n", bundle.sessions.len()));

    markdown.push_str("\n## Entities\n");
    for entity in &bundle.entities {
        markdown.push_str(&format!(
            "- `{}` {} ({}) [{}]\n",
            entity.id,
            entity.name,
            entity.entity_type,
            display_scope(&entity.scope)
        ));
    }

    markdown.push_str("\n## Relations\n");
    for relation in &bundle.relations {
        markdown.push_str(&format!(
            "- `{}` --{}--> `{}` ({})\n",
            relation.from_entity, relation.relation, relation.to_entity, relation.source_type
        ));
    }

    markdown.push_str("\n## Observations\n");
    for observation in &bundle.observations {
        markdown.push_str(&format!(
            "- `{}` {}: {} [{}]\n",
            observation.entity_id,
            observation.category,
            observation.content,
            display_scope(&observation.scope)
        ));
    }

    markdown.push_str("\n## Decisions\n");
    for decision in &bundle.decisions {
        markdown.push_str(&format!(
            "- `{}` {} ({}) [{}]\n",
            decision.id,
            decision.title,
            decision.status,
            display_scope(&decision.scope)
        ));
    }

    markdown.push_str("\n## State\n");
    for item in &bundle.state {
        markdown.push_str(&format!(
            "- `{}` {} ({}, {}) [{}]\n",
            item.key,
            item.title,
            item.status,
            item.priority,
            display_scope(&item.scope)
        ));
    }

    markdown.push_str("\n## Context\n");
    for item in &bundle.context {
        markdown.push_str(&format!(
            "- `{}` {} ({}, v{}) [{}]\n",
            item.key,
            item.title,
            item.category,
            item.version,
            display_scope(&item.scope)
        ));
    }

    markdown.push_str("\n## Sessions\n");
    for session in &bundle.sessions {
        markdown.push_str(&format!(
            "- `{}` {} ({}) [{}]\n",
            session.id,
            session.session_type,
            session.status,
            display_scope(&session.scope)
        ));
    }

    markdown
}

fn export_bundle_to_dot(bundle: &grafiki_core::ExportBundle) -> String {
    let mut dot = String::from("digraph grafiki {\n");
    dot.push_str("  rankdir=LR;\n");
    for entity in &bundle.entities {
        dot.push_str(&format!(
            "  \"{}\" [label=\"{}\\n{}\"];\n",
            escape_dot(&entity.id),
            escape_dot(&entity.name),
            escape_dot(&entity.entity_type)
        ));
    }
    for relation in &bundle.relations {
        dot.push_str(&format!(
            "  \"{}\" -> \"{}\" [label=\"{}\"];\n",
            escape_dot(&relation.from_entity),
            escape_dot(&relation.to_entity),
            escape_dot(&relation.relation)
        ));
    }
    dot.push_str("}\n");
    dot
}

fn export_bundle_to_graphml(bundle: &grafiki_core::ExportBundle) -> String {
    let mut graphml = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    graphml.push_str("<graphml xmlns=\"http://graphml.graphdrawing.org/xmlns\">\n");
    graphml.push_str("  <key id=\"name\" for=\"node\" attr.name=\"name\" attr.type=\"string\"/>\n");
    graphml.push_str(
        "  <key id=\"entity_type\" for=\"node\" attr.name=\"entity_type\" attr.type=\"string\"/>\n",
    );
    graphml.push_str(
        "  <key id=\"node_scope\" for=\"node\" attr.name=\"scope\" attr.type=\"string\"/>\n",
    );
    graphml.push_str(
        "  <key id=\"relation\" for=\"edge\" attr.name=\"relation\" attr.type=\"string\"/>\n",
    );
    graphml.push_str(
        "  <key id=\"confidence\" for=\"edge\" attr.name=\"confidence\" attr.type=\"double\"/>\n",
    );
    graphml.push_str(
        "  <key id=\"source_type\" for=\"edge\" attr.name=\"source_type\" attr.type=\"string\"/>\n",
    );
    graphml.push_str(
        "  <key id=\"source\" for=\"edge\" attr.name=\"source\" attr.type=\"string\"/>\n",
    );
    graphml.push_str(&format!(
        "  <graph id=\"{}\" edgedefault=\"directed\">\n",
        escape_xml(&bundle.project)
    ));

    for entity in &bundle.entities {
        graphml.push_str(&format!("    <node id=\"{}\">\n", escape_xml(&entity.id)));
        graphml.push_str(&format!(
            "      <data key=\"name\">{}</data>\n",
            escape_xml(&entity.name)
        ));
        graphml.push_str(&format!(
            "      <data key=\"entity_type\">{}</data>\n",
            escape_xml(&entity.entity_type)
        ));
        graphml.push_str(&format!(
            "      <data key=\"node_scope\">{}</data>\n",
            escape_xml(&entity.scope)
        ));
        graphml.push_str("    </node>\n");
    }

    for relation in &bundle.relations {
        graphml.push_str(&format!(
            "    <edge id=\"{}\" source=\"{}\" target=\"{}\">\n",
            escape_xml(&relation.id),
            escape_xml(&relation.from_entity),
            escape_xml(&relation.to_entity)
        ));
        graphml.push_str(&format!(
            "      <data key=\"relation\">{}</data>\n",
            escape_xml(&relation.relation)
        ));
        graphml.push_str(&format!(
            "      <data key=\"confidence\">{}</data>\n",
            relation.confidence
        ));
        graphml.push_str(&format!(
            "      <data key=\"source_type\">{}</data>\n",
            escape_xml(&relation.source_type)
        ));
        if let Some(source) = &relation.source {
            graphml.push_str(&format!(
                "      <data key=\"source\">{}</data>\n",
                escape_xml(source)
            ));
        }
        graphml.push_str("    </edge>\n");
    }

    graphml.push_str("  </graph>\n");
    graphml.push_str("</graphml>\n");
    graphml
}

fn export_bundle_to_html(bundle: &grafiki_core::ExportBundle) -> String {
    let mut html = String::new();
    html.push_str("<!doctype html>\n<html lang=\"en\">\n<head>\n");
    html.push_str("  <meta charset=\"utf-8\">\n");
    html.push_str("  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n");
    html.push_str(&format!(
        "  <title>Grafiki Export - {}</title>\n",
        escape_xml(&bundle.project)
    ));
    html.push_str(
        r#"  <style>
    :root {
      color-scheme: light;
      --ink: #17201b;
      --muted: #5d675f;
      --line: #d9ded8;
      --paper: #f7f8f5;
      --panel: #ffffff;
      --accent: #0f766e;
      --accent-2: #b45309;
    }
    * { box-sizing: border-box; }
    body {
      margin: 0;
      font-family: Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
      color: var(--ink);
      background: var(--paper);
      letter-spacing: 0;
    }
    header {
      padding: 32px clamp(20px, 5vw, 64px) 20px;
      border-bottom: 1px solid var(--line);
      background: #ffffff;
    }
    main {
      width: min(1180px, calc(100vw - 32px));
      margin: 24px auto 48px;
    }
    h1, h2, h3 { margin: 0; line-height: 1.15; }
    h1 { font-size: clamp(2rem, 4vw, 3.8rem); }
    h2 { font-size: 1.15rem; margin-bottom: 12px; }
    h3 { font-size: 1rem; margin-bottom: 8px; }
    p { color: var(--muted); line-height: 1.55; }
    .meta {
      display: flex;
      flex-wrap: wrap;
      gap: 8px;
      margin-top: 16px;
    }
    .pill {
      border: 1px solid var(--line);
      border-radius: 999px;
      padding: 6px 10px;
      color: var(--muted);
      background: #fbfcfa;
      font-size: 0.86rem;
    }
    .grid {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(150px, 1fr));
      gap: 10px;
      margin-bottom: 22px;
    }
    .metric, section {
      background: var(--panel);
      border: 1px solid var(--line);
      border-radius: 8px;
    }
    .metric { padding: 14px; }
    .metric strong {
      display: block;
      font-size: 1.8rem;
      line-height: 1;
      color: var(--accent);
    }
    .metric span { color: var(--muted); font-size: 0.86rem; }
    section { padding: 18px; margin-top: 16px; overflow: auto; }
    svg {
      width: 100%;
      min-width: 720px;
      height: auto;
      display: block;
      background: #fbfcfa;
      border: 1px solid var(--line);
      border-radius: 8px;
    }
    table {
      width: 100%;
      border-collapse: collapse;
      font-size: 0.92rem;
    }
    th, td {
      padding: 10px 8px;
      border-bottom: 1px solid var(--line);
      vertical-align: top;
      text-align: left;
    }
    th { color: var(--muted); font-weight: 600; }
    code {
      font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
      font-size: 0.9em;
      background: #eef4f2;
      color: #0f4f4a;
      border-radius: 4px;
      padding: 2px 4px;
    }
  </style>
"#,
    );
    html.push_str("</head>\n<body>\n<header>\n");
    html.push_str(&format!(
        "  <h1>Grafiki Export: {}</h1>\n",
        escape_xml(&bundle.project)
    ));
    html.push_str("  <div class=\"meta\">\n");
    html.push_str(&format!(
        "    <span class=\"pill\">Scope: {}</span>\n",
        escape_xml(display_scope(&bundle.scope))
    ));
    html.push_str("    <span class=\"pill\">Local-first memory snapshot</span>\n");
    html.push_str("  </div>\n</header>\n<main>\n");

    html.push_str("  <div class=\"grid\">\n");
    html_metric(&mut html, "Entities", bundle.entities.len());
    html_metric(&mut html, "Relations", bundle.relations.len());
    html_metric(&mut html, "Observations", bundle.observations.len());
    html_metric(&mut html, "Decisions", bundle.decisions.len());
    html_metric(&mut html, "State", bundle.state.len());
    html_metric(&mut html, "Context", bundle.context.len());
    html.push_str("  </div>\n");

    html.push_str("  <section>\n    <h2>Knowledge Graph</h2>\n");
    html.push_str(&export_bundle_to_svg(bundle));
    html.push_str("  </section>\n");

    html.push_str("  <section>\n    <h2>Entities</h2>\n    <table>\n");
    html.push_str("      <thead><tr><th>ID</th><th>Name</th><th>Type</th><th>Scope</th></tr></thead>\n      <tbody>\n");
    for entity in &bundle.entities {
        html.push_str(&format!(
            "        <tr><td><code>{}</code></td><td>{}</td><td>{}</td><td>{}</td></tr>\n",
            escape_xml(&entity.id),
            escape_xml(&entity.name),
            escape_xml(&entity.entity_type),
            escape_xml(display_scope(&entity.scope))
        ));
    }
    html.push_str("      </tbody>\n    </table>\n  </section>\n");

    html.push_str("  <section>\n    <h2>Relations</h2>\n    <table>\n");
    html.push_str("      <thead><tr><th>From</th><th>Relation</th><th>To</th><th>Source</th></tr></thead>\n      <tbody>\n");
    for relation in &bundle.relations {
        html.push_str(&format!(
            "        <tr><td><code>{}</code></td><td>{}</td><td><code>{}</code></td><td>{}</td></tr>\n",
            escape_xml(&relation.from_entity),
            escape_xml(&relation.relation),
            escape_xml(&relation.to_entity),
            escape_xml(relation.source.as_deref().unwrap_or(&relation.source_type))
        ));
    }
    html.push_str("      </tbody>\n    </table>\n  </section>\n");

    html.push_str("  <section>\n    <h2>Observations</h2>\n    <table>\n");
    html.push_str("      <thead><tr><th>Entity</th><th>Category</th><th>Observation</th><th>Scope</th></tr></thead>\n      <tbody>\n");
    for observation in &bundle.observations {
        html.push_str(&format!(
            "        <tr><td><code>{}</code></td><td>{}</td><td>{}</td><td>{}</td></tr>\n",
            escape_xml(&observation.entity_id),
            escape_xml(&observation.category),
            escape_xml(&observation.content),
            escape_xml(display_scope(&observation.scope))
        ));
    }
    html.push_str("      </tbody>\n    </table>\n  </section>\n");

    html.push_str("  <section>\n    <h2>Decisions</h2>\n    <table>\n");
    html.push_str("      <thead><tr><th>ID</th><th>Title</th><th>Status</th><th>Scope</th></tr></thead>\n      <tbody>\n");
    for decision in &bundle.decisions {
        html.push_str(&format!(
            "        <tr><td><code>{}</code></td><td>{}</td><td>{}</td><td>{}</td></tr>\n",
            escape_xml(&decision.id),
            escape_xml(&decision.title),
            escape_xml(&decision.status),
            escape_xml(display_scope(&decision.scope))
        ));
    }
    html.push_str("      </tbody>\n    </table>\n  </section>\n");

    html.push_str("</main>\n</body>\n</html>\n");
    html
}

fn html_metric(html: &mut String, label: &str, value: usize) {
    html.push_str(&format!(
        "    <div class=\"metric\"><strong>{}</strong><span>{}</span></div>\n",
        value,
        escape_xml(label)
    ));
}

fn export_bundle_to_svg(bundle: &grafiki_core::ExportBundle) -> String {
    let width = 1000.0;
    let height = 650.0;
    let center_x = width / 2.0;
    let center_y = height / 2.0;
    let radius = 245.0;
    let count = bundle.entities.len().max(1) as f64;
    let mut positions: HashMap<&str, (f64, f64)> = HashMap::new();

    for (index, entity) in bundle.entities.iter().enumerate() {
        let angle = std::f64::consts::TAU * index as f64 / count - std::f64::consts::FRAC_PI_2;
        let x = center_x + radius * angle.cos();
        let y = center_y + radius * angle.sin();
        positions.insert(entity.id.as_str(), (x, y));
    }

    let mut svg = format!(
        "    <svg viewBox=\"0 0 {} {}\" role=\"img\" aria-label=\"Grafiki knowledge graph\">\n",
        width as i32, height as i32
    );

    for relation in &bundle.relations {
        let Some((from_x, from_y)) = positions.get(relation.from_entity.as_str()) else {
            continue;
        };
        let Some((to_x, to_y)) = positions.get(relation.to_entity.as_str()) else {
            continue;
        };
        let label_x = (from_x + to_x) / 2.0;
        let label_y = (from_y + to_y) / 2.0;
        svg.push_str(&format!(
            "      <line x1=\"{from_x:.1}\" y1=\"{from_y:.1}\" x2=\"{to_x:.1}\" y2=\"{to_y:.1}\" stroke=\"#91a19a\" stroke-width=\"1.5\"/>\n"
        ));
        svg.push_str(&format!(
            "      <text x=\"{label_x:.1}\" y=\"{label_y:.1}\" text-anchor=\"middle\" fill=\"#5d675f\" font-size=\"12\">{}</text>\n",
            escape_xml(&relation.relation)
        ));
    }

    for entity in &bundle.entities {
        let Some((x, y)) = positions.get(entity.id.as_str()) else {
            continue;
        };
        svg.push_str(&format!(
            "      <circle cx=\"{x:.1}\" cy=\"{y:.1}\" r=\"34\" fill=\"#e6f3f1\" stroke=\"#0f766e\" stroke-width=\"2\"/>\n"
        ));
        svg.push_str(&format!(
            "      <text x=\"{x:.1}\" y=\"{:.1}\" text-anchor=\"middle\" fill=\"#17201b\" font-size=\"12\" font-weight=\"700\">{}</text>\n",
            y - 3.0,
            escape_xml(&truncate_label(&entity.name, 18))
        ));
        svg.push_str(&format!(
            "      <text x=\"{x:.1}\" y=\"{:.1}\" text-anchor=\"middle\" fill=\"#5d675f\" font-size=\"10\">{}</text>\n",
            y + 12.0,
            escape_xml(&truncate_label(&entity.entity_type, 16))
        ));
    }

    if bundle.entities.is_empty() {
        svg.push_str("      <text x=\"500\" y=\"325\" text-anchor=\"middle\" fill=\"#5d675f\" font-size=\"18\">No entities exported for this scope.</text>\n");
    }

    svg.push_str("    </svg>\n");
    svg
}

fn truncate_label(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_owned();
    }
    let mut label: String = value.chars().take(max_chars.saturating_sub(3)).collect();
    label.push_str("...");
    label
}

fn export_bundle_to_wiki(
    bundle: &grafiki_core::ExportBundle,
    output_dir: &PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    let entities_dir = output_dir.join("entities");
    let decisions_dir = output_dir.join("decisions");
    fs::create_dir_all(&entities_dir)?;
    fs::create_dir_all(&decisions_dir)?;

    fs::write(output_dir.join("index.md"), wiki_index(bundle))?;
    fs::write(output_dir.join("relations.md"), wiki_relations(bundle))?;
    fs::write(
        output_dir.join("observations.md"),
        wiki_observations(bundle),
    )?;
    fs::write(output_dir.join("state.md"), wiki_state(bundle))?;
    fs::write(output_dir.join("context.md"), wiki_context(bundle))?;
    fs::write(output_dir.join("sessions.md"), wiki_sessions(bundle))?;

    for entity in &bundle.entities {
        let path = entities_dir.join(format!("{}.md", safe_file_stem(&entity.id)));
        fs::write(path, wiki_entity_page(bundle, entity))?;
    }

    for decision in &bundle.decisions {
        let path = decisions_dir.join(format!("{}.md", safe_file_stem(&decision.id)));
        fs::write(path, wiki_decision_page(decision))?;
    }

    Ok(())
}

fn wiki_index(bundle: &grafiki_core::ExportBundle) -> String {
    let mut markdown = String::new();
    markdown.push_str(&format!("# Grafiki Wiki: {}\n\n", bundle.project));
    markdown.push_str(&format!("- Scope: {}\n", display_scope(&bundle.scope)));
    markdown.push_str(&format!("- Entities: {}\n", bundle.entities.len()));
    markdown.push_str(&format!("- Relations: {}\n", bundle.relations.len()));
    markdown.push_str(&format!("- Observations: {}\n", bundle.observations.len()));
    markdown.push_str(&format!("- Decisions: {}\n", bundle.decisions.len()));
    markdown.push_str(&format!("- State items: {}\n", bundle.state.len()));
    markdown.push_str(&format!("- Context documents: {}\n", bundle.context.len()));
    markdown.push_str(&format!("- Sessions: {}\n", bundle.sessions.len()));
    markdown.push_str("\n## Navigation\n");
    markdown.push_str("- [Relations](relations.md)\n");
    markdown.push_str("- [Observations](observations.md)\n");
    markdown.push_str("- [State](state.md)\n");
    markdown.push_str("- [Context](context.md)\n");
    markdown.push_str("- [Sessions](sessions.md)\n");
    markdown.push_str("\n## Entities\n");
    if bundle.entities.is_empty() {
        markdown.push_str("- None.\n");
    }
    for entity in &bundle.entities {
        markdown.push_str(&format!(
            "- [{}](entities/{}.md) ({}) [{}]\n",
            entity.name,
            safe_file_stem(&entity.id),
            entity.entity_type,
            display_scope(&entity.scope)
        ));
    }
    markdown.push_str("\n## Decisions\n");
    if bundle.decisions.is_empty() {
        markdown.push_str("- None.\n");
    }
    for decision in &bundle.decisions {
        markdown.push_str(&format!(
            "- [{}](decisions/{}.md) ({}) [{}]\n",
            decision.title,
            safe_file_stem(&decision.id),
            decision.status,
            display_scope(&decision.scope)
        ));
    }
    markdown
}

fn wiki_entity_page(
    bundle: &grafiki_core::ExportBundle,
    entity: &grafiki_core::GraphEntity,
) -> String {
    let mut markdown = String::new();
    markdown.push_str(&format!("# {}\n\n", entity.name));
    markdown.push_str(&format!("- ID: `{}`\n", entity.id));
    markdown.push_str(&format!("- Type: `{}`\n", entity.entity_type));
    markdown.push_str(&format!("- Scope: {}\n", display_scope(&entity.scope)));

    markdown.push_str("\n## Observations\n");
    let mut observation_count = 0;
    for observation in bundle
        .observations
        .iter()
        .filter(|observation| observation.entity_id == entity.id)
    {
        observation_count += 1;
        markdown.push_str(&format!(
            "- {}: {} [{}]\n",
            observation.category,
            observation.content,
            display_scope(&observation.scope)
        ));
    }
    if observation_count == 0 {
        markdown.push_str("- None.\n");
    }

    markdown.push_str("\n## Relations\n");
    let mut relation_count = 0;
    for relation in bundle
        .relations
        .iter()
        .filter(|relation| relation.from_entity == entity.id || relation.to_entity == entity.id)
    {
        relation_count += 1;
        markdown.push_str(&format!(
            "- `{}` --{}--> `{}` ({})\n",
            relation.from_entity, relation.relation, relation.to_entity, relation.source_type
        ));
    }
    if relation_count == 0 {
        markdown.push_str("- None.\n");
    }

    markdown
}

fn wiki_decision_page(decision: &grafiki_core::ExportDecision) -> String {
    let mut markdown = String::new();
    markdown.push_str(&format!("# {}\n\n", decision.title));
    markdown.push_str(&format!("- ID: `{}`\n", decision.id));
    markdown.push_str(&format!("- Status: `{}`\n", decision.status));
    markdown.push_str(&format!("- Scope: {}\n", display_scope(&decision.scope)));
    if let Some(reasoning) = &decision.reasoning {
        markdown.push_str("\n## Reasoning\n");
        markdown.push_str(reasoning);
        markdown.push('\n');
    }
    markdown
}

fn wiki_relations(bundle: &grafiki_core::ExportBundle) -> String {
    let mut markdown = String::from("# Relations\n\n");
    if bundle.relations.is_empty() {
        markdown.push_str("- None.\n");
    }
    for relation in &bundle.relations {
        markdown.push_str(&format!(
            "- `{}` --{}--> `{}` (confidence {}, source {})\n",
            relation.from_entity,
            relation.relation,
            relation.to_entity,
            relation.confidence,
            relation.source.as_deref().unwrap_or(&relation.source_type)
        ));
    }
    markdown
}

fn wiki_observations(bundle: &grafiki_core::ExportBundle) -> String {
    let mut markdown = String::from("# Observations\n\n");
    if bundle.observations.is_empty() {
        markdown.push_str("- None.\n");
    }
    for observation in &bundle.observations {
        markdown.push_str(&format!(
            "- `{}` {}: {} [{}]\n",
            observation.entity_id,
            observation.category,
            observation.content,
            display_scope(&observation.scope)
        ));
    }
    markdown
}

fn wiki_state(bundle: &grafiki_core::ExportBundle) -> String {
    let mut markdown = String::from("# State\n\n");
    if bundle.state.is_empty() {
        markdown.push_str("- None.\n");
    }
    for item in &bundle.state {
        markdown.push_str(&format!(
            "- `{}` {} ({}, {}) [{}]\n",
            item.key,
            item.title,
            item.status,
            item.priority,
            display_scope(&item.scope)
        ));
    }
    markdown
}

fn wiki_context(bundle: &grafiki_core::ExportBundle) -> String {
    let mut markdown = String::from("# Context\n\n");
    if bundle.context.is_empty() {
        markdown.push_str("- None.\n");
    }
    for item in &bundle.context {
        markdown.push_str(&format!(
            "- `{}` {} ({}, v{}) [{}]\n",
            item.key,
            item.title,
            item.category,
            item.version,
            display_scope(&item.scope)
        ));
    }
    markdown
}

fn wiki_sessions(bundle: &grafiki_core::ExportBundle) -> String {
    let mut markdown = String::from("# Sessions\n\n");
    if bundle.sessions.is_empty() {
        markdown.push_str("- None.\n");
    }
    for session in &bundle.sessions {
        markdown.push_str(&format!(
            "- `{}` {} ({}) [{}]\n",
            session.id,
            session.session_type,
            session.status,
            display_scope(&session.scope)
        ));
    }
    markdown
}

fn safe_file_stem(value: &str) -> String {
    let mut stem = String::new();
    let mut previous_was_separator = false;
    for character in value.chars() {
        let next = if character.is_ascii_alphanumeric() || character == '_' {
            Some(character.to_ascii_lowercase())
        } else if character == '-' {
            Some('-')
        } else {
            None
        };
        match next {
            Some(character) => {
                stem.push(character);
                previous_was_separator = character == '-';
            }
            None if !previous_was_separator => {
                stem.push('-');
                previous_was_separator = true;
            }
            None => {}
        }
    }
    let stem = stem.trim_matches('-').to_owned();
    if stem.is_empty() {
        "item".to_owned()
    } else {
        stem
    }
}

fn write_or_print(
    output: Option<PathBuf>,
    content: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    match output {
        Some(path) => fs::write(path, content)?,
        None => println!("{content}"),
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DaemonRecord {
    project: String,
    pid: u32,
    host: String,
    port: u16,
    log_path: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
struct DaemonStartReport {
    project: String,
    running: bool,
    already_running: bool,
    pid: u32,
    host: String,
    port: u16,
    pid_path: PathBuf,
    log_path: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
struct DaemonStatusReport {
    project: String,
    running: bool,
    pid: Option<u32>,
    host: Option<String>,
    port: Option<u16>,
    pid_path: PathBuf,
    log_path: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
struct DaemonStopReport {
    project: String,
    stopped: bool,
    pid: Option<u32>,
    pid_path: PathBuf,
}

fn daemon_start(
    project: Option<String>,
    path: PathBuf,
    host: String,
    port: u16,
    allow_non_local: bool,
    token: Option<String>,
    log: Option<PathBuf>,
) -> Result<DaemonStartReport, Box<dyn std::error::Error>> {
    if !allow_non_local && !is_local_bind_host(&host) {
        return Err(format!(
            "Refusing to bind HTTP API to non-local host {host}. Use --allow-non-local if you really want this."
        )
        .into());
    }
    if allow_non_local && token.as_deref().unwrap_or_default().is_empty() {
        return Err("Non-local HTTP binds require --token or GRAFIKI_HTTP_TOKEN.".into());
    }

    let context = grafiki_core::resolve_project(ProjectResolveOptions {
        project_name: project,
        start_dir: path,
        grafiki_home: None,
    })?;
    let pid_path = daemon_pid_path(&context)?;
    let log_path = log.unwrap_or(daemon_log_path(&context)?);
    ensure_parent_dir(&pid_path)?;
    ensure_parent_dir(&log_path)?;

    if let Some(record) = read_daemon_record(&pid_path)? {
        if pid_running(record.pid) {
            return Ok(DaemonStartReport {
                project: context.project,
                running: true,
                already_running: true,
                pid: record.pid,
                host: record.host,
                port: record.port,
                pid_path,
                log_path: record.log_path,
            });
        }
        fs::remove_file(&pid_path)?;
    }

    let log_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)?;
    let stderr_file = log_file.try_clone()?;
    let executable = env::current_exe()?;
    let mut command = ProcessCommand::new(executable);
    command
        .arg("serve")
        .arg("--project")
        .arg(&context.project)
        .arg("--path")
        .arg(&context.project_dir)
        .arg("--host")
        .arg(&host)
        .arg("--port")
        .arg(port.to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::from(log_file))
        .stderr(Stdio::from(stderr_file));
    if allow_non_local {
        command.arg("--allow-non-local");
    }
    if let Some(token) = &token {
        command.arg("--token").arg(token);
    }
    let child = command.spawn()?;
    let pid = child.id();
    let record = DaemonRecord {
        project: context.project.clone(),
        pid,
        host: host.clone(),
        port,
        log_path: log_path.clone(),
    };
    fs::write(&pid_path, serde_json::to_string_pretty(&record)?)?;

    Ok(DaemonStartReport {
        project: context.project,
        running: true,
        already_running: false,
        pid,
        host,
        port,
        pid_path,
        log_path,
    })
}

fn daemon_status(
    project: Option<String>,
    path: PathBuf,
) -> Result<DaemonStatusReport, Box<dyn std::error::Error>> {
    let context = grafiki_core::resolve_project(ProjectResolveOptions {
        project_name: project,
        start_dir: path,
        grafiki_home: None,
    })?;
    let pid_path = daemon_pid_path(&context)?;
    let default_log_path = daemon_log_path(&context)?;
    let record = read_daemon_record(&pid_path)?;
    let running = record
        .as_ref()
        .map(|record| pid_running(record.pid))
        .unwrap_or(false);

    Ok(DaemonStatusReport {
        project: context.project,
        running,
        pid: record.as_ref().map(|record| record.pid),
        host: record.as_ref().map(|record| record.host.clone()),
        port: record.as_ref().map(|record| record.port),
        pid_path,
        log_path: record
            .as_ref()
            .map(|record| record.log_path.clone())
            .unwrap_or(default_log_path),
    })
}

fn daemon_stop(
    project: Option<String>,
    path: PathBuf,
) -> Result<DaemonStopReport, Box<dyn std::error::Error>> {
    let context = grafiki_core::resolve_project(ProjectResolveOptions {
        project_name: project,
        start_dir: path,
        grafiki_home: None,
    })?;
    let pid_path = daemon_pid_path(&context)?;
    let record = read_daemon_record(&pid_path)?;
    let Some(record) = record else {
        return Ok(DaemonStopReport {
            project: context.project,
            stopped: false,
            pid: None,
            pid_path,
        });
    };

    let running = pid_running(record.pid);
    if running {
        let _ = ProcessCommand::new("kill")
            .arg(record.pid.to_string())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        for _ in 0..20 {
            if !pid_running(record.pid) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }
    if pid_path.exists() {
        fs::remove_file(&pid_path)?;
    }

    Ok(DaemonStopReport {
        project: context.project,
        stopped: running,
        pid: Some(record.pid),
        pid_path,
    })
}

fn daemon_dir(
    context: &grafiki_core::ProjectContext,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let home = context
        .db_path
        .parent()
        .ok_or("Could not resolve Grafiki home directory")?;
    Ok(home.join("daemons"))
}

fn daemon_pid_path(
    context: &grafiki_core::ProjectContext,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    Ok(daemon_dir(context)?.join(format!("{}.pid.json", context.project)))
}

fn daemon_log_path(
    context: &grafiki_core::ProjectContext,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    Ok(daemon_dir(context)?.join(format!("{}.log", context.project)))
}

fn read_daemon_record(
    pid_path: &PathBuf,
) -> Result<Option<DaemonRecord>, Box<dyn std::error::Error>> {
    if !pid_path.exists() {
        return Ok(None);
    }
    let content = fs::read_to_string(pid_path)?;
    Ok(Some(serde_json::from_str(&content)?))
}

fn ensure_parent_dir(path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}

fn pid_running(pid: u32) -> bool {
    ProcessCommand::new("kill")
        .arg("-0")
        .arg(pid.to_string())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn print_daemon_start_report(
    report: &DaemonStartReport,
    format: OutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    match format {
        OutputFormat::Plain => {
            if report.already_running {
                println!("Grafiki daemon already running: {}", report.project);
            } else {
                println!("Started Grafiki daemon: {}", report.project);
            }
            println!("PID: {}", report.pid);
            println!("URL: http://{}:{}", report.host, report.port);
            println!("PID file: {}", report.pid_path.display());
            println!("Log: {}", report.log_path.display());
        }
        OutputFormat::Md => {
            println!("# Grafiki Daemon");
            println!();
            println!("- Project: {}", report.project);
            println!("- Running: {}", report.running);
            println!("- Already running: {}", report.already_running);
            println!("- PID: {}", report.pid);
            println!("- URL: http://{}:{}", report.host, report.port);
            println!("- PID file: {}", report.pid_path.display());
            println!("- Log: {}", report.log_path.display());
        }
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(report)?),
    }
    Ok(())
}

fn print_daemon_status_report(
    report: &DaemonStatusReport,
    format: OutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    match format {
        OutputFormat::Plain => {
            println!("Grafiki daemon: {}", report.project);
            println!("Running: {}", report.running);
            if let Some(pid) = report.pid {
                println!("PID: {pid}");
            }
            if let (Some(host), Some(port)) = (&report.host, report.port) {
                println!("URL: http://{host}:{port}");
            }
            println!("PID file: {}", report.pid_path.display());
            println!("Log: {}", report.log_path.display());
        }
        OutputFormat::Md => {
            println!("# Grafiki Daemon Status");
            println!();
            println!("- Project: {}", report.project);
            println!("- Running: {}", report.running);
            if let Some(pid) = report.pid {
                println!("- PID: {pid}");
            }
            if let (Some(host), Some(port)) = (&report.host, report.port) {
                println!("- URL: http://{host}:{port}");
            }
            println!("- PID file: {}", report.pid_path.display());
            println!("- Log: {}", report.log_path.display());
        }
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(report)?),
    }
    Ok(())
}

fn print_daemon_stop_report(
    report: &DaemonStopReport,
    format: OutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    match format {
        OutputFormat::Plain => {
            if report.stopped {
                println!("Stopped Grafiki daemon: {}", report.project);
            } else {
                println!("Grafiki daemon was not running: {}", report.project);
            }
            if let Some(pid) = report.pid {
                println!("PID: {pid}");
            }
            println!("PID file: {}", report.pid_path.display());
        }
        OutputFormat::Md => {
            println!("# Grafiki Daemon Stopped");
            println!();
            println!("- Project: {}", report.project);
            println!("- Stopped: {}", report.stopped);
            if let Some(pid) = report.pid {
                println!("- PID: {pid}");
            }
            println!("- PID file: {}", report.pid_path.display());
        }
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(report)?),
    }
    Ok(())
}

#[derive(Debug)]
struct HttpRequest {
    method: String,
    path: String,
    query: HashMap<String, String>,
    headers: HashMap<String, String>,
    body: String,
}

#[derive(Debug)]
struct HttpResponse {
    status: u16,
    content_type: String,
    body: String,
}

#[derive(Debug, Serialize)]
struct RecordMutationResponse {
    record_type: String,
    id: String,
    title: String,
    scope: String,
    message: String,
}

impl HttpResponse {
    fn new(status: u16, content_type: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            status,
            content_type: content_type.into(),
            body: body.into(),
        }
    }

    fn json(status: u16, value: serde_json::Value) -> Self {
        let body = serde_json::to_string_pretty(&value)
            .unwrap_or_else(|_| "{\"error\":\"failed to serialize response\"}".to_owned());
        Self::new(status, "application/json; charset=utf-8", body)
    }
}

fn serve_http(
    project: Option<String>,
    path: PathBuf,
    host: String,
    port: u16,
    allow_non_local: bool,
    token: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    if !allow_non_local && !is_local_bind_host(&host) {
        return Err(format!(
            "Refusing to bind HTTP API to non-local host {host}. Use --allow-non-local if you really want this."
        )
        .into());
    }
    if allow_non_local && token.as_deref().unwrap_or_default().is_empty() {
        return Err("Non-local HTTP binds require --token or GRAFIKI_HTTP_TOKEN.".into());
    }

    let listener = TcpListener::bind(format!("{host}:{port}"))?;
    spawn_embedding_worker(project.clone(), path.clone());
    println!("Grafiki HTTP API listening on http://{host}:{port}");
    println!("Health: http://{host}:{port}/health");

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                // Handle each connection on its own thread so one slow/stalled
                // client cannot block the whole API (slowloris). Each handler
                // opens its own SQLite connection, which is safe under WAL.
                let project = project.clone();
                let path = path.clone();
                let token = token.clone();
                thread::spawn(move || {
                    if let Err(error) = handle_http_stream(stream, project, path, token) {
                        eprintln!("HTTP request failed: {error}");
                    }
                });
            }
            Err(error) => eprintln!("HTTP connection failed: {error}"),
        }
    }

    Ok(())
}

fn spawn_embedding_worker(project: Option<String>, path: PathBuf) {
    thread::spawn(move || loop {
        thread::sleep(Duration::from_secs(2));
        if let Err(error) = process_embedding_jobs(ProcessEmbeddingsOptions {
            project_name: project.clone(),
            start_dir: path.clone(),
            grafiki_home: None,
            scope: "*".to_owned(),
            limit: 100,
            rebuild: false,
        }) {
            eprintln!("Embedding worker skipped batch: {error}");
        }
    });
}

fn is_local_bind_host(host: &str) -> bool {
    matches!(host, "127.0.0.1" | "localhost" | "::1")
}

fn handle_http_stream(
    mut stream: TcpStream,
    base_project: Option<String>,
    base_path: PathBuf,
    token: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Bound how long a single client can hold a worker thread.
    stream.set_read_timeout(Some(Duration::from_secs(15)))?;
    stream.set_write_timeout(Some(Duration::from_secs(15)))?;
    let response = match parse_http_request(&stream) {
        Ok(request) => match route_http_request(&request, base_project, base_path, token) {
            Ok(response) => response,
            Err(error) => {
                // Log details server-side; never leak internals to the client.
                eprintln!("HTTP handler error: {error}");
                HttpResponse::json(500, serde_json::json!({ "error": "internal error" }))
            }
        },
        Err(error) => {
            eprintln!("HTTP request parse error: {error}");
            HttpResponse::json(400, serde_json::json!({ "error": "invalid request" }))
        }
    };
    write_http_response(&mut stream, response)?;
    Ok(())
}

fn parse_http_request(stream: &TcpStream) -> Result<HttpRequest, Box<dyn std::error::Error>> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next().ok_or("Missing HTTP method")?.to_owned();
    let target = parts.next().ok_or("Missing HTTP target")?;

    let mut content_length = 0usize;
    let mut headers = HashMap::new();
    loop {
        let mut header = String::new();
        let bytes = reader.read_line(&mut header)?;
        if bytes == 0 || header == "\r\n" || header == "\n" {
            break;
        }
        if let Some((name, value)) = header.split_once(':') {
            let name = name.trim().to_ascii_lowercase();
            let value = value.trim().to_owned();
            if name == "content-length" {
                content_length = value.parse::<usize>().unwrap_or(0);
            }
            headers.insert(name, value);
        }
    }

    // Cap body size so an attacker-controlled Content-Length cannot trigger an
    // unbounded allocation (OOM). 16 MiB is far above any legitimate request.
    const MAX_BODY_BYTES: usize = 16 * 1024 * 1024;
    if content_length > MAX_BODY_BYTES {
        return Err("request body exceeds 16MiB limit".into());
    }
    let mut body = vec![0u8; content_length];
    if content_length > 0 {
        reader.read_exact(&mut body)?;
    }
    let (path, query) = parse_http_target(target);
    Ok(HttpRequest {
        method,
        path,
        query,
        headers,
        body: String::from_utf8_lossy(&body).into_owned(),
    })
}

fn parse_http_target(target: &str) -> (String, HashMap<String, String>) {
    let (path, query_string) = target.split_once('?').unwrap_or((target, ""));
    let mut query = HashMap::new();
    for pair in query_string.split('&').filter(|pair| !pair.is_empty()) {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        query.insert(percent_decode(key), percent_decode(value));
    }
    (percent_decode(path), query)
}

/// Constant-time comparison so token verification does not leak the secret via
/// response timing. (Length is allowed to differ early, as is standard.)
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

fn http_authorized(request: &HttpRequest, token: Option<&str>) -> bool {
    let Some(expected) = token.filter(|token| !token.is_empty()) else {
        return true;
    };
    if request
        .query
        .get("token")
        .map(|token| constant_time_eq(token.as_bytes(), expected.as_bytes()))
        .unwrap_or(false)
    {
        return true;
    }
    if request
        .headers
        .get("x-grafiki-token")
        .map(|token| constant_time_eq(token.as_bytes(), expected.as_bytes()))
        .unwrap_or(false)
    {
        return true;
    }
    request
        .headers
        .get("authorization")
        .and_then(|header| header.strip_prefix("Bearer "))
        .map(|token| constant_time_eq(token.as_bytes(), expected.as_bytes()))
        .unwrap_or(false)
}

fn route_http_request(
    request: &HttpRequest,
    base_project: Option<String>,
    base_path: PathBuf,
    token: Option<String>,
) -> Result<HttpResponse, Box<dyn std::error::Error>> {
    // Only an explicit allowlist of routes is public. Using ends_with("/health")
    // here previously let "/api/context/health", "/api/memory/<t>/health" etc.
    // bypass the token check and return real data.
    let is_public = matches!(request.path.as_str(), "/" | "/health" | "/api/health");
    if !is_public && !http_authorized(request, token.as_deref()) {
        return Ok(HttpResponse::json(
            401,
            serde_json::json!({ "error": "Unauthorized" }),
        ));
    }

    match (request.method.as_str(), request.path.as_str()) {
        ("GET", "/" | "/health" | "/api/health") => Ok(HttpResponse::json(
            200,
            serde_json::json!({ "status": "ok", "service": "grafiki" }),
        )),
        ("GET", "/api/status") => http_status(&request.query, base_project, base_path),
        ("GET", "/api/ask") => http_ask(&request.query, base_project, base_path),
        ("GET", "/api/search") => http_search(&request.query, base_project, base_path),
        ("GET", "/api/embeddings/status") => {
            http_embeddings_status(&request.query, base_project, base_path)
        }
        ("GET", "/api/report" | "/api/analyze") => {
            http_report(&request.query, base_project, base_path)
        }
        ("GET", "/api/export") => http_export(&request.query, base_project, base_path),
        ("GET", "/api/events") => http_events(&request.query, base_project, base_path),
        ("GET", "/api/log") => http_log(&request.query, base_project, base_path),
        ("GET", "/api/agent-queries" | "/api/agent/activity") => {
            http_agent_queries(&request.query, base_project, base_path)
        }
        ("GET", "/api/candidates") => http_candidates_list(&request.query, base_project, base_path),
        ("GET", "/api/context") => http_context_list(&request.query, base_project, base_path),
        ("GET", path) if path.starts_with("/api/graph/") => {
            let entity_id = path.trim_start_matches("/api/graph/").to_owned();
            http_graph(&request.query, base_project, base_path, entity_id)
        }
        ("GET", path) if path.starts_with("/api/context/") => {
            let key = path.trim_start_matches("/api/context/").to_owned();
            http_context(&request.query, base_project, base_path, key)
        }
        ("GET", path) if path.starts_with("/api/memory/") => {
            let rest = path.trim_start_matches("/api/memory/");
            let (record_type, id) = rest
                .split_once('/')
                .ok_or("Expected /api/memory/<type>/<id>")?;
            http_memory_record(
                &request.query,
                base_project,
                base_path,
                record_type.to_owned(),
                id.to_owned(),
            )
        }
        ("POST", "/api/sessions/start") => http_start(request, base_project, base_path),
        ("POST", "/api/sessions/end") => http_end(request, base_project, base_path),
        ("POST", "/api/sessions/handoff") => http_handoff(request, base_project, base_path),
        ("POST", "/api/capture/auto") => http_auto_capture(request, base_project, base_path),
        ("POST", "/api/capture/start") => http_capture_start(request, base_project, base_path),
        ("POST", "/api/capture/stop") => http_capture_stop(request, base_project, base_path),
        ("POST", "/api/capture/ingest") => http_capture_ingest(request, base_project, base_path),
        ("POST", "/api/capture/import-transcripts") => {
            http_capture_import_transcripts(request, base_project, base_path)
        }
        ("GET", "/api/capture/config") => http_capture_config(base_project, base_path),
        ("POST", "/api/capture/config") => {
            http_capture_config_update(request, base_project, base_path)
        }
        ("POST", "/api/capture/terminal-command") => {
            http_capture_terminal_command(request, base_project, base_path)
        }
        ("POST", "/api/capture/watch-files") => {
            http_capture_watch_files(request, base_project, base_path)
        }
        ("POST", "/api/capture/git-summary") => {
            http_capture_git_summary(request, base_project, base_path)
        }
        ("POST", "/api/capture/summarize") => {
            http_capture_summarize(request, base_project, base_path)
        }
        ("GET", "/api/capture/status") => {
            http_capture_status(&request.query, base_project, base_path)
        }
        ("GET", "/api/capture/events") => {
            http_capture_events(&request.query, base_project, base_path)
        }
        ("POST", "/api/entities/save") => http_save(request, base_project, base_path),
        ("POST", "/api/decisions") => http_decide(request, base_project, base_path),
        ("POST", "/api/state") => http_state_set(request, base_project, base_path),
        ("POST", "/api/context/add") => http_context_add(request, base_project, base_path),
        ("POST", "/api/context/update") => http_context_update(request, base_project, base_path),
        ("POST", "/api/context/delete") => http_context_delete(request, base_project, base_path),
        ("POST", "/api/memory/update") => http_memory_update(request, base_project, base_path),
        ("POST", "/api/memory/delete") => http_memory_delete(request, base_project, base_path),
        ("POST", "/api/candidates/propose") => {
            http_candidate_propose(request, base_project, base_path)
        }
        ("POST", "/api/candidates/edit") => http_candidate_edit(request, base_project, base_path),
        ("POST", "/api/candidates/approve") => {
            http_candidate_approve(request, base_project, base_path)
        }
        ("POST", "/api/candidates/bulk") => http_candidate_bulk(request, base_project, base_path),
        ("POST", "/api/candidates/reject") => {
            http_candidate_reject(request, base_project, base_path)
        }
        ("POST", "/api/embeddings/process") => {
            http_embeddings_process(request, base_project, base_path, false)
        }
        ("POST", "/api/embeddings/rebuild") => {
            http_embeddings_process(request, base_project, base_path, true)
        }
        ("POST", "/api/import") => http_import(request, base_project, base_path),
        ("GET" | "POST", _) => Ok(HttpResponse::json(
            404,
            serde_json::json!({ "error": "Unknown endpoint" }),
        )),
        _ => Ok(HttpResponse::json(
            405,
            serde_json::json!({ "error": "Method not allowed" }),
        )),
    }
}

fn http_status(
    query: &HashMap<String, String>,
    base_project: Option<String>,
    base_path: PathBuf,
) -> Result<HttpResponse, Box<dyn std::error::Error>> {
    let report = get_status(StatusOptions {
        project_name: query_project(query, base_project),
        start_dir: query_path(query, base_path),
        grafiki_home: None,
        scope: query_value(query, "scope", ""),
    })?;
    json_response(&report)
}

fn http_ask(
    query: &HashMap<String, String>,
    base_project: Option<String>,
    base_path: PathBuf,
) -> Result<HttpResponse, Box<dyn std::error::Error>> {
    let question = query
        .get("q")
        .or_else(|| query.get("question"))
        .ok_or("Missing required query parameter: q")?
        .to_owned();
    let briefing = ask_memory(AskMemoryOptions {
        project_name: query_project(query, base_project),
        start_dir: query_path(query, base_path),
        grafiki_home: None,
        question,
        scope: query_value(query, "scope", ""),
        limit: query_usize(query, "limit", 8),
        agent: query
            .get("agent")
            .cloned()
            .or_else(|| Some("http".to_owned())),
    })?;
    json_response(&briefing)
}

fn http_auto_capture(
    request: &HttpRequest,
    base_project: Option<String>,
    base_path: PathBuf,
) -> Result<HttpResponse, Box<dyn std::error::Error>> {
    let body = json_body(request)?;
    let report = auto_capture(
        json_optional_string(&body, "project").or(base_project),
        json_optional_string(&body, "path")
            .map(PathBuf::from)
            .unwrap_or(base_path),
        json_arg_string(&body, "scope", ""),
        json_optional_string(&body, "source"),
        json_arg_usize(&body, "limit", 25),
    )?;
    json_response(&report)
}

fn http_capture_start(
    request: &HttpRequest,
    base_project: Option<String>,
    base_path: PathBuf,
) -> Result<HttpResponse, Box<dyn std::error::Error>> {
    let body = json_body(request)?;
    let report = start_capture_session(StartCaptureOptions {
        project_name: json_optional_string(&body, "project").or(base_project),
        start_dir: json_optional_string(&body, "path")
            .map(PathBuf::from)
            .unwrap_or(base_path),
        grafiki_home: None,
        scope: json_arg_string(&body, "scope", ""),
        source_app: json_optional_string(&body, "source_app")
            .or_else(|| json_optional_string(&body, "sourceApp")),
        consent_profile: json_optional_string(&body, "consent_profile")
            .or_else(|| json_optional_string(&body, "consentProfile")),
        redaction_profile: json_optional_string(&body, "redaction_profile")
            .or_else(|| json_optional_string(&body, "redactionProfile")),
    })?;
    json_response(&report)
}

fn http_capture_stop(
    request: &HttpRequest,
    base_project: Option<String>,
    base_path: PathBuf,
) -> Result<HttpResponse, Box<dyn std::error::Error>> {
    let body = json_body(request)?;
    let report = stop_capture_session(StopCaptureOptions {
        project_name: json_optional_string(&body, "project").or(base_project),
        start_dir: json_optional_string(&body, "path")
            .map(PathBuf::from)
            .unwrap_or(base_path),
        grafiki_home: None,
        capture_id: json_required_string(&body, "id")
            .or_else(|_| json_required_string(&body, "capture"))?,
    })?;
    json_response(&report)
}

fn http_capture_ingest(
    request: &HttpRequest,
    base_project: Option<String>,
    base_path: PathBuf,
) -> Result<HttpResponse, Box<dyn std::error::Error>> {
    let body = json_body(request)?;
    let report = ingest_capture_event(IngestCaptureEventOptions {
        project_name: json_optional_string(&body, "project").or(base_project),
        start_dir: json_optional_string(&body, "path")
            .map(PathBuf::from)
            .unwrap_or(base_path),
        grafiki_home: None,
        capture_id: json_optional_string(&body, "capture")
            .or_else(|| json_optional_string(&body, "capture_id"))
            .or_else(|| json_optional_string(&body, "captureId")),
        scope: json_arg_string(&body, "scope", ""),
        source_type: json_optional_string(&body, "source_type")
            .or_else(|| json_optional_string(&body, "sourceType"))
            .or_else(|| json_optional_string(&body, "type"))
            .ok_or("Missing required argument: source_type")?,
        source: json_optional_string(&body, "source"),
        title: json_optional_string(&body, "title"),
        text: json_optional_string(&body, "text"),
        payload: body.get("payload").cloned(),
        metadata: body.get("metadata").cloned(),
        privacy_level: json_optional_string(&body, "privacy")
            .or_else(|| json_optional_string(&body, "privacy_level"))
            .or_else(|| json_optional_string(&body, "privacyLevel")),
        redacted: json_arg_bool(&body, "redacted", false),
        captured_at: json_optional_string(&body, "captured_at")
            .or_else(|| json_optional_string(&body, "capturedAt")),
    })?;
    json_response(&report)
}

fn http_capture_import_transcripts(
    request: &HttpRequest,
    base_project: Option<String>,
    base_path: PathBuf,
) -> Result<HttpResponse, Box<dyn std::error::Error>> {
    let body = json_body(request)?;
    let project_name = json_optional_string(&body, "project").or(base_project);
    let start_dir = json_optional_string(&body, "path")
        .map(PathBuf::from)
        .unwrap_or(base_path);
    ensure_capture_source_enabled(project_name.clone(), &start_dir, "transcripts")?;
    let report = import_agent_transcripts(ImportAgentTranscriptsOptions {
        project_name,
        start_dir,
        grafiki_home: None,
        agent: json_required_string(&body, "agent")?,
        input: json_optional_string(&body, "input").map(PathBuf::from),
        scope: json_arg_string(&body, "scope", ""),
        limit: json_arg_usize(&body, "limit", 200),
        summarize: json_arg_bool(&body, "summarize", false),
    })?;
    json_response(&report)
}

fn http_capture_config(
    base_project: Option<String>,
    base_path: PathBuf,
) -> Result<HttpResponse, Box<dyn std::error::Error>> {
    let report = load_capture_config(CaptureConfigOptions {
        project_name: base_project,
        start_dir: base_path,
        grafiki_home: None,
    })?;
    json_response(&report)
}

fn http_capture_config_update(
    request: &HttpRequest,
    base_project: Option<String>,
    base_path: PathBuf,
) -> Result<HttpResponse, Box<dyn std::error::Error>> {
    let body = json_body(request)?;
    let report = update_capture_config(UpdateCaptureConfigOptions {
        project_name: json_optional_string(&body, "project").or(base_project),
        start_dir: json_optional_string(&body, "path")
            .map(PathBuf::from)
            .unwrap_or(base_path),
        grafiki_home: None,
        sources: CaptureSourceUpdates {
            git: json_optional_bool(&body, "git"),
            transcripts: json_optional_bool(&body, "transcripts"),
            terminal: json_optional_bool(&body, "terminal"),
            files: json_optional_bool(&body, "files"),
            ide: json_optional_bool(&body, "ide"),
            screen: json_optional_bool(&body, "screen"),
            browser: json_optional_bool(&body, "browser"),
            audio: json_optional_bool(&body, "audio"),
            system: json_optional_bool(&body, "system"),
        },
        add_blocked_paths: json_optional_vec(&body, "add_blocked_paths")
            .or_else(|| json_optional_vec(&body, "add_blocked_path"))
            .or_else(|| json_optional_vec(&body, "blocked_paths"))
            .unwrap_or_default(),
        remove_blocked_paths: json_optional_vec(&body, "remove_blocked_paths")
            .or_else(|| json_optional_vec(&body, "remove_blocked_path"))
            .unwrap_or_default(),
        add_blocked_apps: json_optional_vec(&body, "add_blocked_apps")
            .or_else(|| json_optional_vec(&body, "add_blocked_app"))
            .or_else(|| json_optional_vec(&body, "blocked_apps"))
            .unwrap_or_default(),
        remove_blocked_apps: json_optional_vec(&body, "remove_blocked_apps")
            .or_else(|| json_optional_vec(&body, "remove_blocked_app"))
            .unwrap_or_default(),
        redaction_profile: json_optional_string(&body, "redaction_profile")
            .or_else(|| json_optional_string(&body, "redactionProfile")),
        terminal_output: json_optional_string(&body, "terminal_output")
            .or_else(|| json_optional_string(&body, "terminalOutput")),
        screen_policy: json_optional_string(&body, "screen_policy")
            .or_else(|| json_optional_string(&body, "screenPolicy")),
        browser_policy: json_optional_string(&body, "browser_policy")
            .or_else(|| json_optional_string(&body, "browserPolicy")),
    })?;
    json_response(&report)
}

fn http_capture_terminal_command(
    request: &HttpRequest,
    base_project: Option<String>,
    base_path: PathBuf,
) -> Result<HttpResponse, Box<dyn std::error::Error>> {
    let body = json_body(request)?;
    let report = capture_terminal_command(TerminalCommandCaptureOptions {
        project_name: json_optional_string(&body, "project").or(base_project),
        start_dir: json_optional_string(&body, "path")
            .map(PathBuf::from)
            .unwrap_or(base_path),
        scope: json_arg_string(&body, "scope", ""),
        command: json_optional_string(&body, "command")
            .or_else(|| json_optional_string(&body, "cmd"))
            .ok_or("Missing required argument: command")?,
        cwd: json_optional_string(&body, "cwd").map(PathBuf::from),
        exit_code: json_optional_i32(&body, "exit_code")
            .or_else(|| json_optional_i32(&body, "exitCode")),
        duration_ms: json_optional_u64(&body, "duration_ms")
            .or_else(|| json_optional_u64(&body, "durationMs")),
        shell: json_optional_string(&body, "shell"),
    })?;
    json_response(&report)
}

fn http_capture_watch_files(
    request: &HttpRequest,
    base_project: Option<String>,
    base_path: PathBuf,
) -> Result<HttpResponse, Box<dyn std::error::Error>> {
    let body = json_body(request)?;
    let report = watch_files_capture(FileWatchCaptureOptions {
        project_name: json_optional_string(&body, "project").or(base_project),
        start_dir: json_optional_string(&body, "path")
            .map(PathBuf::from)
            .unwrap_or(base_path),
        scope: json_arg_string(&body, "scope", ""),
        since_seconds: json_optional_u64(&body, "since_seconds")
            .or_else(|| json_optional_u64(&body, "sinceSeconds"))
            .unwrap_or(300),
        duration_seconds: json_optional_u64(&body, "duration_seconds")
            .or_else(|| json_optional_u64(&body, "durationSeconds"))
            .unwrap_or(0),
        interval_ms: json_optional_u64(&body, "interval_ms")
            .or_else(|| json_optional_u64(&body, "intervalMs"))
            .unwrap_or(1000),
        limit: json_arg_usize(&body, "limit", 100),
        summarize: json_arg_bool(&body, "summarize", false),
    })?;
    json_response(&report)
}

fn http_capture_git_summary(
    request: &HttpRequest,
    base_project: Option<String>,
    base_path: PathBuf,
) -> Result<HttpResponse, Box<dyn std::error::Error>> {
    let body = json_body(request)?;
    let report = capture_git_summary(GitSummaryCaptureOptions {
        project_name: json_optional_string(&body, "project").or(base_project),
        start_dir: json_optional_string(&body, "path")
            .map(PathBuf::from)
            .unwrap_or(base_path),
        scope: json_arg_string(&body, "scope", ""),
        source: json_arg_string(&body, "source", "git"),
        limit: json_arg_usize(&body, "limit", 80),
        summarize: json_arg_bool(&body, "summarize", false),
    })?;
    json_response(&report)
}

fn http_capture_status(
    query: &HashMap<String, String>,
    base_project: Option<String>,
    base_path: PathBuf,
) -> Result<HttpResponse, Box<dyn std::error::Error>> {
    let report = get_capture_status(CaptureStatusOptions {
        project_name: query_project(query, base_project),
        start_dir: query_path(query, base_path),
        grafiki_home: None,
        scope: query_value(query, "scope", ""),
    })?;
    json_response(&report)
}

fn http_capture_events(
    query: &HashMap<String, String>,
    base_project: Option<String>,
    base_path: PathBuf,
) -> Result<HttpResponse, Box<dyn std::error::Error>> {
    let events = list_capture_events(ListCaptureEventsOptions {
        project_name: query_project(query, base_project),
        start_dir: query_path(query, base_path),
        grafiki_home: None,
        capture_id: query
            .get("capture")
            .or_else(|| query.get("capture_id"))
            .cloned(),
        source_type: query
            .get("type")
            .or_else(|| query.get("source_type"))
            .cloned(),
        scope: query_value(query, "scope", ""),
        limit: query_usize(query, "limit", 50),
    })?;
    json_response(&events)
}

fn http_capture_summarize(
    request: &HttpRequest,
    base_project: Option<String>,
    base_path: PathBuf,
) -> Result<HttpResponse, Box<dyn std::error::Error>> {
    let body = json_body(request)?;
    let report = propose_capture_candidates(ProposeCaptureCandidatesOptions {
        project_name: json_optional_string(&body, "project").or(base_project),
        start_dir: json_optional_string(&body, "path")
            .map(PathBuf::from)
            .unwrap_or(base_path),
        grafiki_home: None,
        capture_id: json_optional_string(&body, "capture")
            .or_else(|| json_optional_string(&body, "capture_id"))
            .or_else(|| json_optional_string(&body, "captureId")),
        scope: json_arg_string(&body, "scope", ""),
        limit: json_arg_usize(&body, "limit", 80),
    })?;
    json_response(&report)
}

fn http_search(
    query: &HashMap<String, String>,
    base_project: Option<String>,
    base_path: PathBuf,
) -> Result<HttpResponse, Box<dyn std::error::Error>> {
    let search_query = query
        .get("q")
        .or_else(|| query.get("query"))
        .ok_or("Missing required query parameter: q")?
        .to_owned();
    let report = search_memory(SearchMemoryOptions {
        project_name: query_project(query, base_project),
        start_dir: query_path(query, base_path),
        grafiki_home: None,
        query: search_query,
        record_type: query_value(query, "type", "all"),
        mode: CoreSearchMode::parse(&query_value(query, "mode", "keyword"))?,
        scope: query_value(query, "scope", ""),
        limit: query_usize(query, "limit", 10),
    })?;
    json_response(&report)
}

fn http_embeddings_status(
    query: &HashMap<String, String>,
    base_project: Option<String>,
    base_path: PathBuf,
) -> Result<HttpResponse, Box<dyn std::error::Error>> {
    let report = get_embedding_status(EmbeddingStatusOptions {
        project_name: query_project(query, base_project),
        start_dir: query_path(query, base_path),
        grafiki_home: None,
        scope: query_value(query, "scope", ""),
    })?;
    json_response(&report)
}

fn http_embeddings_process(
    request: &HttpRequest,
    base_project: Option<String>,
    base_path: PathBuf,
    rebuild: bool,
) -> Result<HttpResponse, Box<dyn std::error::Error>> {
    let body = json_body(request)?;
    let report = process_embedding_jobs(ProcessEmbeddingsOptions {
        project_name: json_optional_string(&body, "project").or(base_project),
        start_dir: json_optional_string(&body, "path")
            .map(PathBuf::from)
            .unwrap_or(base_path),
        grafiki_home: None,
        scope: json_arg_string(&body, "scope", ""),
        limit: json_arg_usize(&body, "limit", 100),
        rebuild,
    })?;
    json_response(&report)
}

fn http_report(
    query: &HashMap<String, String>,
    base_project: Option<String>,
    base_path: PathBuf,
) -> Result<HttpResponse, Box<dyn std::error::Error>> {
    let report = generate_report(ProjectReportOptions {
        project_name: query_project(query, base_project),
        start_dir: query_path(query, base_path),
        grafiki_home: None,
        scope: query_value(query, "scope", ""),
    })?;
    match query_value(query, "format", "json").as_str() {
        "plain" => Ok(HttpResponse::new(
            200,
            "text/plain; charset=utf-8",
            project_report_to_plain(&report),
        )),
        "md" | "markdown" => Ok(HttpResponse::new(
            200,
            "text/markdown; charset=utf-8",
            project_report_to_markdown(&report),
        )),
        _ => json_response(&report),
    }
}

fn http_export(
    query: &HashMap<String, String>,
    base_project: Option<String>,
    base_path: PathBuf,
) -> Result<HttpResponse, Box<dyn std::error::Error>> {
    let bundle = export_memory(ExportOptions {
        project_name: query_project(query, base_project),
        start_dir: query_path(query, base_path),
        grafiki_home: None,
        scope: query_value(query, "scope", ""),
    })?;
    match query_value(query, "format", "json").as_str() {
        "json" => json_response(&bundle),
        "md" | "markdown" => Ok(HttpResponse::new(
            200,
            "text/markdown; charset=utf-8",
            export_bundle_to_markdown(&bundle),
        )),
        "dot" => Ok(HttpResponse::new(
            200,
            "text/vnd.graphviz; charset=utf-8",
            export_bundle_to_dot(&bundle),
        )),
        "graphml" => Ok(HttpResponse::new(
            200,
            "application/xml; charset=utf-8",
            export_bundle_to_graphml(&bundle),
        )),
        "html" => Ok(HttpResponse::new(
            200,
            "text/html; charset=utf-8",
            export_bundle_to_html(&bundle),
        )),
        "wiki" => Ok(HttpResponse::json(
            400,
            serde_json::json!({ "error": "Wiki export writes a directory; use the CLI export command for wiki format." }),
        )),
        _ => Ok(HttpResponse::json(
            400,
            serde_json::json!({ "error": "Unsupported export format" }),
        )),
    }
}

fn http_graph(
    query: &HashMap<String, String>,
    base_project: Option<String>,
    base_path: PathBuf,
    entity_id: String,
) -> Result<HttpResponse, Box<dyn std::error::Error>> {
    let report = get_graph(GraphOptions {
        project_name: query_project(query, base_project),
        start_dir: query_path(query, base_path),
        grafiki_home: None,
        entity_id,
        depth: query_usize(query, "depth", 2),
    })?;
    json_response(&report)
}

fn http_events(
    query: &HashMap<String, String>,
    base_project: Option<String>,
    base_path: PathBuf,
) -> Result<HttpResponse, Box<dyn std::error::Error>> {
    let report = list_events(EventListOptions {
        project_name: query_project(query, base_project),
        start_dir: query_path(query, base_path),
        grafiki_home: None,
        scope: query_value(query, "scope", ""),
        since: query.get("since").cloned(),
        limit: query_usize(query, "limit", query_usize(query, "last", 20)),
    })?;
    json_response(&report)
}

fn http_agent_queries(
    query: &HashMap<String, String>,
    base_project: Option<String>,
    base_path: PathBuf,
) -> Result<HttpResponse, Box<dyn std::error::Error>> {
    let report = list_agent_queries(ListAgentQueriesOptions {
        project_name: query_project(query, base_project),
        start_dir: query_path(query, base_path),
        grafiki_home: None,
        scope: query_value(query, "scope", ""),
        limit: query_usize(query, "limit", query_usize(query, "last", 20)),
    })?;
    json_response(&report)
}

fn http_log(
    query: &HashMap<String, String>,
    base_project: Option<String>,
    base_path: PathBuf,
) -> Result<HttpResponse, Box<dyn std::error::Error>> {
    let report = list_sessions(SessionLogOptions {
        project_name: query_project(query, base_project),
        start_dir: query_path(query, base_path),
        grafiki_home: None,
        scope: query_value(query, "scope", ""),
        session_type: query.get("type").cloned(),
        limit: query_usize(query, "limit", query_usize(query, "last", 20)),
    })?;
    json_response(&report)
}

fn http_candidates_list(
    query: &HashMap<String, String>,
    base_project: Option<String>,
    base_path: PathBuf,
) -> Result<HttpResponse, Box<dyn std::error::Error>> {
    let candidates = list_candidates(ListCandidatesOptions {
        project_name: query_project(query, base_project),
        start_dir: query_path(query, base_path),
        grafiki_home: None,
        status: Some(query_value(query, "status", "pending")),
        scope: query_value(query, "scope", ""),
        limit: query_usize(query, "limit", 20),
    })?;
    json_response(&candidates)
}

fn http_candidate_propose(
    request: &HttpRequest,
    base_project: Option<String>,
    base_path: PathBuf,
) -> Result<HttpResponse, Box<dyn std::error::Error>> {
    let body = json_body(request)?;
    let report = propose_candidate_from_json(
        &body,
        query_project(&request.query, base_project),
        query_path(&request.query, base_path),
    )?;
    json_response(&report)
}

fn http_candidate_edit(
    request: &HttpRequest,
    base_project: Option<String>,
    base_path: PathBuf,
) -> Result<HttpResponse, Box<dyn std::error::Error>> {
    let body = json_body(request)?;
    let report = edit_candidate(EditCandidateOptions {
        project_name: query_project(&request.query, base_project),
        start_dir: query_path(&request.query, base_path),
        grafiki_home: None,
        id: json_required_string(&body, "id")?,
        record_type: json_optional_string(&body, "type")
            .or_else(|| json_optional_string(&body, "record_type"))
            .or_else(|| json_optional_string(&body, "recordType")),
        payload: body.get("payload").cloned(),
        scope: json_optional_string(&body, "scope"),
        confidence: json_optional_f64(&body, "confidence"),
        rationale: json_optional_string(&body, "rationale"),
    })?;
    json_response(&report)
}

fn http_candidate_approve(
    request: &HttpRequest,
    base_project: Option<String>,
    base_path: PathBuf,
) -> Result<HttpResponse, Box<dyn std::error::Error>> {
    let body = json_body(request)?;
    let report = approve_candidate(ApproveCandidateOptions {
        project_name: query_project(&request.query, base_project),
        start_dir: query_path(&request.query, base_path),
        grafiki_home: None,
        id: json_required_string(&body, "id")?,
    })?;
    json_response(&report)
}

fn http_candidate_bulk(
    request: &HttpRequest,
    base_project: Option<String>,
    base_path: PathBuf,
) -> Result<HttpResponse, Box<dyn std::error::Error>> {
    let body = json_body(request)?;
    let report = bulk_review_candidates(BulkCandidateReviewOptions {
        project_name: query_project(&request.query, base_project),
        start_dir: query_path(&request.query, base_path),
        grafiki_home: None,
        action: json_required_string(&body, "action")?,
        ids: json_string_array(&body, "ids")?,
        rationale: json_optional_string(&body, "rationale"),
    })?;
    json_response(&report)
}

fn http_candidate_reject(
    request: &HttpRequest,
    base_project: Option<String>,
    base_path: PathBuf,
) -> Result<HttpResponse, Box<dyn std::error::Error>> {
    let body = json_body(request)?;
    let report = reject_candidate(RejectCandidateOptions {
        project_name: query_project(&request.query, base_project),
        start_dir: query_path(&request.query, base_path),
        grafiki_home: None,
        id: json_required_string(&body, "id")?,
        rationale: json_optional_string(&body, "rationale"),
    })?;
    json_response(&report)
}

fn http_context(
    query: &HashMap<String, String>,
    base_project: Option<String>,
    base_path: PathBuf,
    key: String,
) -> Result<HttpResponse, Box<dyn std::error::Error>> {
    let document = get_context(GetContextOptions {
        project_name: query_project(query, base_project),
        start_dir: query_path(query, base_path),
        grafiki_home: None,
        key,
    })?;
    json_response(&document)
}

fn http_context_list(
    query: &HashMap<String, String>,
    base_project: Option<String>,
    base_path: PathBuf,
) -> Result<HttpResponse, Box<dyn std::error::Error>> {
    let summaries = list_context(ContextListOptions {
        project_name: query_project(query, base_project),
        start_dir: query_path(query, base_path),
        grafiki_home: None,
        category: query.get("category").cloned(),
        scope: query_value(query, "scope", ""),
    })?;
    json_response(&summaries)
}

fn http_memory_record(
    query: &HashMap<String, String>,
    base_project: Option<String>,
    base_path: PathBuf,
    record_type: String,
    id: String,
) -> Result<HttpResponse, Box<dyn std::error::Error>> {
    let detail = get_memory_record_detail(GetMemoryRecordOptions {
        project_name: query_project(query, base_project),
        start_dir: query_path(query, base_path),
        grafiki_home: None,
        record_type,
        id,
        scope: query_value(query, "scope", ""),
    })?;
    json_response(&detail)
}

fn http_memory_update(
    request: &HttpRequest,
    base_project: Option<String>,
    base_path: PathBuf,
) -> Result<HttpResponse, Box<dyn std::error::Error>> {
    let body = json_body(request)?;
    let response = update_memory_record_from_json(
        &body,
        query_project(&request.query, base_project),
        query_path(&request.query, base_path),
    )?;
    json_response(&response)
}

fn http_memory_delete(
    request: &HttpRequest,
    base_project: Option<String>,
    base_path: PathBuf,
) -> Result<HttpResponse, Box<dyn std::error::Error>> {
    let body = json_body(request)?;
    let response = delete_memory_record_from_json(
        &body,
        query_project(&request.query, base_project),
        query_path(&request.query, base_path),
    )?;
    json_response(&response)
}

fn update_memory_record_from_json(
    args: &serde_json::Value,
    project_name: Option<String>,
    start_dir: PathBuf,
) -> Result<RecordMutationResponse, Box<dyn std::error::Error>> {
    let record_type = json_record_type(args)?;
    let id = json_record_id(args)?;

    match record_type.as_str() {
        "context" => {
            let report = update_context(UpdateContextOptions {
                project_name,
                start_dir,
                grafiki_home: None,
                key: id,
                title: json_optional_string(args, "title"),
                category: json_optional_string(args, "category"),
                scope: json_optional_string(args, "scope"),
                content: json_optional_string(args, "content"),
            })?;
            Ok(RecordMutationResponse {
                record_type,
                id: report.key,
                title: report.title,
                scope: report.scope,
                message: "Context updated.".to_owned(),
            })
        }
        "state" => {
            let report = upsert_state(UpsertStateOptions {
                project_name,
                start_dir,
                grafiki_home: None,
                key: id,
                title: json_required_string(args, "title")?,
                status: json_arg_string(args, "status", "in-progress"),
                owner: json_optional_string(args, "owner"),
                details: json_optional_string(args, "details"),
                blockers: json_arg_vec(args, "blockers"),
                depends_on: json_arg_vec(args, "depends_on"),
                scope: json_arg_string(args, "scope", ""),
                priority: json_arg_string(args, "priority", "medium"),
            })?;
            Ok(RecordMutationResponse {
                record_type,
                id: report.key,
                title: report.title,
                scope: report.scope,
                message: "State item updated.".to_owned(),
            })
        }
        "decision" => {
            let report = update_decision(UpdateDecisionOptions {
                project_name,
                start_dir,
                grafiki_home: None,
                id,
                title: json_optional_string(args, "title"),
                reasoning: json_optional_string(args, "reasoning")
                    .or_else(|| json_optional_string(args, "content")),
                scope: json_optional_string(args, "scope"),
                status: json_optional_string(args, "status"),
            })?;
            Ok(RecordMutationResponse {
                record_type,
                id: report.id,
                title: report.title,
                scope: report.scope,
                message: "Decision updated.".to_owned(),
            })
        }
        "entity" => {
            let report = update_entity(UpdateEntityOptions {
                project_name,
                start_dir,
                grafiki_home: None,
                id,
                name: json_optional_string(args, "name")
                    .or_else(|| json_optional_string(args, "title")),
                entity_type: json_optional_string(args, "entity_type")
                    .or_else(|| json_optional_string(args, "entityType")),
                scope: json_optional_string(args, "scope"),
            })?;
            Ok(RecordMutationResponse {
                record_type,
                id: report.id,
                title: report.name,
                scope: report.scope,
                message: "Entity updated.".to_owned(),
            })
        }
        "observation" => {
            let report = update_observation(UpdateObservationOptions {
                project_name,
                start_dir,
                grafiki_home: None,
                id,
                content: json_optional_string(args, "content"),
                category: json_optional_string(args, "category"),
            })?;
            Ok(RecordMutationResponse {
                record_type,
                id: report.id,
                title: report.entity_name,
                scope: report.scope,
                message: "Observation updated.".to_owned(),
            })
        }
        "relation" => {
            let report = update_relation(UpdateRelationOptions {
                project_name,
                start_dir,
                grafiki_home: None,
                id,
                relation: json_optional_string(args, "relation"),
                weight: json_optional_f64(args, "weight"),
                confidence: json_optional_f64(args, "confidence"),
                source_type: json_optional_string(args, "source_type")
                    .or_else(|| json_optional_string(args, "sourceType")),
                source: json_optional_string(args, "source"),
            })?;
            Ok(RecordMutationResponse {
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
                project_name,
                start_dir,
                grafiki_home: None,
                id,
                session_type: json_optional_string(args, "session_type")
                    .or_else(|| json_optional_string(args, "sessionType")),
                status: json_optional_string(args, "status"),
                scope: json_optional_string(args, "scope"),
                goal: json_optional_string(args, "goal"),
                summary: json_optional_string(args, "summary"),
                accomplishments: json_optional_vec(args, "accomplishments"),
                remaining: json_optional_vec(args, "remaining"),
                files_changed: json_optional_vec(args, "files")
                    .or_else(|| json_optional_vec(args, "files_changed"))
                    .or_else(|| json_optional_vec(args, "filesChanged")),
            })?;
            Ok(RecordMutationResponse {
                record_type,
                id: report.id,
                title: report.goal.unwrap_or_else(|| "Session".to_owned()),
                scope: report.scope,
                message: "Session updated.".to_owned(),
            })
        }
        other => Err(format!("Unsupported memory record type: {other}").into()),
    }
}

fn delete_memory_record_from_json(
    args: &serde_json::Value,
    project_name: Option<String>,
    start_dir: PathBuf,
) -> Result<RecordMutationResponse, Box<dyn std::error::Error>> {
    let record_type = json_record_type(args)?;
    let id = json_record_id(args)?;

    match record_type.as_str() {
        "context" => {
            let report = delete_context(DeleteContextOptions {
                project_name,
                start_dir,
                grafiki_home: None,
                key: id,
            })?;
            Ok(RecordMutationResponse {
                record_type,
                id: report.key,
                title: report.title,
                scope: report.scope,
                message: "Context deleted.".to_owned(),
            })
        }
        "state" => {
            let report = delete_state(DeleteStateOptions {
                project_name,
                start_dir,
                grafiki_home: None,
                key: id,
            })?;
            Ok(RecordMutationResponse {
                record_type,
                id: report.key,
                title: report.title,
                scope: report.scope,
                message: "State item deleted.".to_owned(),
            })
        }
        "decision" => {
            let report = delete_decision(DeleteDecisionOptions {
                project_name,
                start_dir,
                grafiki_home: None,
                id,
            })?;
            Ok(RecordMutationResponse {
                record_type,
                id: report.id,
                title: report.title,
                scope: report.scope,
                message: "Decision deleted.".to_owned(),
            })
        }
        "entity" => {
            let report = delete_entity(DeleteEntityOptions {
                project_name,
                start_dir,
                grafiki_home: None,
                id,
            })?;
            Ok(RecordMutationResponse {
                record_type,
                id: report.id,
                title: report.name,
                scope: report.scope,
                message: "Entity deleted.".to_owned(),
            })
        }
        "observation" => {
            let report = delete_observation(DeleteObservationOptions {
                project_name,
                start_dir,
                grafiki_home: None,
                id,
            })?;
            Ok(RecordMutationResponse {
                record_type,
                id: report.id,
                title: report.entity_name,
                scope: report.scope,
                message: "Observation invalidated.".to_owned(),
            })
        }
        "relation" => {
            let report = delete_relation(DeleteRelationOptions {
                project_name,
                start_dir,
                grafiki_home: None,
                id,
            })?;
            Ok(RecordMutationResponse {
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
        "session" => Err("Session records can be updated but not deleted.".into()),
        other => Err(format!("Unsupported memory record type: {other}").into()),
    }
}

fn propose_candidate_from_json(
    args: &serde_json::Value,
    project_name: Option<String>,
    start_dir: PathBuf,
) -> Result<grafiki_core::CandidateMutationReport, Box<dyn std::error::Error>> {
    let payload = args
        .get("payload")
        .filter(|value| value.is_object())
        .cloned()
        .ok_or("Missing required object argument: payload")?;
    propose_candidate(ProposeCandidateOptions {
        project_name,
        start_dir,
        grafiki_home: None,
        source_type: json_arg_string(args, "source_type", "agent"),
        source: json_optional_string(args, "source"),
        record_type: json_optional_string(args, "record_type")
            .or_else(|| json_optional_string(args, "recordType"))
            .or_else(|| json_optional_string(args, "type"))
            .filter(|value| !value.trim().is_empty())
            .ok_or("Missing required argument: type")?,
        payload,
        scope: json_arg_string(args, "scope", ""),
        confidence: json_optional_f64(args, "confidence").unwrap_or(0.5),
        rationale: json_optional_string(args, "rationale"),
        evidence: json_evidence_inputs(args),
    })
    .map_err(Into::into)
}

fn http_start(
    request: &HttpRequest,
    base_project: Option<String>,
    base_path: PathBuf,
) -> Result<HttpResponse, Box<dyn std::error::Error>> {
    let body = json_body(request)?;
    let report = start_session(StartSessionOptions {
        project_name: query_project(&request.query, base_project),
        start_dir: query_path(&request.query, base_path),
        grafiki_home: None,
        session_type: json_arg_string(&body, "type", "codex"),
        goal: json_required_string(&body, "goal")?,
        scope: json_arg_string(&body, "scope", ""),
    })?;
    json_response(&report)
}

fn http_end(
    request: &HttpRequest,
    base_project: Option<String>,
    base_path: PathBuf,
) -> Result<HttpResponse, Box<dyn std::error::Error>> {
    let body = json_body(request)?;
    let report = end_session(EndSessionOptions {
        project_name: query_project(&request.query, base_project),
        start_dir: query_path(&request.query, base_path),
        grafiki_home: None,
        session_id: json_optional_string(&body, "session"),
        status: json_arg_string(&body, "status", "completed"),
        summary: json_optional_string(&body, "summary"),
        accomplishments: json_arg_vec(&body, "accomplishments"),
        remaining: json_arg_vec(&body, "remaining"),
        files_changed: json_arg_vec(&body, "files"),
    })?;
    json_response(&report)
}

fn http_handoff(
    request: &HttpRequest,
    base_project: Option<String>,
    base_path: PathBuf,
) -> Result<HttpResponse, Box<dyn std::error::Error>> {
    let body = json_body(request)?;
    let report = handoff_session(HandoffOptions {
        project_name: query_project(&request.query, base_project),
        start_dir: query_path(&request.query, base_path),
        grafiki_home: None,
        session_id: json_optional_string(&body, "session")
            .or_else(|| json_optional_string(&body, "session_id")),
    })?;
    json_response(&report)
}

fn http_save(
    request: &HttpRequest,
    base_project: Option<String>,
    base_path: PathBuf,
) -> Result<HttpResponse, Box<dyn std::error::Error>> {
    let body = json_body(request)?;
    let report = save_entity(SaveEntityOptions {
        project_name: query_project(&request.query, base_project),
        start_dir: query_path(&request.query, base_path),
        grafiki_home: None,
        name: json_required_string(&body, "name")?,
        entity_type: json_arg_string(&body, "entity_type", "concept"),
        observe: json_optional_string(&body, "observe"),
        category: json_arg_string(&body, "category", "general"),
        scope: json_arg_string(&body, "scope", ""),
        relate: json_optional_string(&body, "relate"),
    })?;
    json_response(&report)
}

fn http_decide(
    request: &HttpRequest,
    base_project: Option<String>,
    base_path: PathBuf,
) -> Result<HttpResponse, Box<dyn std::error::Error>> {
    let body = json_body(request)?;
    let report = log_decision(LogDecisionOptions {
        project_name: query_project(&request.query, base_project),
        start_dir: query_path(&request.query, base_path),
        grafiki_home: None,
        title: json_required_string(&body, "title")?,
        reasoning: json_optional_string(&body, "reasoning"),
        alternatives: json_arg_vec(&body, "alternatives"),
        tags: json_arg_vec(&body, "tags"),
        scope: json_arg_string(&body, "scope", ""),
        supersedes: json_optional_string(&body, "supersedes"),
    })?;
    json_response(&report)
}

fn http_state_set(
    request: &HttpRequest,
    base_project: Option<String>,
    base_path: PathBuf,
) -> Result<HttpResponse, Box<dyn std::error::Error>> {
    let body = json_body(request)?;
    let report = upsert_state(UpsertStateOptions {
        project_name: query_project(&request.query, base_project),
        start_dir: query_path(&request.query, base_path),
        grafiki_home: None,
        key: json_required_string(&body, "key")?,
        title: json_required_string(&body, "title")?,
        status: json_arg_string(&body, "status", "in-progress"),
        owner: json_optional_string(&body, "owner"),
        details: json_optional_string(&body, "details"),
        blockers: json_arg_vec(&body, "blockers"),
        depends_on: json_arg_vec(&body, "depends_on"),
        scope: json_arg_string(&body, "scope", ""),
        priority: json_arg_string(&body, "priority", "medium"),
    })?;
    json_response(&report)
}

fn http_context_add(
    request: &HttpRequest,
    base_project: Option<String>,
    base_path: PathBuf,
) -> Result<HttpResponse, Box<dyn std::error::Error>> {
    let body = json_body(request)?;
    let report = add_context(AddContextOptions {
        project_name: query_project(&request.query, base_project),
        start_dir: query_path(&request.query, base_path),
        grafiki_home: None,
        key: json_required_string(&body, "key")?,
        title: json_required_string(&body, "title")?,
        category: json_required_string(&body, "category")?,
        scope: json_arg_string(&body, "scope", ""),
        content: json_required_string(&body, "content")?,
    })?;
    json_response(&report)
}

fn http_context_update(
    request: &HttpRequest,
    base_project: Option<String>,
    base_path: PathBuf,
) -> Result<HttpResponse, Box<dyn std::error::Error>> {
    let body = json_body(request)?;
    let report = update_context(UpdateContextOptions {
        project_name: query_project(&request.query, base_project),
        start_dir: query_path(&request.query, base_path),
        grafiki_home: None,
        key: json_required_string(&body, "key")?,
        title: json_optional_string(&body, "title"),
        category: json_optional_string(&body, "category"),
        scope: json_optional_string(&body, "scope"),
        content: json_optional_string(&body, "content"),
    })?;
    json_response(&report)
}

fn http_context_delete(
    request: &HttpRequest,
    base_project: Option<String>,
    base_path: PathBuf,
) -> Result<HttpResponse, Box<dyn std::error::Error>> {
    let body = json_body(request)?;
    let report = delete_context(DeleteContextOptions {
        project_name: query_project(&request.query, base_project),
        start_dir: query_path(&request.query, base_path),
        grafiki_home: None,
        key: json_required_string(&body, "key")?,
    })?;
    json_response(&report)
}

fn http_import(
    request: &HttpRequest,
    base_project: Option<String>,
    base_path: PathBuf,
) -> Result<HttpResponse, Box<dyn std::error::Error>> {
    let bundle: grafiki_core::ExportBundle = serde_json::from_str(&request.body)?;
    let report = import_memory(ImportOptions {
        project_name: query_project(&request.query, base_project),
        start_dir: query_path(&request.query, base_path),
        grafiki_home: None,
        bundle,
    })?;
    json_response(&report)
}

fn json_body(request: &HttpRequest) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    if request.body.trim().is_empty() {
        return Ok(serde_json::json!({}));
    }
    Ok(serde_json::from_str(&request.body)?)
}

fn json_response<T: serde::Serialize>(
    value: &T,
) -> Result<HttpResponse, Box<dyn std::error::Error>> {
    Ok(HttpResponse::new(
        200,
        "application/json; charset=utf-8",
        serde_json::to_string_pretty(value)?,
    ))
}

fn query_project(query: &HashMap<String, String>, base_project: Option<String>) -> Option<String> {
    query.get("project").cloned().or(base_project)
}

fn query_path(query: &HashMap<String, String>, base_path: PathBuf) -> PathBuf {
    query.get("path").map(PathBuf::from).unwrap_or(base_path)
}

fn query_value(query: &HashMap<String, String>, key: &str, default: &str) -> String {
    query
        .get(key)
        .filter(|value| !value.trim().is_empty())
        .cloned()
        .unwrap_or_else(|| default.to_owned())
}

fn query_usize(query: &HashMap<String, String>, key: &str, default: usize) -> usize {
    query
        .get(key)
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(default)
}

fn write_http_response(
    stream: &mut TcpStream,
    response: HttpResponse,
) -> Result<(), Box<dyn std::error::Error>> {
    let body = response.body.into_bytes();
    write!(
        stream,
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        response.status,
        http_status_text(response.status),
        response.content_type,
        body.len()
    )?;
    stream.write_all(&body)?;
    Ok(())
}

fn http_status_text(status: u16) -> &'static str {
    match status {
        200 => "OK",
        400 => "Bad Request",
        401 => "Unauthorized",
        404 => "Not Found",
        405 => "Method Not Allowed",
        500 => "Internal Server Error",
        _ => "OK",
    }
}

fn percent_decode(value: &str) -> String {
    let mut bytes = Vec::with_capacity(value.len());
    let raw = value.as_bytes();
    let mut index = 0;
    while index < raw.len() {
        match raw[index] {
            b'+' => {
                bytes.push(b' ');
                index += 1;
            }
            b'%' if index + 2 < raw.len() => {
                let hex = &value[index + 1..index + 3];
                if let Ok(byte) = u8::from_str_radix(hex, 16) {
                    bytes.push(byte);
                    index += 3;
                } else {
                    bytes.push(raw[index]);
                    index += 1;
                }
            }
            byte => {
                bytes.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

fn run_mcp(project: Option<String>, path: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let message: serde_json::Value = match serde_json::from_str(&line) {
            Ok(message) => message,
            Err(error) => {
                writeln!(
                    stdout,
                    "{}",
                    mcp_error(serde_json::Value::Null, -32700, &error.to_string())
                )?;
                stdout.flush()?;
                continue;
            }
        };

        let response = match message {
            // JSON-RPC 2.0 batch: process each member, return an array of the
            // responses that warrant one (notifications produce none).
            serde_json::Value::Array(items) => {
                if items.is_empty() {
                    Some(mcp_error(
                        serde_json::Value::Null,
                        -32600,
                        "Invalid Request: empty batch",
                    ))
                } else {
                    let mut responses = Vec::new();
                    for item in items {
                        if let Some(resp) = handle_mcp_message(item, project.clone(), path.clone())?
                        {
                            responses.push(resp);
                        }
                    }
                    if responses.is_empty() {
                        None
                    } else {
                        Some(serde_json::Value::Array(responses))
                    }
                }
            }
            other => handle_mcp_message(other, project.clone(), path.clone())?,
        };

        if let Some(response) = response {
            writeln!(stdout, "{response}")?;
            stdout.flush()?;
        }
    }

    Ok(())
}

fn handle_mcp_message(
    message: serde_json::Value,
    project: Option<String>,
    path: PathBuf,
) -> Result<Option<serde_json::Value>, Box<dyn std::error::Error>> {
    let id = message.get("id").cloned();
    let method = message
        .get("method")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");

    if id.is_none() {
        return Ok(None);
    }
    let id = id.unwrap_or(serde_json::Value::Null);

    let response = match method {
        "initialize" => mcp_result(
            id,
            serde_json::json!({
                "protocolVersion": "2024-11-05",
                "capabilities": { "tools": {} },
                "serverInfo": {
                    "name": "grafiki",
                    "version": env!("CARGO_PKG_VERSION")
                }
            }),
        ),
        "ping" => mcp_result(id, serde_json::json!({})),
        "tools/list" => mcp_result(id, serde_json::json!({ "tools": mcp_tools() })),
        "tools/call" => match handle_mcp_tool_call(&message, project, path) {
            Ok(result) => mcp_result(id, result),
            // Per MCP spec, tool execution failures are reported as a successful
            // result with isError=true (so the agent sees the message), not as a
            // JSON-RPC protocol error.
            Err(error) => mcp_result(
                id,
                serde_json::json!({
                    "content": [{ "type": "text", "text": format!("Error: {error}") }],
                    "isError": true
                }),
            ),
        },
        _ => mcp_error(id, -32601, "Method not found"),
    };

    Ok(Some(response))
}

fn handle_mcp_tool_call(
    message: &serde_json::Value,
    project: Option<String>,
    path: PathBuf,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let params = message
        .get("params")
        .ok_or("Missing tools/call params object")?;
    let name = params
        .get("name")
        .and_then(serde_json::Value::as_str)
        .ok_or("Missing tool name")?;
    let args = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}));

    match name {
        "grafiki_start" => {
            let report = start_session(StartSessionOptions {
                project_name: project,
                start_dir: path,
                grafiki_home: None,
                session_type: json_arg_string(&args, "type", "codex"),
                goal: json_required_string(&args, "goal")?,
                scope: json_arg_string(&args, "scope", ""),
            })?;
            mcp_json_tool_result(&report)
        }
        "grafiki_end" => {
            let report = end_session(EndSessionOptions {
                project_name: project,
                start_dir: path,
                grafiki_home: None,
                session_id: json_optional_string(&args, "session"),
                status: json_arg_string(&args, "status", "completed"),
                summary: json_optional_string(&args, "summary"),
                accomplishments: json_arg_vec(&args, "accomplishments"),
                remaining: json_arg_vec(&args, "remaining"),
                files_changed: json_arg_vec(&args, "files"),
            })?;
            mcp_json_tool_result(&report)
        }
        "grafiki_handoff" => {
            let report = handoff_session(HandoffOptions {
                project_name: project,
                start_dir: path,
                grafiki_home: None,
                session_id: json_optional_string(&args, "session")
                    .or_else(|| json_optional_string(&args, "session_id")),
            })?;
            mcp_json_tool_result(&report)
        }
        "grafiki_status" => {
            let report = get_status(StatusOptions {
                project_name: project,
                start_dir: path,
                grafiki_home: None,
                scope: json_arg_string(&args, "scope", ""),
            })?;
            mcp_json_tool_result(&report)
        }
        "grafiki_ask" => {
            let briefing = ask_memory(AskMemoryOptions {
                project_name: project,
                start_dir: path,
                grafiki_home: None,
                question: json_required_string(&args, "question")?,
                scope: json_arg_string(&args, "scope", ""),
                limit: json_arg_usize(&args, "limit", 8),
                agent: json_optional_string(&args, "agent").or_else(|| Some("mcp".to_owned())),
            })?;
            mcp_json_tool_result(&briefing)
        }
        "grafiki_agent_activity" => {
            let queries = list_agent_queries(ListAgentQueriesOptions {
                project_name: project,
                start_dir: path,
                grafiki_home: None,
                scope: json_arg_string(&args, "scope", ""),
                limit: json_arg_usize(&args, "limit", 20),
            })?;
            mcp_json_tool_result(&queries)
        }
        "grafiki_auto_capture" => {
            let report = auto_capture(
                project,
                path,
                json_arg_string(&args, "scope", ""),
                json_optional_string(&args, "source"),
                json_arg_usize(&args, "limit", 25),
            )?;
            mcp_json_tool_result(&report)
        }
        "grafiki_capture_start" => {
            let report = start_capture_session(StartCaptureOptions {
                project_name: project,
                start_dir: path,
                grafiki_home: None,
                scope: json_arg_string(&args, "scope", ""),
                source_app: json_optional_string(&args, "source_app")
                    .or_else(|| json_optional_string(&args, "sourceApp")),
                consent_profile: json_optional_string(&args, "consent_profile")
                    .or_else(|| json_optional_string(&args, "consentProfile")),
                redaction_profile: json_optional_string(&args, "redaction_profile")
                    .or_else(|| json_optional_string(&args, "redactionProfile")),
            })?;
            mcp_json_tool_result(&report)
        }
        "grafiki_capture_stop" => {
            let report = stop_capture_session(StopCaptureOptions {
                project_name: project,
                start_dir: path,
                grafiki_home: None,
                capture_id: json_required_string(&args, "id")
                    .or_else(|_| json_required_string(&args, "capture"))?,
            })?;
            mcp_json_tool_result(&report)
        }
        "grafiki_capture_ingest" => {
            let report = ingest_capture_event(IngestCaptureEventOptions {
                project_name: project,
                start_dir: path,
                grafiki_home: None,
                capture_id: json_optional_string(&args, "capture")
                    .or_else(|| json_optional_string(&args, "capture_id"))
                    .or_else(|| json_optional_string(&args, "captureId")),
                scope: json_arg_string(&args, "scope", ""),
                source_type: json_optional_string(&args, "source_type")
                    .or_else(|| json_optional_string(&args, "sourceType"))
                    .or_else(|| json_optional_string(&args, "type"))
                    .ok_or("Missing required argument: source_type")?,
                source: json_optional_string(&args, "source"),
                title: json_optional_string(&args, "title"),
                text: json_optional_string(&args, "text"),
                payload: args.get("payload").cloned(),
                metadata: args.get("metadata").cloned(),
                privacy_level: json_optional_string(&args, "privacy")
                    .or_else(|| json_optional_string(&args, "privacy_level"))
                    .or_else(|| json_optional_string(&args, "privacyLevel")),
                redacted: json_arg_bool(&args, "redacted", false),
                captured_at: json_optional_string(&args, "captured_at")
                    .or_else(|| json_optional_string(&args, "capturedAt")),
            })?;
            mcp_json_tool_result(&report)
        }
        "grafiki_capture_import_transcripts" => {
            ensure_capture_source_enabled(project.clone(), &path, "transcripts")?;
            let report = import_agent_transcripts(ImportAgentTranscriptsOptions {
                project_name: project,
                start_dir: path,
                grafiki_home: None,
                agent: json_required_string(&args, "agent")?,
                input: json_optional_string(&args, "input").map(PathBuf::from),
                scope: json_arg_string(&args, "scope", ""),
                limit: json_arg_usize(&args, "limit", 200),
                summarize: json_arg_bool(&args, "summarize", false),
            })?;
            mcp_json_tool_result(&report)
        }
        "grafiki_capture_config" => {
            let report = load_capture_config(CaptureConfigOptions {
                project_name: project,
                start_dir: path,
                grafiki_home: None,
            })?;
            mcp_json_tool_result(&report)
        }
        "grafiki_capture_config_set" => {
            let report = update_capture_config(UpdateCaptureConfigOptions {
                project_name: project,
                start_dir: path,
                grafiki_home: None,
                sources: CaptureSourceUpdates {
                    git: json_optional_bool(&args, "git"),
                    transcripts: json_optional_bool(&args, "transcripts"),
                    terminal: json_optional_bool(&args, "terminal"),
                    files: json_optional_bool(&args, "files"),
                    ide: json_optional_bool(&args, "ide"),
                    screen: json_optional_bool(&args, "screen"),
                    browser: json_optional_bool(&args, "browser"),
                    audio: json_optional_bool(&args, "audio"),
                    system: json_optional_bool(&args, "system"),
                },
                add_blocked_paths: json_optional_vec(&args, "add_blocked_paths")
                    .or_else(|| json_optional_vec(&args, "add_blocked_path"))
                    .or_else(|| json_optional_vec(&args, "blocked_paths"))
                    .unwrap_or_default(),
                remove_blocked_paths: json_optional_vec(&args, "remove_blocked_paths")
                    .or_else(|| json_optional_vec(&args, "remove_blocked_path"))
                    .unwrap_or_default(),
                add_blocked_apps: json_optional_vec(&args, "add_blocked_apps")
                    .or_else(|| json_optional_vec(&args, "add_blocked_app"))
                    .or_else(|| json_optional_vec(&args, "blocked_apps"))
                    .unwrap_or_default(),
                remove_blocked_apps: json_optional_vec(&args, "remove_blocked_apps")
                    .or_else(|| json_optional_vec(&args, "remove_blocked_app"))
                    .unwrap_or_default(),
                redaction_profile: json_optional_string(&args, "redaction_profile")
                    .or_else(|| json_optional_string(&args, "redactionProfile")),
                terminal_output: json_optional_string(&args, "terminal_output")
                    .or_else(|| json_optional_string(&args, "terminalOutput")),
                screen_policy: json_optional_string(&args, "screen_policy")
                    .or_else(|| json_optional_string(&args, "screenPolicy")),
                browser_policy: json_optional_string(&args, "browser_policy")
                    .or_else(|| json_optional_string(&args, "browserPolicy")),
            })?;
            mcp_json_tool_result(&report)
        }
        "grafiki_capture_terminal_command" => {
            let report = capture_terminal_command(TerminalCommandCaptureOptions {
                project_name: project,
                start_dir: path,
                scope: json_arg_string(&args, "scope", ""),
                command: json_optional_string(&args, "command")
                    .or_else(|| json_optional_string(&args, "cmd"))
                    .ok_or("Missing required argument: command")?,
                cwd: json_optional_string(&args, "cwd").map(PathBuf::from),
                exit_code: json_optional_i32(&args, "exit_code")
                    .or_else(|| json_optional_i32(&args, "exitCode")),
                duration_ms: json_optional_u64(&args, "duration_ms")
                    .or_else(|| json_optional_u64(&args, "durationMs")),
                shell: json_optional_string(&args, "shell"),
            })?;
            mcp_json_tool_result(&report)
        }
        "grafiki_capture_watch_files" => {
            let report = watch_files_capture(FileWatchCaptureOptions {
                project_name: project,
                start_dir: path,
                scope: json_arg_string(&args, "scope", ""),
                since_seconds: json_optional_u64(&args, "since_seconds")
                    .or_else(|| json_optional_u64(&args, "sinceSeconds"))
                    .unwrap_or(300),
                duration_seconds: json_optional_u64(&args, "duration_seconds")
                    .or_else(|| json_optional_u64(&args, "durationSeconds"))
                    .unwrap_or(0),
                interval_ms: json_optional_u64(&args, "interval_ms")
                    .or_else(|| json_optional_u64(&args, "intervalMs"))
                    .unwrap_or(1000),
                limit: json_arg_usize(&args, "limit", 100),
                summarize: json_arg_bool(&args, "summarize", false),
            })?;
            mcp_json_tool_result(&report)
        }
        "grafiki_capture_git_summary" => {
            let report = capture_git_summary(GitSummaryCaptureOptions {
                project_name: project,
                start_dir: path,
                scope: json_arg_string(&args, "scope", ""),
                source: json_arg_string(&args, "source", "git"),
                limit: json_arg_usize(&args, "limit", 80),
                summarize: json_arg_bool(&args, "summarize", false),
            })?;
            mcp_json_tool_result(&report)
        }
        "grafiki_capture_status" => {
            let report = get_capture_status(CaptureStatusOptions {
                project_name: project,
                start_dir: path,
                grafiki_home: None,
                scope: json_arg_string(&args, "scope", ""),
            })?;
            mcp_json_tool_result(&report)
        }
        "grafiki_capture_events" => {
            let events = list_capture_events(ListCaptureEventsOptions {
                project_name: project,
                start_dir: path,
                grafiki_home: None,
                capture_id: json_optional_string(&args, "capture")
                    .or_else(|| json_optional_string(&args, "capture_id"))
                    .or_else(|| json_optional_string(&args, "captureId")),
                source_type: json_optional_string(&args, "source_type")
                    .or_else(|| json_optional_string(&args, "sourceType"))
                    .or_else(|| json_optional_string(&args, "type")),
                scope: json_arg_string(&args, "scope", ""),
                limit: json_arg_usize(&args, "limit", 50),
            })?;
            mcp_json_tool_result(&events)
        }
        "grafiki_capture_summarize" => {
            let report = propose_capture_candidates(ProposeCaptureCandidatesOptions {
                project_name: project,
                start_dir: path,
                grafiki_home: None,
                capture_id: json_optional_string(&args, "capture")
                    .or_else(|| json_optional_string(&args, "capture_id"))
                    .or_else(|| json_optional_string(&args, "captureId")),
                scope: json_arg_string(&args, "scope", ""),
                limit: json_arg_usize(&args, "limit", 80),
            })?;
            mcp_json_tool_result(&report)
        }
        "grafiki_search" => {
            let report = search_memory(SearchMemoryOptions {
                project_name: project,
                start_dir: path,
                grafiki_home: None,
                query: json_required_string(&args, "query")?,
                record_type: json_arg_string(&args, "type", "all"),
                mode: CoreSearchMode::parse(&json_arg_string(&args, "mode", "keyword"))?,
                scope: json_arg_string(&args, "scope", ""),
                limit: json_arg_usize(&args, "limit", 10),
            })?;
            mcp_json_tool_result(&report)
        }
        "grafiki_candidate_propose" => {
            let report = propose_candidate_from_json(&args, project, path)?;
            mcp_json_tool_result(&report)
        }
        "grafiki_candidate_edit" => {
            let report = edit_candidate(EditCandidateOptions {
                project_name: project,
                start_dir: path,
                grafiki_home: None,
                id: json_required_string(&args, "id")?,
                record_type: json_optional_string(&args, "type")
                    .or_else(|| json_optional_string(&args, "record_type"))
                    .or_else(|| json_optional_string(&args, "recordType")),
                payload: args.get("payload").cloned(),
                scope: json_optional_string(&args, "scope"),
                confidence: json_optional_f64(&args, "confidence"),
                rationale: json_optional_string(&args, "rationale"),
            })?;
            mcp_json_tool_result(&report)
        }
        "grafiki_candidate_list" => {
            let candidates = list_candidates(ListCandidatesOptions {
                project_name: project,
                start_dir: path,
                grafiki_home: None,
                status: Some(json_arg_string(&args, "status", "pending")),
                scope: json_arg_string(&args, "scope", ""),
                limit: json_arg_usize(&args, "limit", 20),
            })?;
            mcp_json_tool_result(&candidates)
        }
        "grafiki_candidate_bulk" => {
            let report = bulk_review_candidates(BulkCandidateReviewOptions {
                project_name: project,
                start_dir: path,
                grafiki_home: None,
                action: json_required_string(&args, "action")?,
                ids: json_string_array(&args, "ids")?,
                rationale: json_optional_string(&args, "rationale"),
            })?;
            mcp_json_tool_result(&report)
        }
        "grafiki_candidate_approve" => {
            let report = approve_candidate(ApproveCandidateOptions {
                project_name: project,
                start_dir: path,
                grafiki_home: None,
                id: json_required_string(&args, "id")?,
            })?;
            mcp_json_tool_result(&report)
        }
        "grafiki_candidate_reject" => {
            let report = reject_candidate(RejectCandidateOptions {
                project_name: project,
                start_dir: path,
                grafiki_home: None,
                id: json_required_string(&args, "id")?,
                rationale: json_optional_string(&args, "rationale"),
            })?;
            mcp_json_tool_result(&report)
        }
        "grafiki_record" => {
            let detail = get_memory_record_detail(GetMemoryRecordOptions {
                project_name: project,
                start_dir: path,
                grafiki_home: None,
                record_type: json_arg_string(&args, "type", ""),
                id: json_required_string(&args, "id")?,
                scope: json_arg_string(&args, "scope", ""),
            })?;
            mcp_json_tool_result(&detail)
        }
        "grafiki_update_record" => {
            let response = update_memory_record_from_json(&args, project, path)?;
            mcp_json_tool_result(&response)
        }
        "grafiki_delete_record" => {
            let response = delete_memory_record_from_json(&args, project, path)?;
            mcp_json_tool_result(&response)
        }
        "grafiki_save" => {
            let report = save_entity(SaveEntityOptions {
                project_name: project,
                start_dir: path,
                grafiki_home: None,
                name: json_required_string(&args, "name")?,
                entity_type: json_arg_string(&args, "entity_type", "concept"),
                observe: json_optional_string(&args, "observe"),
                category: json_arg_string(&args, "category", "general"),
                scope: json_arg_string(&args, "scope", ""),
                relate: json_optional_string(&args, "relate"),
            })?;
            mcp_json_tool_result(&report)
        }
        "grafiki_decide" => {
            let report = log_decision(LogDecisionOptions {
                project_name: project,
                start_dir: path,
                grafiki_home: None,
                title: json_required_string(&args, "title")?,
                reasoning: json_optional_string(&args, "reasoning"),
                alternatives: json_arg_vec(&args, "alternatives"),
                tags: json_arg_vec(&args, "tags"),
                scope: json_arg_string(&args, "scope", ""),
                supersedes: json_optional_string(&args, "supersedes"),
            })?;
            mcp_json_tool_result(&report)
        }
        "grafiki_state_set" => {
            let report = upsert_state(UpsertStateOptions {
                project_name: project,
                start_dir: path,
                grafiki_home: None,
                key: json_required_string(&args, "key")?,
                title: json_required_string(&args, "title")?,
                status: json_arg_string(&args, "status", "in-progress"),
                owner: json_optional_string(&args, "owner"),
                details: json_optional_string(&args, "details"),
                blockers: json_arg_vec(&args, "blockers"),
                depends_on: json_arg_vec(&args, "depends_on"),
                scope: json_arg_string(&args, "scope", ""),
                priority: json_arg_string(&args, "priority", "medium"),
            })?;
            mcp_json_tool_result(&report)
        }
        "grafiki_embeddings_status" => {
            let report = get_embedding_status(EmbeddingStatusOptions {
                project_name: project,
                start_dir: path,
                grafiki_home: None,
                scope: json_arg_string(&args, "scope", ""),
            })?;
            mcp_json_tool_result(&report)
        }
        "grafiki_embeddings_process" => {
            let report = process_embedding_jobs(ProcessEmbeddingsOptions {
                project_name: project,
                start_dir: path,
                grafiki_home: None,
                scope: json_arg_string(&args, "scope", ""),
                limit: json_arg_usize(&args, "limit", 100),
                rebuild: json_arg_bool(&args, "rebuild", false),
            })?;
            mcp_json_tool_result(&report)
        }
        "grafiki_report" => {
            let report = generate_report(ProjectReportOptions {
                project_name: project,
                start_dir: path,
                grafiki_home: None,
                scope: json_arg_string(&args, "scope", ""),
            })?;
            mcp_json_tool_result(&report)
        }
        "grafiki_graph" => {
            let report = get_graph(GraphOptions {
                project_name: project,
                start_dir: path,
                grafiki_home: None,
                entity_id: json_required_string(&args, "entity_id")?,
                depth: json_arg_usize(&args, "depth", 2),
            })?;
            mcp_json_tool_result(&report)
        }
        "grafiki_export" => {
            let bundle = export_memory(ExportOptions {
                project_name: project,
                start_dir: path,
                grafiki_home: None,
                scope: json_arg_string(&args, "scope", ""),
            })?;
            let text = match json_arg_string(&args, "format", "json").as_str() {
                "json" => serde_json::to_string_pretty(&bundle)?,
                "md" | "markdown" => export_bundle_to_markdown(&bundle),
                "dot" => export_bundle_to_dot(&bundle),
                "graphml" => export_bundle_to_graphml(&bundle),
                "html" => export_bundle_to_html(&bundle),
                "wiki" => "Wiki export writes a directory; use the CLI export command.".to_owned(),
                _ => serde_json::to_string_pretty(&bundle)?,
            };
            Ok(mcp_text_tool_result(text))
        }
        _ => Err(format!("Unknown Grafiki tool: {name}").into()),
    }
}

fn mcp_tools() -> serde_json::Value {
    serde_json::json!([
        mcp_tool(
            "grafiki_start",
            "Start a Grafiki session and return a scoped briefing.",
            serde_json::json!({
                "goal": { "type": "string" },
                "type": { "type": "string", "default": "codex" },
                "scope": { "type": "string", "default": "" }
            }),
            &["goal"]
        ),
        mcp_tool(
            "grafiki_end",
            "End the current Grafiki session.",
            serde_json::json!({
                "session": { "type": "string" },
                "status": { "type": "string", "default": "completed" },
                "summary": { "type": "string" },
                "accomplishments": { "type": "array", "items": { "type": "string" } },
                "remaining": { "type": "array", "items": { "type": "string" } },
                "files": { "type": "array", "items": { "type": "string" } }
            }),
            &[]
        ),
        mcp_tool(
            "grafiki_handoff",
            "Create a child session from the current or selected Grafiki session and return the generated handoff context.",
            serde_json::json!({
                "session": { "type": "string", "description": "Optional parent session id. Uses the latest active session when omitted." },
                "session_id": { "type": "string", "description": "Alias for session." }
            }),
            &[]
        ),
        mcp_tool(
            "grafiki_status",
            "Show active sessions, work, decisions, and recent events.",
            serde_json::json!({
                "scope": { "type": "string", "default": "" }
            }),
            &[]
        ),
        mcp_tool(
            "grafiki_ask",
            "Ask Grafiki for an agent-ready memory briefing for a question or coding task.",
            serde_json::json!({
                "question": { "type": "string" },
                "scope": { "type": "string", "default": "" },
                "limit": { "type": "integer", "default": 8 },
                "agent": { "type": "string", "default": "mcp" }
            }),
            &["question"]
        ),
        mcp_tool(
            "grafiki_agent_activity",
            "List recent agent questions and the memory records Grafiki returned.",
            serde_json::json!({
                "scope": { "type": "string", "default": "" },
                "limit": { "type": "integer", "default": 20 }
            }),
            &[]
        ),
        mcp_tool(
            "grafiki_auto_capture",
            "Inspect the current git working tree and propose reviewable Grafiki memory candidates.",
            serde_json::json!({
                "scope": { "type": "string", "default": "" },
                "source": { "type": "string", "description": "Optional source label, such as the calling agent or thread id." },
                "limit": { "type": "integer", "default": 25 }
            }),
            &[]
        ),
        mcp_tool(
            "grafiki_capture_start",
            "Start a raw automatic capture session for transcript, screen, IDE, terminal, file, git, and agent events.",
            serde_json::json!({
                "scope": { "type": "string", "default": "" },
                "source_app": { "type": "string" },
                "consent_profile": { "type": "string", "default": "local-explicit" },
                "redaction_profile": { "type": "string", "default": "default" }
            }),
            &[]
        ),
        mcp_tool(
            "grafiki_capture_stop",
            "Stop a raw automatic capture session.",
            serde_json::json!({
                "id": { "type": "string" },
                "capture": { "type": "string" }
            }),
            &[]
        ),
        mcp_tool(
            "grafiki_capture_ingest",
            "Ingest one raw capture event from transcript, screen, IDE, terminal, browser, file, git, agent, or system sources.",
            serde_json::json!({
                "capture": { "type": "string" },
                "source_type": { "type": "string", "enum": ["transcript", "screen", "ide", "file", "terminal", "browser", "agent", "system", "git"] },
                "type": { "type": "string", "description": "Alias for source_type." },
                "source": { "type": "string" },
                "title": { "type": "string" },
                "text": { "type": "string" },
                "payload": { "type": "object" },
                "metadata": { "type": "object" },
                "privacy": { "type": "string", "enum": ["public", "internal", "sensitive", "secret"], "default": "internal" },
                "redacted": { "type": "boolean", "default": false },
                "scope": { "type": "string", "default": "" },
                "captured_at": { "type": "string" }
            }),
            &["source_type"]
        ),
        mcp_tool(
            "grafiki_capture_import_transcripts",
            "Import Codex, Claude Code, Cursor, or generic transcript files as raw capture events, with optional candidate summarization.",
            serde_json::json!({
                "agent": { "type": "string", "enum": ["codex", "claude-code", "cursor", "generic"] },
                "input": { "type": "string", "description": "Optional transcript file or directory. Defaults to the known local history folder for the agent." },
                "scope": { "type": "string", "default": "" },
                "limit": { "type": "integer", "default": 200 },
                "summarize": { "type": "boolean", "default": false }
            }),
            &["agent"]
        ),
        mcp_tool(
            "grafiki_capture_config",
            "Show workspace capture consent settings.",
            serde_json::json!({}),
            &[]
        ),
        mcp_tool(
            "grafiki_capture_config_set",
            "Update workspace capture consent settings.",
            serde_json::json!({
                "git": { "type": "boolean" },
                "transcripts": { "type": "boolean" },
                "terminal": { "type": "boolean" },
                "files": { "type": "boolean" },
                "ide": { "type": "boolean" },
                "screen": { "type": "boolean" },
                "browser": { "type": "boolean" },
                "audio": { "type": "boolean" },
                "system": { "type": "boolean" },
                "add_blocked_paths": { "type": "array", "items": { "type": "string" } },
                "remove_blocked_paths": { "type": "array", "items": { "type": "string" } },
                "add_blocked_apps": { "type": "array", "items": { "type": "string" } },
                "remove_blocked_apps": { "type": "array", "items": { "type": "string" } },
                "redaction_profile": { "type": "string" },
                "terminal_output": { "type": "string", "enum": ["off", "digest", "full"] },
                "screen_policy": { "type": "string", "enum": ["off", "manual", "allowlist"] },
                "browser_policy": { "type": "string", "enum": ["off", "allowlist"] }
            }),
            &[]
        ),
        mcp_tool(
            "grafiki_capture_terminal_command",
            "Record one terminal command execution as metadata-only raw capture. Does not capture stdout.",
            serde_json::json!({
                "command": { "type": "string" },
                "cmd": { "type": "string", "description": "Alias for command." },
                "cwd": { "type": "string" },
                "exit_code": { "type": "integer" },
                "duration_ms": { "type": "integer" },
                "shell": { "type": "string" },
                "scope": { "type": "string", "default": "" }
            }),
            &[]
        ),
        mcp_tool(
            "grafiki_capture_watch_files",
            "Poll recent workspace file metadata into raw capture events.",
            serde_json::json!({
                "scope": { "type": "string", "default": "" },
                "since_seconds": { "type": "integer", "default": 300 },
                "duration_seconds": { "type": "integer", "default": 0 },
                "interval_ms": { "type": "integer", "default": 1000 },
                "limit": { "type": "integer", "default": 100 },
                "summarize": { "type": "boolean", "default": false }
            }),
            &[]
        ),
        mcp_tool(
            "grafiki_capture_git_summary",
            "Record a git branch/status/diff-stat snapshot as raw capture, with optional candidate summarization.",
            serde_json::json!({
                "scope": { "type": "string", "default": "" },
                "source": { "type": "string", "default": "git" },
                "limit": { "type": "integer", "default": 80 },
                "summarize": { "type": "boolean", "default": false }
            }),
            &[]
        ),
        mcp_tool(
            "grafiki_capture_status",
            "Show active raw capture sessions and recent captured events.",
            serde_json::json!({
                "scope": { "type": "string", "default": "" }
            }),
            &[]
        ),
        mcp_tool(
            "grafiki_capture_events",
            "List raw capture events before they are promoted into memory.",
            serde_json::json!({
                "capture": { "type": "string" },
                "source_type": { "type": "string" },
                "type": { "type": "string", "description": "Alias for source_type." },
                "scope": { "type": "string", "default": "" },
                "limit": { "type": "integer", "default": 50 }
            }),
            &[]
        ),
        mcp_tool(
            "grafiki_capture_summarize",
            "Summarize raw capture events into pending memory candidates for review.",
            serde_json::json!({
                "capture": { "type": "string" },
                "scope": { "type": "string", "default": "" },
                "limit": { "type": "integer", "default": 80 }
            }),
            &[]
        ),
        mcp_tool(
            "grafiki_search",
            "Search Grafiki project memory.",
            serde_json::json!({
                "query": { "type": "string" },
                "type": { "type": "string", "default": "all" },
                "mode": { "type": "string", "enum": ["keyword", "semantic", "hybrid"], "default": "keyword" },
                "scope": { "type": "string", "default": "" },
                "limit": { "type": "integer", "default": 10 }
            }),
            &["query"]
        ),
        mcp_tool(
            "grafiki_candidate_propose",
            "Propose untrusted memory for review without writing it into trusted memory.",
            serde_json::json!({
                "type": { "type": "string", "enum": ["entity", "observation", "decision", "context", "state"] },
                "payload": { "type": "object" },
                "source_type": { "type": "string", "default": "agent" },
                "source": { "type": "string" },
                "scope": { "type": "string", "default": "" },
                "confidence": { "type": "number", "default": 0.5 },
                "rationale": { "type": "string" }
            }),
            &["type", "payload"]
        ),
        mcp_tool(
            "grafiki_candidate_list",
            "List candidate memory awaiting review.",
            serde_json::json!({
                "status": { "type": "string", "enum": ["pending", "approved", "rejected", "all"], "default": "pending" },
                "scope": { "type": "string", "default": "" },
                "limit": { "type": "integer", "default": 20 }
            }),
            &[]
        ),
        mcp_tool(
            "grafiki_candidate_edit",
            "Edit a pending candidate before approving it into trusted memory.",
            serde_json::json!({
                "id": { "type": "string" },
                "type": { "type": "string", "enum": ["entity", "observation", "decision", "context", "state"] },
                "payload": { "type": "object" },
                "scope": { "type": "string" },
                "confidence": { "type": "number" },
                "rationale": { "type": "string" }
            }),
            &["id"]
        ),
        mcp_tool(
            "grafiki_candidate_bulk",
            "Approve or reject several candidates in one review action.",
            serde_json::json!({
                "action": { "type": "string", "enum": ["approve", "reject"] },
                "ids": { "type": "array", "items": { "type": "string" } },
                "rationale": { "type": "string" }
            }),
            &["action", "ids"]
        ),
        mcp_tool(
            "grafiki_candidate_approve",
            "Approve one candidate into trusted Grafiki memory.",
            serde_json::json!({
                "id": { "type": "string" }
            }),
            &["id"]
        ),
        mcp_tool(
            "grafiki_candidate_reject",
            "Reject one candidate without trusting it.",
            serde_json::json!({
                "id": { "type": "string" },
                "rationale": { "type": "string" }
            }),
            &["id"]
        ),
        mcp_tool(
            "grafiki_record",
            "Load one full Grafiki memory record with metadata, related records, and provenance events.",
            serde_json::json!({
                "type": { "type": "string", "enum": ["entity", "observation", "decision", "context", "state", "relation", "session"] },
                "id": { "type": "string" },
                "scope": { "type": "string", "default": "" }
            }),
            &["type", "id"]
        ),
        mcp_tool(
            "grafiki_update_record",
            "Update one Grafiki memory record by type and id.",
            serde_json::json!({
                "type": { "type": "string", "enum": ["entity", "observation", "decision", "context", "state", "relation", "session"] },
                "id": { "type": "string" },
                "key": { "type": "string", "description": "Alias for id, mainly for context and state records." },
                "title": { "type": "string" },
                "name": { "type": "string" },
                "content": { "type": "string" },
                "reasoning": { "type": "string" },
                "scope": { "type": "string" },
                "status": { "type": "string" },
                "category": { "type": "string" },
                "entity_type": { "type": "string" },
                "owner": { "type": "string" },
                "details": { "type": "string" },
                "priority": { "type": "string" },
                "relation": { "type": "string" },
                "weight": { "type": "number" },
                "confidence": { "type": "number" },
                "source": { "type": "string" },
                "source_type": { "type": "string" },
                "session_type": { "type": "string" },
                "goal": { "type": "string" },
                "summary": { "type": "string" }
            }),
            &["type", "id"]
        ),
        mcp_tool(
            "grafiki_delete_record",
            "Delete or invalidate one Grafiki memory record by type and id. Sessions are not deletable.",
            serde_json::json!({
                "type": { "type": "string", "enum": ["entity", "observation", "decision", "context", "state", "relation"] },
                "id": { "type": "string" },
                "key": { "type": "string", "description": "Alias for id, mainly for context and state records." }
            }),
            &["type", "id"]
        ),
        mcp_tool(
            "grafiki_save",
            "Save an entity, observation, and optional relation.",
            serde_json::json!({
                "name": { "type": "string" },
                "entity_type": { "type": "string", "default": "concept" },
                "observe": { "type": "string" },
                "category": { "type": "string", "default": "general" },
                "scope": { "type": "string", "default": "" },
                "relate": { "type": "string", "description": "target-id:relation" }
            }),
            &["name"]
        ),
        mcp_tool(
            "grafiki_decide",
            "Log a project decision.",
            serde_json::json!({
                "title": { "type": "string" },
                "reasoning": { "type": "string" },
                "alternatives": { "type": "array", "items": { "type": "string" } },
                "tags": { "type": "array", "items": { "type": "string" } },
                "scope": { "type": "string", "default": "" },
                "supersedes": { "type": "string" }
            }),
            &["title"]
        ),
        mcp_tool(
            "grafiki_state_set",
            "Create or update tracked work state.",
            serde_json::json!({
                "key": { "type": "string" },
                "title": { "type": "string" },
                "status": { "type": "string", "default": "in-progress" },
                "owner": { "type": "string" },
                "details": { "type": "string" },
                "blockers": { "type": "array", "items": { "type": "string" } },
                "depends_on": { "type": "array", "items": { "type": "string" } },
                "scope": { "type": "string", "default": "" },
                "priority": { "type": "string", "default": "medium" }
            }),
            &["key", "title"]
        ),
        mcp_tool(
            "grafiki_embeddings_status",
            "Show semantic-search embedding queue counts and model metadata.",
            serde_json::json!({
                "scope": { "type": "string", "default": "" }
            }),
            &[]
        ),
        mcp_tool(
            "grafiki_embeddings_process",
            "Process pending semantic-search embedding jobs, optionally rebuilding the queue first.",
            serde_json::json!({
                "scope": { "type": "string", "default": "" },
                "limit": { "type": "integer", "default": 100 },
                "rebuild": { "type": "boolean", "default": false }
            }),
            &[]
        ),
        mcp_tool(
            "grafiki_report",
            "Generate a project memory report.",
            serde_json::json!({
                "scope": { "type": "string", "default": "" }
            }),
            &[]
        ),
        mcp_tool(
            "grafiki_graph",
            "Traverse the entity relation graph from one entity.",
            serde_json::json!({
                "entity_id": { "type": "string" },
                "depth": { "type": "integer", "default": 2 }
            }),
            &["entity_id"]
        ),
        mcp_tool(
            "grafiki_export",
            "Export scoped project memory as text.",
            serde_json::json!({
                "scope": { "type": "string", "default": "" },
                "format": { "type": "string", "default": "json" }
            }),
            &[]
        ),
    ])
}

fn mcp_tool(
    name: &str,
    description: &str,
    properties: serde_json::Value,
    required: &[&str],
) -> serde_json::Value {
    serde_json::json!({
        "name": name,
        "description": description,
        "inputSchema": {
            "type": "object",
            "properties": properties,
            "required": required,
            "additionalProperties": false
        }
    })
}

fn mcp_json_tool_result<T: serde::Serialize>(
    value: &T,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    Ok(mcp_text_tool_result(serde_json::to_string_pretty(value)?))
}

fn mcp_text_tool_result(text: String) -> serde_json::Value {
    serde_json::json!({
        "content": [
            {
                "type": "text",
                "text": text
            }
        ],
        "isError": false
    })
}

fn mcp_result(id: serde_json::Value, result: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result
    })
}

fn mcp_error(id: serde_json::Value, code: i64, message: &str) -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": code,
            "message": message
        }
    })
}

fn json_required_string(
    args: &serde_json::Value,
    key: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    json_optional_string(args, key)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("Missing required argument: {key}").into())
}

fn json_arg_string(args: &serde_json::Value, key: &str, default: &str) -> String {
    json_optional_string(args, key).unwrap_or_else(|| default.to_owned())
}

fn json_optional_string(args: &serde_json::Value, key: &str) -> Option<String> {
    args.get(key).and_then(|value| match value {
        serde_json::Value::String(value) => Some(value.clone()),
        serde_json::Value::Number(value) => Some(value.to_string()),
        serde_json::Value::Bool(value) => Some(value.to_string()),
        _ => None,
    })
}

fn json_record_type(args: &serde_json::Value) -> Result<String, Box<dyn std::error::Error>> {
    let raw = json_optional_string(args, "type")
        .or_else(|| json_optional_string(args, "record_type"))
        .or_else(|| json_optional_string(args, "recordType"))
        .filter(|value| !value.trim().is_empty())
        .ok_or("Missing required argument: type")?;

    let normalized = match raw.trim().to_ascii_lowercase().as_str() {
        "entity" | "entities" => "entity",
        "observation" | "observations" => "observation",
        "decision" | "decisions" => "decision",
        "context" | "contexts" => "context",
        "state" | "states" | "state_item" | "state-item" | "stateitem" => "state",
        "relation" | "relations" => "relation",
        "session" | "sessions" => "session",
        other => return Err(format!("Unsupported memory record type: {other}").into()),
    };
    Ok(normalized.to_owned())
}

fn json_record_id(args: &serde_json::Value) -> Result<String, Box<dyn std::error::Error>> {
    json_optional_string(args, "id")
        .or_else(|| json_optional_string(args, "key"))
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "Missing required argument: id".into())
}

fn json_optional_f64(args: &serde_json::Value, key: &str) -> Option<f64> {
    args.get(key).and_then(|value| match value {
        serde_json::Value::Number(value) => value.as_f64(),
        serde_json::Value::String(value) => value.parse::<f64>().ok(),
        _ => None,
    })
}

fn json_optional_i32(args: &serde_json::Value, key: &str) -> Option<i32> {
    args.get(key).and_then(|value| match value {
        serde_json::Value::Number(value) => value.as_i64().map(|value| value as i32),
        serde_json::Value::String(value) => value.parse::<i32>().ok(),
        _ => None,
    })
}

fn json_optional_u64(args: &serde_json::Value, key: &str) -> Option<u64> {
    args.get(key).and_then(|value| match value {
        serde_json::Value::Number(value) => value.as_u64(),
        serde_json::Value::String(value) => value.parse::<u64>().ok(),
        _ => None,
    })
}

fn json_optional_vec(args: &serde_json::Value, key: &str) -> Option<Vec<String>> {
    args.get(key).map(|_| json_arg_vec(args, key))
}

fn json_arg_usize(args: &serde_json::Value, key: &str, default: usize) -> usize {
    args.get(key)
        .and_then(|value| match value {
            serde_json::Value::Number(value) => value.as_u64().map(|value| value as usize),
            serde_json::Value::String(value) => value.parse::<usize>().ok(),
            _ => None,
        })
        .unwrap_or(default)
}

fn json_arg_bool(args: &serde_json::Value, key: &str, default: bool) -> bool {
    args.get(key)
        .and_then(|value| match value {
            serde_json::Value::Bool(value) => Some(*value),
            serde_json::Value::String(value) => value.parse::<bool>().ok(),
            _ => None,
        })
        .unwrap_or(default)
}

fn json_optional_bool(args: &serde_json::Value, key: &str) -> Option<bool> {
    args.get(key).and_then(|value| match value {
        serde_json::Value::Bool(value) => Some(*value),
        serde_json::Value::String(value) => value.parse::<bool>().ok(),
        _ => None,
    })
}

fn json_arg_vec(args: &serde_json::Value, key: &str) -> Vec<String> {
    match args.get(key) {
        Some(serde_json::Value::Array(items)) => items
            .iter()
            .filter_map(serde_json::Value::as_str)
            .map(ToOwned::to_owned)
            .collect(),
        Some(serde_json::Value::String(value)) => split_csv(Some(value.clone())),
        _ => Vec::new(),
    }
}

fn json_string_array(
    args: &serde_json::Value,
    key: &str,
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let ids = json_arg_vec(args, key);
    if ids.is_empty() {
        Err(format!("Missing required non-empty string array: {key}").into())
    } else {
        Ok(ids)
    }
}

fn json_evidence_inputs(args: &serde_json::Value) -> Vec<EvidenceInput> {
    let Some(items) = args.get("evidence").and_then(serde_json::Value::as_array) else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| {
            let source_type = json_optional_string(item, "source_type")
                .or_else(|| json_optional_string(item, "sourceType"))?;
            Some(EvidenceInput {
                source_event_id: json_optional_string(item, "source_event_id")
                    .or_else(|| json_optional_string(item, "sourceEventId")),
                source_type,
                source: json_optional_string(item, "source"),
                title: json_optional_string(item, "title"),
                excerpt: json_arg_string(item, "excerpt", ""),
                uri: json_optional_string(item, "uri"),
                byte_start: json_optional_i64(item, "byte_start")
                    .or_else(|| json_optional_i64(item, "byteStart")),
                byte_end: json_optional_i64(item, "byte_end")
                    .or_else(|| json_optional_i64(item, "byteEnd")),
                line_start: json_optional_i64(item, "line_start")
                    .or_else(|| json_optional_i64(item, "lineStart")),
                line_end: json_optional_i64(item, "line_end")
                    .or_else(|| json_optional_i64(item, "lineEnd")),
                captured_at: json_optional_string(item, "captured_at")
                    .or_else(|| json_optional_string(item, "capturedAt")),
            })
        })
        .collect()
}

fn json_optional_i64(args: &serde_json::Value, key: &str) -> Option<i64> {
    args.get(key).and_then(|value| match value {
        serde_json::Value::Number(value) => value.as_i64(),
        serde_json::Value::String(value) => value.parse::<i64>().ok(),
        _ => None,
    })
}

fn read_content(
    content: Option<String>,
    file: Option<PathBuf>,
) -> Result<String, Box<dyn std::error::Error>> {
    read_optional_content(content, file)?
        .ok_or_else(|| grafiki_core::GrafikiError::MissingContextContent.into())
}

fn read_optional_content(
    content: Option<String>,
    file: Option<PathBuf>,
) -> Result<Option<String>, Box<dyn std::error::Error>> {
    match (content, file) {
        (Some(content), None) => Ok(Some(content)),
        (None, Some(file)) => Ok(Some(fs::read_to_string(file)?)),
        (Some(_), Some(_)) => Err("Pass either --content or --file, not both.".into()),
        (None, None) => Ok(None),
    }
}

fn print_context_report(
    report: &grafiki_core::ContextReport,
    format: OutputFormat,
    verb: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    match format {
        OutputFormat::Plain => {
            println!("{verb} context: {}", report.key);
            println!("Title: {}", report.title);
            println!("Category: {}", report.category);
            println!("Scope: {}", display_scope(&report.scope));
            println!("Version: {}", report.version);
        }
        OutputFormat::Md => {
            println!("# Grafiki Context {verb}\n");
            println!("- Key: {}", report.key);
            println!("- Title: {}", report.title);
            println!("- Category: {}", report.category);
            println!("- Scope: {}", display_scope(&report.scope));
            println!("- Version: {}", report.version);
        }
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(report)?),
    }
    Ok(())
}
