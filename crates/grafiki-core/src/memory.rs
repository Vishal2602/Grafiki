use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::time::Instant;

use crate::embeddings::{
    configured_embedding_provider, configured_embedding_provider_summary, cosine_similarity,
    EmbeddingProvider,
};
#[cfg(feature = "sqlite-vec")]
use crate::embeddings::{SqliteVecBackend, VectorBackend, VectorRecord};
use rusqlite::{params, params_from_iter, types::Type, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::db::{open_project_database, schema::initialize_schema};
use crate::project::{resolve_project, ProjectContext, ProjectResolveOptions};
use crate::scope::Scope;
use crate::ulid::new_ulid;
use crate::{GrafikiError, Result};

#[derive(Debug, Clone)]
pub struct EndSessionOptions {
    pub project_name: Option<String>,
    pub start_dir: PathBuf,
    pub grafiki_home: Option<PathBuf>,
    pub session_id: Option<String>,
    pub status: String,
    pub summary: Option<String>,
    pub accomplishments: Vec<String>,
    pub remaining: Vec<String>,
    pub files_changed: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EndSessionReport {
    pub session_id: String,
    pub project: String,
    pub status: String,
    pub summary: Option<String>,
}

#[derive(Debug, Clone)]
pub struct HandoffOptions {
    pub project_name: Option<String>,
    pub start_dir: PathBuf,
    pub grafiki_home: Option<PathBuf>,
    pub session_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HandoffReport {
    pub parent_session_id: String,
    pub child_session_id: String,
    pub project: String,
    pub scope: String,
    pub handoff_context: String,
}

#[derive(Debug, Clone)]
pub struct UpdateSessionOptions {
    pub project_name: Option<String>,
    pub start_dir: PathBuf,
    pub grafiki_home: Option<PathBuf>,
    pub id: String,
    pub session_type: Option<String>,
    pub status: Option<String>,
    pub scope: Option<String>,
    pub goal: Option<String>,
    pub summary: Option<String>,
    pub accomplishments: Option<Vec<String>>,
    pub remaining: Option<Vec<String>>,
    pub files_changed: Option<Vec<String>>,
}

#[derive(Debug, Clone)]
pub struct LogDecisionOptions {
    pub project_name: Option<String>,
    pub start_dir: PathBuf,
    pub grafiki_home: Option<PathBuf>,
    pub title: String,
    pub reasoning: Option<String>,
    pub alternatives: Vec<String>,
    pub tags: Vec<String>,
    pub scope: String,
    pub supersedes: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionReport {
    pub decision_id: String,
    pub project: String,
    pub title: String,
    pub scope: String,
    pub supersedes: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DecisionListOptions {
    pub project_name: Option<String>,
    pub start_dir: PathBuf,
    pub grafiki_home: Option<PathBuf>,
    pub scope: String,
    pub status: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionItem {
    pub id: String,
    pub title: String,
    pub status: String,
    pub scope: String,
    pub reasoning: Option<String>,
}

#[derive(Debug, Clone)]
pub struct UpdateDecisionOptions {
    pub project_name: Option<String>,
    pub start_dir: PathBuf,
    pub grafiki_home: Option<PathBuf>,
    pub id: String,
    pub title: Option<String>,
    pub reasoning: Option<String>,
    pub scope: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DeleteDecisionOptions {
    pub project_name: Option<String>,
    pub start_dir: PathBuf,
    pub grafiki_home: Option<PathBuf>,
    pub id: String,
}

#[derive(Debug, Clone)]
pub struct SaveEntityOptions {
    pub project_name: Option<String>,
    pub start_dir: PathBuf,
    pub grafiki_home: Option<PathBuf>,
    pub name: String,
    pub entity_type: String,
    pub observe: Option<String>,
    pub category: String,
    pub scope: String,
    pub relate: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SaveEntityReport {
    pub entity_id: String,
    pub project: String,
    pub created: bool,
    pub observation_id: Option<String>,
    pub relation_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct EntityListOptions {
    pub project_name: Option<String>,
    pub start_dir: PathBuf,
    pub grafiki_home: Option<PathBuf>,
    pub scope: String,
    pub entity_type: Option<String>,
}

#[derive(Debug, Clone)]
pub struct UpdateEntityOptions {
    pub project_name: Option<String>,
    pub start_dir: PathBuf,
    pub grafiki_home: Option<PathBuf>,
    pub id: String,
    pub name: Option<String>,
    pub entity_type: Option<String>,
    pub scope: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DeleteEntityOptions {
    pub project_name: Option<String>,
    pub start_dir: PathBuf,
    pub grafiki_home: Option<PathBuf>,
    pub id: String,
}

#[derive(Debug, Clone)]
pub struct ObservationListOptions {
    pub project_name: Option<String>,
    pub start_dir: PathBuf,
    pub grafiki_home: Option<PathBuf>,
    pub scope: String,
    pub category: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObservationItem {
    pub id: String,
    pub entity_id: String,
    pub entity_name: String,
    pub content: String,
    pub category: String,
    pub confidence: f64,
    pub scope: String,
}

#[derive(Debug, Clone)]
pub struct UpdateObservationOptions {
    pub project_name: Option<String>,
    pub start_dir: PathBuf,
    pub grafiki_home: Option<PathBuf>,
    pub id: String,
    pub content: Option<String>,
    pub category: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DeleteObservationOptions {
    pub project_name: Option<String>,
    pub start_dir: PathBuf,
    pub grafiki_home: Option<PathBuf>,
    pub id: String,
}

#[derive(Debug, Clone)]
pub struct RelationListOptions {
    pub project_name: Option<String>,
    pub start_dir: PathBuf,
    pub grafiki_home: Option<PathBuf>,
    pub scope: String,
    pub relation: Option<String>,
}

#[derive(Debug, Clone)]
pub struct UpdateRelationOptions {
    pub project_name: Option<String>,
    pub start_dir: PathBuf,
    pub grafiki_home: Option<PathBuf>,
    pub id: String,
    pub relation: Option<String>,
    pub weight: Option<f64>,
    pub confidence: Option<f64>,
    pub source_type: Option<String>,
    pub source: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DeleteRelationOptions {
    pub project_name: Option<String>,
    pub start_dir: PathBuf,
    pub grafiki_home: Option<PathBuf>,
    pub id: String,
}

#[derive(Debug, Clone)]
pub struct SearchMemoryOptions {
    pub project_name: Option<String>,
    pub start_dir: PathBuf,
    pub grafiki_home: Option<PathBuf>,
    pub query: String,
    pub record_type: String,
    pub mode: SearchMode,
    pub scope: String,
    pub limit: usize,
    /// M-E1/M-E2 temporal boost. `0.0` (default) = off → fusion is byte-identical to the
    /// lexical/dense ranking. When `> 0`, recent + frequently-reused records get an additive
    /// bonus in the fused arms (Hybrid/Graph/Rerank), scaled to ~one RRF rank per unit. See
    /// `grafiki_core::decay` and `docs/DECAY_DESIGN.md`.
    pub temporal_weight: f64,
}

#[derive(Debug, Clone)]
pub struct AskMemoryOptions {
    pub project_name: Option<String>,
    pub start_dir: PathBuf,
    pub grafiki_home: Option<PathBuf>,
    pub question: String,
    pub scope: String,
    pub limit: usize,
    pub agent: Option<String>,
    /// M-E2 temporal/decay boost for the briefing's retrieval (0.0 = off; see `crate::decay`).
    pub temporal_weight: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SearchMode {
    Keyword,
    Semantic,
    Hybrid,
    /// Hybrid + a graph-aware arm: Personalized PageRank over the `relations`
    /// graph, seeded from the lexical/dense hits, fused into the RRF. Helps
    /// multi-hop / relational queries (H3).
    Graph,
    /// Hybrid fusion, then a local cross-encoder reranker (bge-reranker) over the
    /// top-N candidates for a more precise final ordering (H4). Requires the model.
    Rerank,
}

impl SearchMode {
    pub fn parse(raw: &str) -> Result<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "keyword" => Ok(Self::Keyword),
            "semantic" => Ok(Self::Semantic),
            "hybrid" => Ok(Self::Hybrid),
            "graph" => Ok(Self::Graph),
            "rerank" => Ok(Self::Rerank),
            _ => Err(GrafikiError::InvalidSearchMode(raw.to_owned())),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchReport {
    pub project: String,
    pub query: String,
    pub mode: SearchMode,
    pub semantic_available: bool,
    pub fallback: Option<String>,
    pub results: Vec<SearchResult>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchResult {
    pub record_type: String,
    pub id: String,
    pub title: String,
    pub snippet: String,
    pub scope: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub score: Option<f64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence: Vec<EvidenceLink>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentMemoryBriefing {
    pub project: String,
    pub scope: String,
    pub question: String,
    pub agent: String,
    pub audit_id: String,
    pub answer: String,
    pub active_sessions: Vec<String>,
    pub active_state: Vec<String>,
    pub recent_decisions: Vec<String>,
    pub recent_events: Vec<String>,
    pub relevant_memory: Vec<SearchResult>,
    pub pending_candidates: usize,
    pub semantic_available: bool,
    pub fallback: Option<String>,
    pub agent_instructions: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ListAgentQueriesOptions {
    pub project_name: Option<String>,
    pub start_dir: PathBuf,
    pub grafiki_home: Option<PathBuf>,
    pub scope: String,
    pub limit: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentQueryLogItem {
    pub id: String,
    pub agent: String,
    pub question: String,
    pub scope: String,
    pub returned_ids: Vec<String>,
    pub retrieval_mode: String,
    pub fallback: Option<String>,
    pub latency_ms: i64,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct EmbeddingStatusOptions {
    pub project_name: Option<String>,
    pub start_dir: PathBuf,
    pub grafiki_home: Option<PathBuf>,
    pub scope: String,
}

#[derive(Debug, Clone)]
pub struct ProcessEmbeddingsOptions {
    pub project_name: Option<String>,
    pub start_dir: PathBuf,
    pub grafiki_home: Option<PathBuf>,
    pub scope: String,
    pub limit: usize,
    pub rebuild: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmbeddingStatusReport {
    pub project: String,
    pub scope: String,
    pub runtime: EmbeddingRuntimeSummary,
    pub pending: i64,
    pub embedded: i64,
    pub failed: i64,
    pub skipped: i64,
    pub metadata: Vec<EmbeddingMetadataSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmbeddingRuntimeSummary {
    pub requested_provider: String,
    pub provider: String,
    pub model: String,
    pub dimension: Option<i64>,
    pub vector_backend: String,
    pub embeddable_records: i64,
    pub indexed_records: i64,
    pub fresh_records: i64,
    pub missing_or_stale_records: i64,
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmbeddingMetadataSummary {
    pub provider: String,
    pub model: String,
    pub dimension: i64,
    pub records: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessEmbeddingsReport {
    pub project: String,
    pub scope: String,
    pub provider: String,
    pub model: String,
    pub dimension: usize,
    pub enqueued: usize,
    pub processed: usize,
    pub skipped: usize,
    pub failed: usize,
    pub pending_remaining: i64,
}

#[derive(Debug, Clone)]
pub struct GraphOptions {
    pub project_name: Option<String>,
    pub start_dir: PathBuf,
    pub grafiki_home: Option<PathBuf>,
    pub entity_id: String,
    pub depth: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GraphReport {
    pub project: String,
    pub root: String,
    pub depth: usize,
    pub entities: Vec<GraphEntity>,
    pub relations: Vec<GraphRelation>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GraphEntity {
    pub id: String,
    pub name: String,
    pub entity_type: String,
    pub scope: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GraphRelation {
    pub id: String,
    pub from_entity: String,
    pub to_entity: String,
    pub relation: String,
    pub weight: f64,
    pub confidence: f64,
    pub source_type: String,
    pub source: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ProjectReportOptions {
    pub project_name: Option<String>,
    pub start_dir: PathBuf,
    pub grafiki_home: Option<PathBuf>,
    pub scope: String,
}

#[derive(Debug, Clone)]
pub struct ExportOptions {
    pub project_name: Option<String>,
    pub start_dir: PathBuf,
    pub grafiki_home: Option<PathBuf>,
    pub scope: String,
}

#[derive(Debug, Clone)]
pub struct ImportOptions {
    pub project_name: Option<String>,
    pub start_dir: PathBuf,
    pub grafiki_home: Option<PathBuf>,
    pub bundle: ExportBundle,
}

#[derive(Debug, Clone)]
pub struct ProposeCandidateOptions {
    pub project_name: Option<String>,
    pub start_dir: PathBuf,
    pub grafiki_home: Option<PathBuf>,
    pub source_type: String,
    pub source: Option<String>,
    pub record_type: String,
    pub payload: serde_json::Value,
    pub scope: String,
    pub confidence: f64,
    pub rationale: Option<String>,
    pub evidence: Vec<EvidenceInput>,
}

#[derive(Debug, Clone)]
pub struct ListCandidatesOptions {
    pub project_name: Option<String>,
    pub start_dir: PathBuf,
    pub grafiki_home: Option<PathBuf>,
    pub status: Option<String>,
    pub scope: String,
    pub limit: usize,
    /// M-E3 review ordering. `Recent` (default) preserves the newest-first behavior; `ActiveLearning`
    /// surfaces the most informative (uncertain × well-evidenced) candidates first.
    pub order: CandidateOrder,
}

/// Ordering for the candidate review queue (M-E3).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CandidateOrder {
    /// Newest candidates first (back-compatible default).
    #[default]
    Recent,
    /// Active-learning: highest `review_priority` (uncertainty × representativeness) first.
    ActiveLearning,
}

#[derive(Debug, Clone)]
pub struct ApproveCandidateOptions {
    pub project_name: Option<String>,
    pub start_dir: PathBuf,
    pub grafiki_home: Option<PathBuf>,
    pub id: String,
}

#[derive(Debug, Clone)]
pub struct EditCandidateOptions {
    pub project_name: Option<String>,
    pub start_dir: PathBuf,
    pub grafiki_home: Option<PathBuf>,
    pub id: String,
    pub record_type: Option<String>,
    pub payload: Option<serde_json::Value>,
    pub scope: Option<String>,
    pub confidence: Option<f64>,
    pub rationale: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RejectCandidateOptions {
    pub project_name: Option<String>,
    pub start_dir: PathBuf,
    pub grafiki_home: Option<PathBuf>,
    pub id: String,
    pub rationale: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RevertApprovalOptions {
    pub project_name: Option<String>,
    pub start_dir: PathBuf,
    pub grafiki_home: Option<PathBuf>,
    pub id: String,
}

#[derive(Debug, Clone)]
pub struct ReopenCandidateOptions {
    pub project_name: Option<String>,
    pub start_dir: PathBuf,
    pub grafiki_home: Option<PathBuf>,
    pub id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceLink {
    pub id: String,
    pub candidate_id: Option<String>,
    pub trusted_record_type: Option<String>,
    pub trusted_record_id: Option<String>,
    pub source_event_id: Option<String>,
    pub source_type: String,
    pub source: Option<String>,
    pub title: Option<String>,
    pub excerpt: String,
    pub uri: Option<String>,
    pub byte_start: Option<i64>,
    pub byte_end: Option<i64>,
    pub line_start: Option<i64>,
    pub line_end: Option<i64>,
    pub captured_at: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceInput {
    pub source_event_id: Option<String>,
    pub source_type: String,
    pub source: Option<String>,
    pub title: Option<String>,
    pub excerpt: String,
    pub uri: Option<String>,
    pub byte_start: Option<i64>,
    pub byte_end: Option<i64>,
    pub line_start: Option<i64>,
    pub line_end: Option<i64>,
    pub captured_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExportBundle {
    pub project: String,
    pub scope: String,
    pub entities: Vec<GraphEntity>,
    pub relations: Vec<GraphRelation>,
    pub observations: Vec<ExportObservation>,
    pub decisions: Vec<ExportDecision>,
    pub state: Vec<StateItem>,
    pub context: Vec<ExportContext>,
    pub sessions: Vec<SessionLogItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportContext {
    pub key: String,
    pub title: String,
    pub category: String,
    pub scope: String,
    pub version: i64,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExportObservation {
    pub id: String,
    pub entity_id: String,
    pub content: String,
    pub category: String,
    pub confidence: f64,
    pub scope: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportDecision {
    pub id: String,
    pub title: String,
    pub status: String,
    pub scope: String,
    pub reasoning: Option<String>,
    #[serde(default)]
    pub superseded_by: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportReport {
    pub project: String,
    pub source_project: String,
    pub entities: usize,
    pub relations: usize,
    pub skipped_relations: usize,
    pub observations: usize,
    pub decisions: usize,
    pub state: usize,
    pub context: usize,
    pub sessions: usize,
    pub skipped_observations: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExtractionCandidate {
    pub id: String,
    pub source_type: String,
    pub source: Option<String>,
    pub record_type: String,
    pub payload: serde_json::Value,
    pub scope: String,
    pub confidence: f64,
    pub status: String,
    pub rationale: Option<String>,
    pub trusted_record_type: Option<String>,
    pub trusted_record_id: Option<String>,
    pub created_at: String,
    pub reviewed_at: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence: Vec<EvidenceLink>,
    /// M-E3: a principled confidence derived from the source-reliability prior + corroboration
    /// (evidence count) — distinct from the caller-supplied `confidence`. Computed at list time.
    #[serde(default)]
    pub calibrated_confidence: f64,
    /// M-E3: uncertainty × representativeness; higher ⇒ review sooner (active learning). Computed
    /// at list time (0.0 elsewhere).
    #[serde(default)]
    pub review_priority: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CandidateMutationReport {
    pub candidate: ExtractionCandidate,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct BulkCandidateReviewOptions {
    pub project_name: Option<String>,
    pub start_dir: PathBuf,
    pub grafiki_home: Option<PathBuf>,
    pub action: String,
    pub ids: Vec<String>,
    pub rationale: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CandidateReviewError {
    pub id: String,
    pub error: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BulkCandidateReviewReport {
    pub action: String,
    pub requested: usize,
    pub succeeded: usize,
    pub failed: usize,
    pub results: Vec<CandidateMutationReport>,
    pub errors: Vec<CandidateReviewError>,
}

#[derive(Debug, Clone)]
pub struct StartCaptureOptions {
    pub project_name: Option<String>,
    pub start_dir: PathBuf,
    pub grafiki_home: Option<PathBuf>,
    pub scope: String,
    pub source_app: Option<String>,
    pub consent_profile: Option<String>,
    pub redaction_profile: Option<String>,
}

#[derive(Debug, Clone)]
pub struct StopCaptureOptions {
    pub project_name: Option<String>,
    pub start_dir: PathBuf,
    pub grafiki_home: Option<PathBuf>,
    pub capture_id: String,
}

#[derive(Debug, Clone)]
pub struct IngestCaptureEventOptions {
    pub project_name: Option<String>,
    pub start_dir: PathBuf,
    pub grafiki_home: Option<PathBuf>,
    pub capture_id: Option<String>,
    pub scope: String,
    pub source_type: String,
    pub source: Option<String>,
    pub title: Option<String>,
    pub text: Option<String>,
    pub payload: Option<serde_json::Value>,
    pub metadata: Option<serde_json::Value>,
    pub privacy_level: Option<String>,
    pub redacted: bool,
    pub captured_at: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ListCaptureEventsOptions {
    pub project_name: Option<String>,
    pub start_dir: PathBuf,
    pub grafiki_home: Option<PathBuf>,
    pub capture_id: Option<String>,
    pub source_type: Option<String>,
    pub scope: String,
    pub limit: usize,
}

#[derive(Debug, Clone)]
pub struct CaptureStatusOptions {
    pub project_name: Option<String>,
    pub start_dir: PathBuf,
    pub grafiki_home: Option<PathBuf>,
    pub scope: String,
}

#[derive(Debug, Clone)]
pub struct ProposeCaptureCandidatesOptions {
    pub project_name: Option<String>,
    pub start_dir: PathBuf,
    pub grafiki_home: Option<PathBuf>,
    pub capture_id: Option<String>,
    pub scope: String,
    pub limit: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaptureSession {
    pub id: String,
    pub project: String,
    pub scope: String,
    pub status: String,
    pub source_app: Option<String>,
    pub consent_profile: String,
    pub redaction_profile: String,
    pub started_at: String,
    pub ended_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CaptureEvent {
    pub id: String,
    pub capture_session: String,
    pub source_type: String,
    pub source: Option<String>,
    pub title: Option<String>,
    pub text: Option<String>,
    pub payload: Option<serde_json::Value>,
    pub metadata: Option<serde_json::Value>,
    pub privacy_level: String,
    pub redacted: bool,
    pub scope: String,
    pub captured_at: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaptureSessionReport {
    pub capture: CaptureSession,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CaptureEventReport {
    pub event: CaptureEvent,
    #[serde(default)]
    pub deduplicated: bool,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CaptureStatusReport {
    pub project: String,
    pub scope: String,
    pub active_sessions: Vec<CaptureSession>,
    pub recent_events: Vec<CaptureEvent>,
    pub event_count: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CaptureCandidateReport {
    pub capture_id: Option<String>,
    pub events_summarized: usize,
    pub candidates: Vec<CandidateMutationReport>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectReport {
    pub project: String,
    pub scope: String,
    pub entity_count: i64,
    pub relation_count: i64,
    pub observation_count: i64,
    pub decision_count: i64,
    pub active_session_count: i64,
    pub god_nodes: Vec<NodeDegree>,
    pub orphan_entities: Vec<NodeDegree>,
    pub suggested_queries: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeDegree {
    pub id: String,
    pub name: String,
    pub entity_type: String,
    pub scope: String,
    pub degree: i64,
}

#[derive(Debug, Clone)]
pub struct StatusOptions {
    pub project_name: Option<String>,
    pub start_dir: PathBuf,
    pub grafiki_home: Option<PathBuf>,
    pub scope: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusReport {
    pub project: String,
    pub scope: String,
    pub active_sessions: Vec<String>,
    pub active_state: Vec<String>,
    pub recent_decisions: Vec<String>,
    pub recent_events: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct UpsertStateOptions {
    pub project_name: Option<String>,
    pub start_dir: PathBuf,
    pub grafiki_home: Option<PathBuf>,
    pub key: String,
    pub title: String,
    pub status: String,
    pub owner: Option<String>,
    pub details: Option<String>,
    pub blockers: Vec<String>,
    pub depends_on: Vec<String>,
    pub scope: String,
    pub priority: String,
}

#[derive(Debug, Clone)]
pub struct StateListOptions {
    pub project_name: Option<String>,
    pub start_dir: PathBuf,
    pub grafiki_home: Option<PathBuf>,
    pub status: Option<String>,
    pub scope: String,
}

#[derive(Debug, Clone)]
pub struct DeleteStateOptions {
    pub project_name: Option<String>,
    pub start_dir: PathBuf,
    pub grafiki_home: Option<PathBuf>,
    pub key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateReport {
    pub id: String,
    pub key: String,
    pub title: String,
    pub status: String,
    pub scope: String,
    pub priority: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateItem {
    pub key: String,
    pub title: String,
    pub status: String,
    pub priority: String,
    pub owner: Option<String>,
    pub scope: String,
    #[serde(default)]
    pub details: Option<String>,
    #[serde(default)]
    pub blockers: Vec<String>,
    #[serde(default)]
    pub depends_on: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct EventListOptions {
    pub project_name: Option<String>,
    pub start_dir: PathBuf,
    pub grafiki_home: Option<PathBuf>,
    pub scope: String,
    pub since: Option<String>,
    pub limit: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventListReport {
    pub project: String,
    pub events: Vec<EventItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventItem {
    pub id: String,
    pub event_type: String,
    pub source_session: Option<String>,
    pub target_type: String,
    pub target_id: String,
    pub scope: String,
    pub summary: String,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct SessionLogOptions {
    pub project_name: Option<String>,
    pub start_dir: PathBuf,
    pub grafiki_home: Option<PathBuf>,
    pub scope: String,
    pub session_type: Option<String>,
    pub limit: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionLogReport {
    pub project: String,
    pub sessions: Vec<SessionLogItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionLogItem {
    pub id: String,
    pub session_type: String,
    pub status: String,
    pub scope: String,
    pub goal: Option<String>,
    pub summary: Option<String>,
    pub accomplishments: Vec<String>,
    pub remaining: Vec<String>,
    pub files_changed: Vec<String>,
    pub decisions_made: Vec<String>,
    pub entities_touched: Vec<String>,
    pub handoff_context: Option<String>,
    pub parent_session: Option<String>,
    pub child_session: Option<String>,
    pub started_at: String,
    pub ended_at: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AddContextOptions {
    pub project_name: Option<String>,
    pub start_dir: PathBuf,
    pub grafiki_home: Option<PathBuf>,
    pub key: String,
    pub title: String,
    pub category: String,
    pub scope: String,
    pub content: String,
}

#[derive(Debug, Clone)]
pub struct GetContextOptions {
    pub project_name: Option<String>,
    pub start_dir: PathBuf,
    pub grafiki_home: Option<PathBuf>,
    pub key: String,
}

#[derive(Debug, Clone)]
pub struct ContextListOptions {
    pub project_name: Option<String>,
    pub start_dir: PathBuf,
    pub grafiki_home: Option<PathBuf>,
    pub category: Option<String>,
    pub scope: String,
}

#[derive(Debug, Clone)]
pub struct UpdateContextOptions {
    pub project_name: Option<String>,
    pub start_dir: PathBuf,
    pub grafiki_home: Option<PathBuf>,
    pub key: String,
    pub title: Option<String>,
    pub category: Option<String>,
    pub scope: Option<String>,
    pub content: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DeleteContextOptions {
    pub project_name: Option<String>,
    pub start_dir: PathBuf,
    pub grafiki_home: Option<PathBuf>,
    pub key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextReport {
    pub key: String,
    pub title: String,
    pub category: String,
    pub scope: String,
    pub version: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextDocument {
    pub key: String,
    pub title: String,
    pub category: String,
    pub scope: String,
    pub version: i64,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextSummary {
    pub key: String,
    pub title: String,
    pub category: String,
    pub scope: String,
    pub version: i64,
}

#[derive(Debug, Clone)]
pub struct GetMemoryRecordOptions {
    pub project_name: Option<String>,
    pub start_dir: PathBuf,
    pub grafiki_home: Option<PathBuf>,
    pub record_type: String,
    pub id: String,
    pub scope: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetailMetadata {
    pub label: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelatedMemoryRecord {
    pub record_type: String,
    pub id: String,
    pub title: String,
    pub relation: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetailEvent {
    pub id: String,
    pub event_type: String,
    pub summary: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryRecordDetail {
    pub record_type: String,
    pub id: String,
    pub title: String,
    pub scope: String,
    pub body: String,
    pub metadata: Vec<DetailMetadata>,
    pub related: Vec<RelatedMemoryRecord>,
    pub events: Vec<DetailEvent>,
    pub focus_entity_id: Option<String>,
}

pub fn end_session(options: EndSessionOptions) -> Result<EndSessionReport> {
    let status = validate_end_status(options.status.trim())?;
    let (project, mut connection) = resolve_and_open(
        options.project_name,
        options.start_dir,
        options.grafiki_home,
    )?;
    let session_id = match options.session_id {
        Some(id) => ensure_session_exists(&connection, &id)?,
        None => latest_active_session(&connection, &project.project)?
            .ok_or(GrafikiError::NoActiveSession)?,
    };
    let session_scope: String = connection.query_row(
        "SELECT scope FROM sessions WHERE id = ?1",
        [&session_id],
        |row| row.get(0),
    )?;

    let tx = connection.transaction()?;
    let updated = tx.execute(
        "
        UPDATE sessions
        SET status = ?1,
            summary = ?2,
            accomplishments = ?3,
            remaining = ?4,
            files_changed = ?5,
            ended_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
        WHERE id = ?6
        ",
        params![
            status,
            options.summary,
            json_array(&options.accomplishments)?,
            json_array(&options.remaining)?,
            json_array(&options.files_changed)?,
            session_id
        ],
    )?;
    if updated == 0 {
        return Err(GrafikiError::SessionNotFound(session_id));
    }

    tx.execute(
        "
        INSERT INTO events (id, event_type, source_session, target_type, target_id, scope, summary)
        VALUES (?1, 'session_ended', ?2, 'session', ?2, ?3, ?4)
        ",
        params![
            new_ulid(),
            session_id,
            session_scope,
            format!("Ended session with status {status}")
        ],
    )?;
    tx.commit()?;

    Ok(EndSessionReport {
        session_id,
        project: project.project,
        status,
        summary: options.summary,
    })
}

pub fn handoff_session(options: HandoffOptions) -> Result<HandoffReport> {
    let (project, mut connection) = resolve_and_open(
        options.project_name,
        options.start_dir,
        options.grafiki_home,
    )?;
    let parent_session_id = match options.session_id {
        Some(id) => ensure_session_exists(&connection, &id)?,
        // The natural flow is `end` then `handoff` — but `end` clears the
        // active session, so a bare handoff used to dead-end here and demand
        // a copy-pasted ULID. Fall back to the most recently ENDED session:
        // that's the one whose summary/accomplishments the handoff needs.
        None => match latest_active_session(&connection, &project.project)? {
            Some(id) => id,
            None => connection
                .query_row(
                    "
                    SELECT id FROM sessions
                    WHERE project = ?1 AND status = 'completed'
                    ORDER BY ended_at DESC, id DESC
                    LIMIT 1
                    ",
                    params![project.project],
                    |row| row.get::<_, String>(0),
                )
                .optional()?
                .ok_or(GrafikiError::NoActiveSession)?,
        },
    };
    let parent = load_session_snapshot(&connection, &parent_session_id)?;
    let scope = Scope::new(&parent.scope)?;
    let scope_chain = scope.chain().into_vec();
    let recent_decisions = decisions_for_session(&connection, &parent_session_id)?;
    let active_state = status_active_state(&connection, &scope_chain)?;
    let handoff_context = build_handoff_context(&parent, &recent_decisions, &active_state);
    let child_session_id = new_ulid();

    let tx = connection.transaction()?;
    tx.execute(
        "
        UPDATE sessions
        SET status = 'handed-off',
            child_session = ?1,
            handoff_context = ?2,
            ended_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
        WHERE id = ?3
        ",
        params![child_session_id, handoff_context, parent_session_id],
    )?;
    tx.execute(
        "
        INSERT INTO sessions (id, session_type, project, scope, goal, parent_session)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6)
        ",
        params![
            child_session_id,
            parent.session_type,
            parent.project,
            parent.scope,
            parent.goal,
            parent_session_id
        ],
    )?;
    tx.execute(
        "
        INSERT INTO events (id, event_type, source_session, target_type, target_id, scope, summary)
        VALUES (?1, 'session_handoff', ?2, 'session', ?2, ?3, ?4)
        ",
        params![
            new_ulid(),
            parent_session_id,
            parent.scope,
            format!("Handed off session to {}", child_session_id)
        ],
    )?;
    tx.execute(
        "
        INSERT INTO events (id, event_type, source_session, target_type, target_id, scope, summary)
        VALUES (?1, 'session_started', ?2, 'session', ?2, ?3, ?4)
        ",
        params![
            new_ulid(),
            child_session_id,
            parent.scope,
            format!("Started child session from handoff {}", parent_session_id)
        ],
    )?;
    tx.commit()?;

    Ok(HandoffReport {
        parent_session_id,
        child_session_id,
        project: project.project,
        scope: scope.as_str().to_owned(),
        handoff_context,
    })
}

pub fn update_session(options: UpdateSessionOptions) -> Result<SessionLogItem> {
    let session_type = options
        .session_type
        .as_deref()
        .map(|session_type| validate_session_type_for_filter(session_type.trim()))
        .transpose()?;
    let status = options
        .status
        .as_deref()
        .map(|status| validate_session_status(status.trim()))
        .transpose()?;
    let scope = options.scope.as_deref().map(Scope::new).transpose()?;
    let (project, mut connection) = resolve_and_open(
        options.project_name,
        options.start_dir,
        options.grafiki_home,
    )?;
    let project_name = project.project.clone();
    let existing = load_session_log_item(&connection, &options.id, &project_name)?;
    let session_type = session_type.unwrap_or(existing.session_type);
    let status = status.unwrap_or(existing.status);
    let scope = scope
        .map(|scope| scope.as_str().to_owned())
        .unwrap_or(existing.scope);
    let goal = options.goal.or(existing.goal);
    let summary = options.summary.or(existing.summary);
    let accomplishments = options
        .accomplishments
        .map(|items| json_array(&items))
        .transpose()?;
    let remaining = options
        .remaining
        .map(|items| json_array(&items))
        .transpose()?;
    let files_changed = options
        .files_changed
        .map(|items| json_array(&items))
        .transpose()?;

    let tx = connection.transaction()?;
    tx.execute(
        "
        UPDATE sessions
        SET session_type = ?1,
            status = ?2,
            scope = ?3,
            goal = ?4,
            summary = ?5,
            accomplishments = COALESCE(?6, accomplishments),
            remaining = COALESCE(?7, remaining),
            files_changed = COALESCE(?8, files_changed),
            ended_at = CASE
                WHEN ?2 = 'active' THEN NULL
                WHEN ended_at IS NULL THEN strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
                ELSE ended_at
            END
        WHERE id = ?9 AND project = ?10
        ",
        params![
            session_type,
            status,
            scope,
            goal,
            summary,
            accomplishments,
            remaining,
            files_changed,
            &options.id,
            &project_name
        ],
    )?;
    tx.commit()?;

    load_session_log_item(&connection, &options.id, &project_name)
}

pub fn log_decision(options: LogDecisionOptions) -> Result<DecisionReport> {
    let scope = Scope::new(options.scope)?;
    let (project, mut connection) = resolve_and_open(
        options.project_name,
        options.start_dir,
        options.grafiki_home,
    )?;
    let active_session = latest_active_session(&connection, &project.project)?;
    let decision_id = new_ulid();
    let title = clamp_field(options.title.trim(), MAX_TITLE_CHARS);
    let reasoning = options
        .reasoning
        .map(|value| clamp_field(&value, MAX_BODY_CHARS));
    let decision_embedding_text = format!("{} {}", title, reasoning.as_deref().unwrap_or(""));

    let tx = connection.transaction()?;
    tx.execute(
        "
        INSERT INTO decisions (id, title, reasoning, alternatives, scope, decided_in, tags)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        ",
        params![
            decision_id,
            title,
            reasoning,
            json_array(&options.alternatives)?,
            scope.as_str(),
            active_session,
            json_array(&options.tags)?
        ],
    )?;
    enqueue_embedding_job(
        &tx,
        "decision",
        &decision_id,
        scope.as_str(),
        &decision_embedding_text,
    )?;

    if let Some(superseded_id) = &options.supersedes {
        // Only supersede a decision that's still in play. Overwriting a
        // 'revoked' status with 'superseded' would resurrect a decision the
        // user explicitly killed (superseded is searchable-by-status; revoked
        // is a hard stop).
        tx.execute(
            "
            UPDATE decisions
            SET status = 'superseded',
                superseded_by = ?1,
                updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
            WHERE id = ?2 AND status != 'revoked'
            ",
            params![decision_id, superseded_id],
        )?;
        tx.execute(
            "
            INSERT INTO events (id, event_type, source_session, target_type, target_id, scope, summary)
            VALUES (?1, 'decision_superseded', ?2, 'decision', ?3, ?4, ?5)
            ",
            params![
                new_ulid(),
                active_session,
                superseded_id,
                scope.as_str(),
                format!("Superseded decision {superseded_id} with {decision_id}")
            ],
        )?;
    }

    tx.execute(
        "
        INSERT INTO events (id, event_type, source_session, target_type, target_id, scope, summary)
        VALUES (?1, 'decision_logged', ?2, 'decision', ?3, ?4, ?5)
        ",
        params![
            new_ulid(),
            active_session,
            decision_id,
            scope.as_str(),
            format!("Logged decision: {title}")
        ],
    )?;
    tx.commit()?;

    Ok(DecisionReport {
        decision_id,
        project: project.project,
        title,
        scope: scope.as_str().to_owned(),
        supersedes: options.supersedes,
    })
}

pub fn list_decisions(options: DecisionListOptions) -> Result<Vec<DecisionItem>> {
    let scope = Scope::new(options.scope)?;
    let scope_chain = browse_scope_chain(&scope);
    let (_project, connection) = resolve_and_open(
        options.project_name,
        options.start_dir,
        options.grafiki_home,
    )?;

    match options.status {
        Some(status) => {
            let status = validate_decision_status(status.trim())?;
            let sql = scoped_query(
                "
                SELECT id, title, status, scope, reasoning
                FROM decisions
                WHERE status = ? AND scope IN ({scopes})
                ORDER BY updated_at DESC, created_at DESC, id DESC
                ",
                scope_chain.len(),
            );
            let mut params: Vec<&dyn rusqlite::ToSql> = vec![&status];
            params.extend(
                scope_chain
                    .iter()
                    .map(|scope| scope as &dyn rusqlite::ToSql),
            );
            let mut statement = connection.prepare(&sql)?;
            let rows = statement.query_map(params.as_slice(), decision_item_from_row)?;
            collect_rows(rows)
        }
        // Superseded decisions are obsolete — a retrieval that includes them
        // surfaces the old rule next to the current one. They're excluded here
        // and from search/embedding/count below; an explicit `status` filter
        // (the arm above) can still list them.
        None => query_scoped_rows(
            &connection,
            "
            SELECT id, title, status, scope, reasoning
            FROM decisions
            WHERE status NOT IN ('revoked', 'superseded') AND scope IN ({scopes})
            ORDER BY updated_at DESC, created_at DESC, id DESC
            ",
            &scope_chain,
            decision_item_from_row,
        ),
    }
}

pub fn update_decision(options: UpdateDecisionOptions) -> Result<DecisionItem> {
    let scope = options.scope.as_deref().map(Scope::new).transpose()?;
    let status = options
        .status
        .as_deref()
        .map(|status| validate_decision_status(status.trim()))
        .transpose()?;
    let (project, mut connection) = resolve_and_open(
        options.project_name,
        options.start_dir,
        options.grafiki_home,
    )?;
    let active_session = latest_active_session(&connection, &project.project)?;
    let existing = load_decision_item(&connection, &options.id)?;
    let title = options.title.unwrap_or(existing.title);
    let reasoning = options.reasoning.or(existing.reasoning);
    let scope = scope
        .map(|scope| scope.as_str().to_owned())
        .unwrap_or(existing.scope);
    let status = status.unwrap_or(existing.status);
    let decision_embedding_text =
        format!("{} {}", title.trim(), reasoning.as_deref().unwrap_or(""));

    let tx = connection.transaction()?;
    tx.execute(
        "
        UPDATE decisions
        SET title = ?1,
            reasoning = ?2,
            scope = ?3,
            status = ?4,
            updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
        WHERE id = ?5
        ",
        params![title, reasoning, scope, status, options.id],
    )?;
    enqueue_embedding_job(
        &tx,
        "decision",
        &options.id,
        &scope,
        &decision_embedding_text,
    )?;
    tx.execute(
        "
        INSERT INTO events (id, event_type, source_session, target_type, target_id, scope, summary)
        VALUES (?1, 'decision_logged', ?2, 'decision', ?3, ?4, ?5)
        ",
        params![
            new_ulid(),
            active_session,
            options.id,
            scope,
            format!("Updated decision: {}", title.trim())
        ],
    )?;
    tx.commit()?;

    load_decision_item(&connection, &options.id)
}

pub fn delete_decision(options: DeleteDecisionOptions) -> Result<DecisionItem> {
    let (_project, mut connection) = resolve_and_open(
        options.project_name,
        options.start_dir,
        options.grafiki_home,
    )?;
    let existing = load_decision_item(&connection, &options.id)?;
    let tx = connection.transaction()?;
    delete_embedding_records(&tx, "decision", &options.id)?;
    tx.execute(
        "UPDATE decisions
         SET status = 'revoked', updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
         WHERE id = ?1",
        [&options.id],
    )?;
    tx.commit()?;
    Ok(existing)
}

#[derive(Debug, Clone)]
struct SessionSnapshot {
    id: String,
    session_type: String,
    project: String,
    scope: String,
    goal: Option<String>,
    summary: Option<String>,
    accomplishments: Option<String>,
    remaining: Option<String>,
    files_changed: Option<String>,
}

fn load_session_snapshot(connection: &Connection, session_id: &str) -> Result<SessionSnapshot> {
    connection
        .query_row(
            "
            SELECT id, session_type, project, scope, goal, summary, accomplishments, remaining, files_changed
            FROM sessions
            WHERE id = ?1
            ",
            [session_id],
            |row| {
                Ok(SessionSnapshot {
                    id: row.get(0)?,
                    session_type: row.get(1)?,
                    project: row.get(2)?,
                    scope: row.get(3)?,
                    goal: row.get(4)?,
                    summary: row.get(5)?,
                    accomplishments: row.get(6)?,
                    remaining: row.get(7)?,
                    files_changed: row.get(8)?,
                })
            },
        )
        .optional()?
        .ok_or_else(|| GrafikiError::SessionNotFound(session_id.to_owned()))
}

fn decisions_for_session(connection: &Connection, session_id: &str) -> Result<Vec<String>> {
    let mut statement = connection.prepare(
        "
        SELECT id, title
        FROM decisions
        WHERE decided_in = ?1
        ORDER BY created_at DESC
        LIMIT 10
        ",
    )?;
    let rows = statement.query_map([session_id], |row| {
        let id: String = row.get(0)?;
        let title: String = row.get(1)?;
        Ok(format!("{id}: {title}"))
    })?;
    collect_rows(rows)
}

fn build_handoff_context(
    session: &SessionSnapshot,
    decisions: &[String],
    active_state: &[String],
) -> String {
    let mut context = String::new();
    context.push_str("# Grafiki Handoff\n\n");
    context.push_str(&format!("- Parent session: {}\n", session.id));
    context.push_str(&format!("- Tool: {}\n", session.session_type));
    context.push_str(&format!("- Project: {}\n", session.project));
    context.push_str(&format!("- Scope: {}\n", display_scope(&session.scope)));
    context.push_str(&format!(
        "- Goal: {}\n",
        session.goal.as_deref().unwrap_or("No goal recorded")
    ));

    push_optional_section(&mut context, "Summary", session.summary.as_deref());
    push_json_list_section(
        &mut context,
        "Accomplishments",
        session.accomplishments.as_deref(),
    );
    push_json_list_section(&mut context, "Remaining", session.remaining.as_deref());
    push_json_list_section(
        &mut context,
        "Files Changed",
        session.files_changed.as_deref(),
    );
    push_plain_list_section(&mut context, "Decisions Made", decisions);
    push_plain_list_section(&mut context, "Relevant Active Work", active_state);
    context.push_str(
        "\nStart from the remaining items and verify current repository state before editing.\n",
    );
    context
}

fn push_optional_section(context: &mut String, title: &str, value: Option<&str>) {
    context.push_str(&format!("\n## {title}\n"));
    match value {
        Some(value) if !value.trim().is_empty() => {
            context.push_str(value.trim());
            context.push('\n');
        }
        _ => context.push_str("- None recorded.\n"),
    }
}

fn push_json_list_section(context: &mut String, title: &str, value: Option<&str>) {
    let items = value
        .and_then(|value| serde_json::from_str::<Vec<String>>(value).ok())
        .unwrap_or_default();
    push_plain_list_section(context, title, &items);
}

fn push_plain_list_section(context: &mut String, title: &str, items: &[String]) {
    context.push_str(&format!("\n## {title}\n"));
    if items.is_empty() {
        context.push_str("- None recorded.\n");
    } else {
        for item in items {
            context.push_str("- ");
            context.push_str(item);
            context.push('\n');
        }
    }
}

pub fn save_entity(options: SaveEntityOptions) -> Result<SaveEntityReport> {
    let entity_type = validate_entity_type(options.entity_type.trim())?;
    let category = validate_observation_category(options.category.trim())?;
    let scope = Scope::new(options.scope)?;
    let (project, mut connection) = resolve_and_open(
        options.project_name,
        options.start_dir,
        options.grafiki_home,
    )?;
    let active_session = latest_active_session(&connection, &project.project)?;
    let entity_name = clamp_field(options.name.trim(), MAX_TITLE_CHARS);
    if entity_name.is_empty() {
        return Err(GrafikiError::InvalidCandidate(
            "entity name is required".to_owned(),
        ));
    }
    let entity_id = entity_id_for_name(&connection, &entity_name)?;
    let entity_embedding_text = format!("{entity_name} {entity_type}");
    let created = entity_exists(&connection, &entity_id)?.not();
    let relation = options
        .relate
        .as_deref()
        .map(parse_relation_spec)
        .transpose()?;

    if let Some((target_id, _)) = &relation {
        if !entity_exists(&connection, target_id)? {
            return Err(GrafikiError::EntityNotFound(target_id.clone()));
        }
    }

    let tx = connection.transaction()?;
    tx.execute(
        "
        INSERT INTO entities (id, name, entity_type, scope)
        VALUES (?1, ?2, ?3, ?4)
        ON CONFLICT(id) DO UPDATE SET
            name = excluded.name,
            entity_type = excluded.entity_type,
            scope = excluded.scope,
            retired_at = NULL,
            updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
        ",
        params![entity_id, entity_name, entity_type, scope.as_str()],
    )?;
    enqueue_embedding_job(
        &tx,
        "entity",
        &entity_id,
        scope.as_str(),
        &entity_embedding_text,
    )?;

    tx.execute(
        "
        INSERT INTO events (id, event_type, source_session, target_type, target_id, scope, summary)
        VALUES (?1, ?2, ?3, 'entity', ?4, ?5, ?6)
        ",
        params![
            new_ulid(),
            if created {
                "entity_created"
            } else {
                "entity_updated"
            },
            active_session,
            entity_id,
            scope.as_str(),
            if created {
                format!("Created entity {}", entity_id)
            } else {
                format!("Updated entity {}", entity_id)
            }
        ],
    )?;

    let observation_id = match options.observe {
        Some(content) => {
            let observation_id = new_ulid();
            let observation_content = clamp_field(content.trim(), MAX_BODY_CHARS);
            tx.execute(
                "
                INSERT INTO observations (id, entity_id, content, category, source)
                VALUES (?1, ?2, ?3, ?4, ?5)
                ",
                params![
                    observation_id,
                    entity_id,
                    observation_content,
                    category,
                    active_session.as_ref().map(|id| format!("session:{id}"))
                ],
            )?;
            enqueue_embedding_job(
                &tx,
                "observation",
                &observation_id,
                scope.as_str(),
                &observation_content,
            )?;
            tx.execute(
                "
                INSERT INTO events (id, event_type, source_session, target_type, target_id, scope, summary)
                VALUES (?1, 'observation_added', ?2, 'observation', ?3, ?4, ?5)
                ",
                params![
                    new_ulid(),
                    active_session,
                    observation_id,
                    scope.as_str(),
                    format!("Added observation to {}", entity_id)
                ],
            )?;
            Some(observation_id)
        }
        None => None,
    };

    let relation_id = match relation {
        Some((target_id, relation_type)) => {
            let proposed_relation_id = new_ulid();
            // `RETURNING id` yields the existing row's id on conflict. Returning
            // the newly generated (discarded) id created dangling API reports and
            // audit targets whenever the relation was an upsert.
            let relation_id: String = tx.query_row(
                "
                INSERT INTO relations (id, from_entity, to_entity, relation, source)
                VALUES (?1, ?2, ?3, ?4, ?5)
                ON CONFLICT(from_entity, to_entity, relation) DO UPDATE SET
                    source = excluded.source,
                    valid_to = NULL
                RETURNING id
                ",
                params![
                    proposed_relation_id,
                    entity_id,
                    target_id,
                    relation_type,
                    active_session.as_ref().map(|id| format!("session:{id}"))
                ],
                |row| row.get(0),
            )?;
            tx.execute(
                "
                INSERT INTO events (id, event_type, source_session, target_type, target_id, scope, summary)
                VALUES (?1, 'relation_created', ?2, 'relation', ?3, ?4, ?5)
                ",
                params![
                    new_ulid(),
                    active_session,
                    relation_id,
                    scope.as_str(),
                    format!("Related {} to {}", entity_id, target_id)
                ],
            )?;
            Some(relation_id)
        }
        None => None,
    };

    tx.commit()?;

    Ok(SaveEntityReport {
        entity_id,
        project: project.project,
        created,
        observation_id,
        relation_id,
    })
}

pub fn list_entities(options: EntityListOptions) -> Result<Vec<GraphEntity>> {
    let scope = Scope::new(options.scope)?;
    let scope_chain = browse_scope_chain(&scope);
    let (_project, connection) = resolve_and_open(
        options.project_name,
        options.start_dir,
        options.grafiki_home,
    )?;

    match options.entity_type {
        Some(entity_type) => {
            let entity_type = validate_entity_type(entity_type.trim())?;
            let sql = scoped_query(
                "
                SELECT id, name, entity_type, scope
                FROM entities
                WHERE retired_at IS NULL AND entity_type = ? AND scope IN ({scopes})
                ORDER BY updated_at DESC, id ASC
                ",
                scope_chain.len(),
            );
            let mut params: Vec<&dyn rusqlite::ToSql> = vec![&entity_type];
            params.extend(
                scope_chain
                    .iter()
                    .map(|scope| scope as &dyn rusqlite::ToSql),
            );
            let mut statement = connection.prepare(&sql)?;
            let rows = statement.query_map(params.as_slice(), graph_entity_from_row)?;
            collect_rows(rows)
        }
        None => query_scoped_rows(
            &connection,
            "
            SELECT id, name, entity_type, scope
            FROM entities
            WHERE retired_at IS NULL AND scope IN ({scopes})
            ORDER BY updated_at DESC, id ASC
            ",
            &scope_chain,
            graph_entity_from_row,
        ),
    }
}

pub fn update_entity(options: UpdateEntityOptions) -> Result<GraphEntity> {
    let scope = options.scope.as_deref().map(Scope::new).transpose()?;
    let entity_type = options
        .entity_type
        .as_deref()
        .map(|entity_type| validate_entity_type(entity_type.trim()))
        .transpose()?;
    let (project, mut connection) = resolve_and_open(
        options.project_name,
        options.start_dir,
        options.grafiki_home,
    )?;
    let active_session = latest_active_session(&connection, &project.project)?;
    let existing = load_graph_entity(&connection, &options.id)?;
    let name = options.name.unwrap_or(existing.name);
    let entity_type = entity_type.unwrap_or(existing.entity_type);
    let scope = scope
        .map(|scope| scope.as_str().to_owned())
        .unwrap_or(existing.scope);
    let entity_embedding_text = format!("{} {}", name.trim(), entity_type);

    let tx = connection.transaction()?;
    tx.execute(
        "
        UPDATE entities
        SET name = ?1,
            entity_type = ?2,
            scope = ?3,
            updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
        WHERE id = ?4
        ",
        params![name, entity_type, scope, options.id],
    )?;
    enqueue_embedding_job(&tx, "entity", &options.id, &scope, &entity_embedding_text)?;
    tx.execute(
        "
        INSERT INTO events (id, event_type, source_session, target_type, target_id, scope, summary)
        VALUES (?1, 'entity_updated', ?2, 'entity', ?3, ?4, ?5)
        ",
        params![
            new_ulid(),
            active_session,
            options.id,
            scope,
            format!("Updated entity {}", name.trim())
        ],
    )?;
    tx.commit()?;

    load_graph_entity(&connection, &options.id)
}

pub fn delete_entity(options: DeleteEntityOptions) -> Result<GraphEntity> {
    let (_project, mut connection) = resolve_and_open(
        options.project_name,
        options.start_dir,
        options.grafiki_home,
    )?;
    let existing = load_graph_entity(&connection, &options.id)?;
    let observation_ids = observation_ids_for_entity(&connection, &options.id)?;
    let tx = connection.transaction()?;
    for observation_id in observation_ids {
        delete_embedding_records(&tx, "observation", &observation_id)?;
    }
    delete_embedding_records(&tx, "entity", &options.id)?;
    tx.execute(
        "UPDATE observations
         SET valid_to = COALESCE(valid_to, strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
         WHERE entity_id = ?1",
        [&options.id],
    )?;
    tx.execute(
        "UPDATE relations
         SET valid_to = COALESCE(valid_to, strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
         WHERE from_entity = ?1 OR to_entity = ?1",
        [&options.id],
    )?;
    tx.execute(
        "UPDATE entities
         SET retired_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now'),
             updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
         WHERE id = ?1",
        [&options.id],
    )?;
    tx.commit()?;
    Ok(existing)
}

pub fn list_observations(options: ObservationListOptions) -> Result<Vec<ObservationItem>> {
    let scope = Scope::new(options.scope)?;
    let scope_chain = browse_scope_chain(&scope);
    let (_project, connection) = resolve_and_open(
        options.project_name,
        options.start_dir,
        options.grafiki_home,
    )?;

    match options.category {
        Some(category) => {
            let category = validate_observation_category(category.trim())?;
            let sql = scoped_query(
                "
                SELECT o.id, o.entity_id, e.name, o.content, o.category, o.confidence, e.scope
                FROM observations o
                JOIN entities e ON e.id = o.entity_id
                WHERE o.valid_to IS NULL AND e.retired_at IS NULL
                  AND o.category = ? AND e.scope IN ({scopes})
                ORDER BY o.created_at DESC, o.id DESC
                ",
                scope_chain.len(),
            );
            let mut params: Vec<&dyn rusqlite::ToSql> = vec![&category];
            params.extend(
                scope_chain
                    .iter()
                    .map(|scope| scope as &dyn rusqlite::ToSql),
            );
            let mut statement = connection.prepare(&sql)?;
            let rows = statement.query_map(params.as_slice(), observation_item_from_row)?;
            collect_rows(rows)
        }
        None => query_scoped_rows(
            &connection,
            "
            SELECT o.id, o.entity_id, e.name, o.content, o.category, o.confidence, e.scope
            FROM observations o
            JOIN entities e ON e.id = o.entity_id
            WHERE o.valid_to IS NULL AND e.retired_at IS NULL AND e.scope IN ({scopes})
            ORDER BY o.created_at DESC, o.id DESC
            ",
            &scope_chain,
            observation_item_from_row,
        ),
    }
}

pub fn update_observation(options: UpdateObservationOptions) -> Result<ObservationItem> {
    let category = options
        .category
        .as_deref()
        .map(|category| validate_observation_category(category.trim()))
        .transpose()?;
    let (project, mut connection) = resolve_and_open(
        options.project_name,
        options.start_dir,
        options.grafiki_home,
    )?;
    let active_session = latest_active_session(&connection, &project.project)?;
    let existing = load_observation_item(&connection, &options.id)?;
    let content = options
        .content
        .map(|value| clamp_field(&value, MAX_BODY_CHARS))
        .unwrap_or(existing.content);
    let category = category.unwrap_or(existing.category);

    let tx = connection.transaction()?;
    tx.execute(
        "
        UPDATE observations
        SET content = ?1,
            category = ?2
        WHERE id = ?3 AND valid_to IS NULL
        ",
        params![content, category, options.id],
    )?;
    enqueue_embedding_job(&tx, "observation", &options.id, &existing.scope, &content)?;
    tx.execute(
        "
        INSERT INTO events (id, event_type, source_session, target_type, target_id, scope, summary)
        VALUES (?1, 'observation_added', ?2, 'observation', ?3, ?4, ?5)
        ",
        params![
            new_ulid(),
            active_session,
            options.id,
            existing.scope,
            format!("Updated observation on {}", existing.entity_id)
        ],
    )?;
    tx.commit()?;

    load_observation_item(&connection, &options.id)
}

pub fn delete_observation(options: DeleteObservationOptions) -> Result<ObservationItem> {
    let (project, mut connection) = resolve_and_open(
        options.project_name,
        options.start_dir,
        options.grafiki_home,
    )?;
    let active_session = latest_active_session(&connection, &project.project)?;
    let existing = load_observation_item(&connection, &options.id)?;

    let tx = connection.transaction()?;
    tx.execute(
        "
        UPDATE observations
        SET valid_to = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
        WHERE id = ?1 AND valid_to IS NULL
        ",
        [&options.id],
    )?;
    delete_embedding_records(&tx, "observation", &options.id)?;
    tx.execute(
        "
        INSERT INTO events (id, event_type, source_session, target_type, target_id, scope, summary)
        VALUES (?1, 'observation_invalidated', ?2, 'observation', ?3, ?4, ?5)
        ",
        params![
            new_ulid(),
            active_session,
            options.id,
            existing.scope,
            format!("Invalidated observation on {}", existing.entity_id)
        ],
    )?;
    tx.commit()?;
    Ok(existing)
}

pub fn list_relations(options: RelationListOptions) -> Result<Vec<GraphRelation>> {
    let scope = Scope::new(options.scope)?;
    let scope_chain = scope.chain().into_vec();
    let (_project, connection) = resolve_and_open(
        options.project_name,
        options.start_dir,
        options.grafiki_home,
    )?;

    let Some(relation) = options.relation else {
        return export_relations(&connection, &scope_chain);
    };

    let relation = validate_relation_type(relation.trim())?;
    let placeholders = placeholders(scope_chain.len());
    let sql = format!(
        "
        SELECT r.id, r.from_entity, r.to_entity, r.relation, r.weight, r.confidence, r.source_type, r.source
        FROM relations r
        JOIN entities f ON f.id = r.from_entity
        JOIN entities t ON t.id = r.to_entity
        WHERE r.valid_to IS NULL
          AND r.relation = ?
          AND f.retired_at IS NULL
          AND t.retired_at IS NULL
          AND f.scope IN ({placeholders})
          AND t.scope IN ({placeholders})
        ORDER BY r.created_at DESC, r.id DESC
        "
    );
    let repeated = repeat_scope_chain(&scope_chain, 2);
    let mut params: Vec<&dyn rusqlite::ToSql> = vec![&relation];
    params.extend(repeated.iter().map(|scope| scope as &dyn rusqlite::ToSql));
    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map(params.as_slice(), graph_relation_from_row)?;
    collect_rows(rows)
}

pub fn update_relation(options: UpdateRelationOptions) -> Result<GraphRelation> {
    let relation = options
        .relation
        .as_deref()
        .map(|relation| validate_relation_type(relation.trim()))
        .transpose()?;
    let source_type = options
        .source_type
        .as_deref()
        .map(validate_relation_source_type)
        .transpose()?;
    let confidence = options
        .confidence
        .map(validate_relation_confidence)
        .transpose()?;
    let (project, mut connection) = resolve_and_open(
        options.project_name,
        options.start_dir,
        options.grafiki_home,
    )?;
    let active_session = latest_active_session(&connection, &project.project)?;
    let existing = load_graph_relation(&connection, &options.id)?;
    let from_entity = load_graph_entity(&connection, &existing.from_entity)?;
    let relation = relation.unwrap_or(existing.relation);
    let weight = options.weight.unwrap_or(existing.weight);
    let confidence = confidence.unwrap_or(existing.confidence);
    let source_type = source_type.unwrap_or(existing.source_type);
    let source = options.source.or(existing.source);

    let tx = connection.transaction()?;
    tx.execute(
        "
        UPDATE relations
        SET relation = ?1,
            weight = ?2,
            confidence = ?3,
            source_type = ?4,
            source = ?5,
            valid_to = NULL
        WHERE id = ?6 AND valid_to IS NULL
        ",
        params![
            relation,
            weight,
            confidence,
            source_type,
            source,
            options.id
        ],
    )?;
    tx.execute(
        "
        INSERT INTO events (id, event_type, source_session, target_type, target_id, scope, summary)
        VALUES (?1, 'relation_created', ?2, 'relation', ?3, ?4, ?5)
        ",
        params![
            new_ulid(),
            active_session,
            options.id,
            from_entity.scope,
            format!(
                "Updated relation {} {} {}",
                existing.from_entity, relation, existing.to_entity
            )
        ],
    )?;
    tx.commit()?;

    load_graph_relation(&connection, &options.id)
}

pub fn delete_relation(options: DeleteRelationOptions) -> Result<GraphRelation> {
    let (project, mut connection) = resolve_and_open(
        options.project_name,
        options.start_dir,
        options.grafiki_home,
    )?;
    let active_session = latest_active_session(&connection, &project.project)?;
    let existing = load_graph_relation(&connection, &options.id)?;
    let from_entity = load_graph_entity(&connection, &existing.from_entity)?;

    let tx = connection.transaction()?;
    tx.execute(
        "
        UPDATE relations
        SET valid_to = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
        WHERE id = ?1 AND valid_to IS NULL
        ",
        [&options.id],
    )?;
    tx.execute(
        "
        INSERT INTO events (id, event_type, source_session, target_type, target_id, scope, summary)
        VALUES (?1, 'relation_removed', ?2, 'relation', ?3, ?4, ?5)
        ",
        params![
            new_ulid(),
            active_session,
            options.id,
            from_entity.scope,
            format!(
                "Removed relation {} {} {}",
                existing.from_entity, existing.relation, existing.to_entity
            )
        ],
    )?;
    tx.commit()?;

    Ok(existing)
}

pub fn search_memory(options: SearchMemoryOptions) -> Result<SearchReport> {
    let scope = Scope::new(options.scope)?;
    let scope_chain = scope.chain().into_vec();
    let (project, connection) = resolve_and_open(
        options.project_name,
        options.start_dir,
        options.grafiki_home,
    )?;
    let limit = options.limit.clamp(1, 100);
    let record_type = options.record_type.as_str();
    // Fused/reranked modes pull a wider candidate pool (3×) so fusion/reranking has
    // material to reorder. Capped at 100 to bound work (esp. the cross-encoder),
    // so the 3× widening only fully holds for limit ≤ 33; larger limits taper to 1×.
    let candidate_limit = if matches!(
        options.mode,
        SearchMode::Hybrid | SearchMode::Graph | SearchMode::Rerank
    ) {
        limit.saturating_mul(3).max(limit).min(100)
    } else {
        limit
    };
    let keyword_results = search_keyword_memory(
        &connection,
        &options.query,
        record_type,
        &scope_chain,
        candidate_limit,
    )?;
    let (semantic_results, semantic_error) = if options.mode == SearchMode::Keyword {
        (Vec::new(), None)
    } else {
        match search_semantic_memory(
            &connection,
            &options.query,
            record_type,
            &scope_chain,
            candidate_limit,
        ) {
            Ok(results) => (results, None),
            Err(GrafikiError::Embedding(error)) => (Vec::new(), Some(error)),
            Err(error) => return Err(error),
        }
    };
    let semantic_available = !semantic_results.is_empty();

    // M-E1/M-E2 temporal boost (opt-in; empty when temporal_weight == 0 ⇒ fusion unchanged).
    // Precomputed over the lexical+dense candidate union — complete for the Hybrid and Rerank arms
    // (which add no further candidates). The Graph arm recomputes over a wider union that also
    // includes its PPR-discovered records (below) so they are boost-eligible too.
    let temporal_boost = if options.temporal_weight > 0.0 {
        let candidates: Vec<(String, String)> = keyword_results
            .iter()
            .chain(semantic_results.iter())
            .map(|r| (r.record_type.clone(), r.id.clone()))
            .collect();
        temporal_boosts(
            &connection,
            &scope_chain,
            &candidates,
            options.temporal_weight,
        )?
    } else {
        HashMap::new()
    };

    let (mut results, fallback) = match options.mode {
        SearchMode::Keyword => (keyword_results.into_iter().take(limit).collect(), None),
        SearchMode::Semantic if semantic_available => (semantic_results, None),
        SearchMode::Semantic if semantic_error.is_some() => (
            keyword_results.into_iter().take(limit).collect(),
            Some(format!(
                "Semantic search is unavailable ({}); returned keyword results.",
                semantic_error.unwrap()
            )),
        ),
        SearchMode::Semantic => (
            keyword_results,
            Some(
                "Semantic search has no indexed vectors yet; returned keyword results. Run `grafiki embeddings rebuild`."
                    .to_owned(),
            ),
        ),
        SearchMode::Hybrid if semantic_available => (
            hybrid_search_results(
                &options.query,
                keyword_results,
                semantic_results,
                Vec::new(),
                limit,
                &temporal_boost,
            ),
            None,
        ),
        SearchMode::Hybrid if semantic_error.is_some() => (
            keyword_results.into_iter().take(limit).collect(),
            Some(format!(
                "Hybrid search is unavailable ({}); returned keyword results.",
                semantic_error.unwrap()
            )),
        ),
        SearchMode::Hybrid => (
            keyword_results.into_iter().take(limit).collect(),
            Some(
                "Hybrid search has no indexed vectors yet; returned keyword results. Run `grafiki embeddings rebuild`."
                    .to_owned(),
            ),
        ),
        // Graph: PPR over relations, seeded from the lexical/dense hits, fused with
        // keyword (+ semantic when available). Model-free with keyword seeds alone.
        SearchMode::Graph => {
            // Best-effort, but never silent: a graph-arm error (e.g. the relations
            // table missing before a migration) falls back to keyword/semantic with
            // a surfaced message rather than a hidden empty arm.
            let (graph_results, graph_error) = match graph_search_results(
                &connection,
                &scope_chain,
                &keyword_results,
                &semantic_results,
                candidate_limit,
            ) {
                Ok(results) => (results, None),
                Err(error) => (
                    Vec::new(),
                    Some(format!(
                        "Graph arm unavailable ({error}); returned keyword/semantic results."
                    )),
                ),
            };
            let fallback = graph_error.or_else(|| {
                if semantic_available {
                    None
                } else {
                    Some(
                        "Graph search used keyword seeds only (no semantic vectors yet)."
                            .to_owned(),
                    )
                }
            });
            // Recompute the boost over the FULL union incl. PPR-discovered graph records, so a
            // recent/reused multi-hop observation is boost-eligible (not just lexical/dense hits).
            let graph_boost = if options.temporal_weight > 0.0 {
                let candidates: Vec<(String, String)> = keyword_results
                    .iter()
                    .chain(semantic_results.iter())
                    .chain(graph_results.iter())
                    .map(|r| (r.record_type.clone(), r.id.clone()))
                    .collect();
                temporal_boosts(
                    &connection,
                    &scope_chain,
                    &candidates,
                    options.temporal_weight,
                )?
            } else {
                HashMap::new()
            };
            (
                hybrid_search_results(
                    &options.query,
                    keyword_results,
                    semantic_results,
                    graph_results,
                    limit,
                    &graph_boost,
                ),
                fallback,
            )
        }
        // Rerank: fuse keyword + semantic, then a cross-encoder reorders the wide
        // candidate pool down to `limit`. The reranker needs the model; without it
        // (or on error) the fused order is returned with a surfaced note.
        SearchMode::Rerank => {
            let fused = hybrid_search_results(
                &options.query,
                keyword_results,
                semantic_results,
                Vec::new(),
                candidate_limit,
                &temporal_boost,
            );
            let (reranked, rerank_note) = rerank_results(&options.query, fused, limit);
            // Describe the candidate pool honestly: surface a real embedding error,
            // and don't claim "fused results" when the pool was keyword-only.
            let pool_note = if semantic_available {
                None
            } else if let Some(error) = &semantic_error {
                Some(format!("candidate pool was keyword-only (semantic arm unavailable: {error})"))
            } else {
                Some("candidate pool was keyword-only (no semantic vectors yet)".to_owned())
            };
            let fallback = match (rerank_note, pool_note) {
                (Some(rerank), Some(pool)) => Some(format!("{rerank} ({pool})")),
                (Some(rerank), None) => Some(rerank),
                (None, Some(pool)) => Some(format!("Rerank applied; {pool}.")),
                (None, None) => None,
            };
            (reranked, fallback)
        }
    };

    attach_search_evidence(&connection, &mut results)?;

    Ok(SearchReport {
        project: project.project,
        query: options.query,
        mode: options.mode,
        semantic_available,
        fallback,
        results,
    })
}

pub fn ask_memory(options: AskMemoryOptions) -> Result<AgentMemoryBriefing> {
    let started = Instant::now();
    let question = options.question.trim().to_owned();
    if question.is_empty() {
        return Err(GrafikiError::InvalidCandidate(
            "ask question is required".to_owned(),
        ));
    }

    let scope = options.scope.clone();
    let agent = options
        .agent
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "unknown".to_owned());
    let limit = options.limit.clamp(1, 20);
    let mut status = get_status(StatusOptions {
        project_name: options.project_name.clone(),
        start_dir: options.start_dir.clone(),
        grafiki_home: options.grafiki_home.clone(),
        scope: scope.clone(),
    })?;
    let mut search = search_memory(SearchMemoryOptions {
        project_name: options.project_name.clone(),
        start_dir: options.start_dir.clone(),
        grafiki_home: options.grafiki_home.clone(),
        query: question.clone(),
        record_type: "all".to_owned(),
        mode: SearchMode::Hybrid,
        scope: scope.clone(),
        limit,
        temporal_weight: options.temporal_weight,
    })?;
    status.active_state =
        crate::context_budget::budget_ranked_strings(status.active_state, 1_200, 400);
    status.active_sessions =
        crate::context_budget::budget_ranked_strings(status.active_sessions, 900, 350);
    status.recent_decisions =
        crate::context_budget::budget_ranked_strings(status.recent_decisions, 1_000, 350);
    status.recent_events =
        crate::context_budget::budget_ranked_strings(status.recent_events, 700, 300);
    search.results = budget_search_results(search.results, 4_800);
    let pending_candidates = list_candidates(ListCandidatesOptions {
        project_name: options.project_name.clone(),
        start_dir: options.start_dir.clone(),
        grafiki_home: options.grafiki_home.clone(),
        status: Some("pending".to_owned()),
        scope: scope.clone(),
        limit: 200,
        order: CandidateOrder::Recent,
    })?
    .len();

    let answer = crate::context_budget::truncate_chars(
        &format_agent_memory_answer(&status, &search, pending_candidates),
        crate::context_budget::DEFAULT_CONTEXT_BUDGET_CHARS,
    );
    let returned_ids = search
        .results
        .iter()
        .map(|result| format!("{}:{}", result.record_type, result.id))
        .collect::<Vec<_>>();
    let audit_id = record_agent_query(
        options.project_name.clone(),
        options.start_dir.clone(),
        options.grafiki_home.clone(),
        &agent,
        &question,
        &scope,
        &returned_ids,
        search.mode,
        search.fallback.as_deref(),
        started.elapsed().as_millis().min(i64::MAX as u128) as i64,
    )?;
    Ok(AgentMemoryBriefing {
        project: status.project,
        scope: status.scope,
        question,
        agent,
        audit_id,
        answer,
        active_sessions: status.active_sessions,
        active_state: status.active_state,
        recent_decisions: status.recent_decisions,
        recent_events: status.recent_events,
        relevant_memory: search.results,
        pending_candidates,
        semantic_available: search.semantic_available,
        fallback: search.fallback,
        agent_instructions: vec![
            "Use this briefing before broad repository exploration.".to_owned(),
            "Call grafiki_record for any memory item you need in full detail.".to_owned(),
            "Save durable, user-approved facts with grafiki_save, grafiki_decide, or grafiki_state_set.".to_owned(),
            "When unsure, use grafiki_candidate_propose so the user can review it in Grafiki Desktop.".to_owned(),
            "End the session with grafiki_end or grafiki_handoff so the next agent inherits context.".to_owned(),
        ],
    })
}

/// Options for [`chat`] — "chat with your memory" (grounded, cited RAG).
#[derive(Clone)]
pub struct ChatOptions {
    pub project_name: Option<String>,
    pub start_dir: PathBuf,
    pub grafiki_home: Option<PathBuf>,
    pub question: String,
    pub scope: String,
    pub limit: usize,
    pub temporal_weight: f64,
}

/// Chat with your memory using the deterministic, model-free extractive provider.
/// See [`chat_with_provider`] to supply a local model (Phase 2, `docs/CHAT_DESIGN.md`).
pub fn chat(options: ChatOptions) -> Result<crate::chat::ChatReply> {
    chat_with_provider(options, &crate::chat::ExtractiveProvider)
}

/// Chat with your memory using a caller-supplied [`crate::chat::ChatProvider`].
///
/// Retrieve (hybrid, scope-aware) → ground → generate → cite. Grounded and cited:
/// the answer is built ONLY from retrieved memory, every source is returned as a
/// [`crate::chat::Citation`], and when nothing is relevant it ABSTAINS with the
/// fixed [`crate::chat::NO_MEMORY_ANSWER`] rather than fabricate. Retrieved
/// snippets are injection-scanned (M-E5) and flagged so a model provider treats
/// them as untrusted data. Logs the query to the audit trail so chat-driven
/// retrievals reinforce reuse-salience (M-E1), exactly like `ask_memory`.
pub fn chat_with_provider(
    options: ChatOptions,
    provider: &dyn crate::chat::ChatProvider,
) -> Result<crate::chat::ChatReply> {
    use crate::chat::{ChatReply, Citation, GroundedMemory, NO_MEMORY_ANSWER};

    let started = Instant::now();
    let question = options.question.trim().to_owned();
    if question.is_empty() {
        return Err(GrafikiError::InvalidCandidate(
            "chat question is required".to_owned(),
        ));
    }
    let scope = options.scope.clone();
    let limit = options.limit.clamp(1, 20);

    let search = search_memory(SearchMemoryOptions {
        project_name: options.project_name.clone(),
        start_dir: options.start_dir.clone(),
        grafiki_home: options.grafiki_home.clone(),
        query: question.clone(),
        record_type: "all".to_owned(),
        mode: SearchMode::Hybrid,
        scope: scope.clone(),
        limit,
        temporal_weight: options.temporal_weight,
    })?;

    // Log the retrieval for reuse-salience, mirroring ask_memory (best-effort:
    // an audit failure must not fail the chat).
    let returned_ids = search
        .results
        .iter()
        .map(|result| format!("{}:{}", result.record_type, result.id))
        .collect::<Vec<_>>();
    let _ = record_agent_query(
        options.project_name.clone(),
        options.start_dir.clone(),
        options.grafiki_home.clone(),
        "chat",
        &question,
        &scope,
        &returned_ids,
        search.mode,
        search.fallback.as_deref(),
        started.elapsed().as_millis().min(i64::MAX as u128) as i64,
    );

    if search.results.is_empty() {
        return Ok(ChatReply {
            question,
            scope,
            answer: NO_MEMORY_ANSWER.to_owned(),
            citations: Vec::new(),
            used_memory: false,
            flagged_injection: false,
        });
    }

    let memories: Vec<GroundedMemory> = search
        .results
        .iter()
        .enumerate()
        .map(|(i, result)| GroundedMemory {
            index: i + 1,
            record_type: result.record_type.clone(),
            id: result.id.clone(),
            title: result.title.clone(),
            snippet: result.snippet.clone(),
            suspicious: crate::injection::is_suspicious(&result.snippet),
        })
        .collect();
    let memories = crate::chat::budget_grounded_memories(
        &memories,
        crate::context_budget::DEFAULT_CONTEXT_BUDGET_CHARS
            .saturating_sub(question.chars().count().min(1_000) + 32),
    );
    let flagged_injection = memories.iter().any(|memory| memory.suspicious);

    // Relevance gate for providers that can't judge it themselves (the
    // extractive floor): retrieval matching one generic word used to present
    // an unrelated memory as a confident, cited answer — the exact
    // "confidently wrong" failure this product exists to prevent.
    if !provider.judges_relevance()
        && crate::chat::question_term_coverage(&question, &memories) < 0.5
    {
        return Ok(ChatReply {
            question,
            scope,
            answer: NO_MEMORY_ANSWER.to_owned(),
            citations: Vec::new(),
            used_memory: false,
            flagged_injection,
        });
    }

    let answer = provider.generate(&question, &memories)?;

    // Abstention contract: an abstaining reply must not claim memory use or
    // attach citations — the UI used to render "nothing in memory" WITH a
    // citation chip pointing at an irrelevant record.
    if answer.trim() == NO_MEMORY_ANSWER {
        return Ok(ChatReply {
            question,
            scope,
            answer: NO_MEMORY_ANSWER.to_owned(),
            citations: Vec::new(),
            used_memory: false,
            flagged_injection,
        });
    }

    // Verify the answer's citations against the memories we actually provided.
    // Two failures used to slip through: (1) EVERY retrieved memory was attached
    // as a citation regardless of what the answer cited, and (2) an answer citing
    // a nonexistent source like [99] was accepted as grounded. A reference to a
    // source we never supplied means the model fabricated — abstain rather than
    // present a confident, wrongly-cited answer.
    let valid_indices: std::collections::HashSet<usize> =
        memories.iter().map(|memory| memory.index).collect();
    let cited = crate::chat::parse_cited_indices(&answer);
    if cited.iter().any(|index| !valid_indices.contains(index)) {
        return Ok(ChatReply {
            question,
            scope,
            answer: NO_MEMORY_ANSWER.to_owned(),
            citations: Vec::new(),
            used_memory: false,
            flagged_injection,
        });
    }

    // Attach ONLY the memories the answer actually cited. If the model cited
    // nothing at all (some models omit markers), fall back to the provided set
    // so the answer still carries its provenance rather than none.
    let citations: Vec<Citation> = memories
        .iter()
        .filter(|memory| cited.is_empty() || cited.contains(&memory.index))
        .map(|memory| Citation {
            index: memory.index,
            record_type: memory.record_type.clone(),
            id: memory.id.clone(),
            title: memory.title.clone(),
            snippet: memory.snippet.clone(),
        })
        .collect();

    Ok(ChatReply {
        question,
        scope,
        answer,
        citations,
        used_memory: true,
        flagged_injection,
    })
}

fn format_agent_memory_answer(
    status: &StatusReport,
    search: &SearchReport,
    pending_candidates: usize,
) -> String {
    let mut lines = Vec::new();
    lines.push(format!("Grafiki memory for: {}", search.query));

    if status.active_state.is_empty()
        && status.active_sessions.is_empty()
        && status.recent_decisions.is_empty()
        && search.results.is_empty()
    {
        lines.push(
            "I do not have trusted memory for this yet. Use the repository as source of truth, then propose durable findings with grafiki_candidate_propose."
                .to_owned(),
        );
    }

    if !status.active_state.is_empty() {
        lines.push(format!("Active work: {}.", status.active_state.join("; ")));
    }
    if !status.active_sessions.is_empty() {
        lines.push(format!(
            "Active sessions: {}.",
            status.active_sessions.join("; ")
        ));
    }
    if !status.recent_decisions.is_empty() {
        lines.push(format!(
            "Recent decisions: {}.",
            status.recent_decisions.join("; ")
        ));
    }
    if !search.results.is_empty() {
        lines.push("Most relevant trusted memory:".to_owned());
        for result in search.results.iter().take(5) {
            let evidence_note = if result.evidence.is_empty() {
                "no evidence link".to_owned()
            } else {
                result
                    .evidence
                    .iter()
                    .take(2)
                    .map(|evidence| {
                        evidence.uri.clone().unwrap_or_else(|| {
                            evidence
                                .source_event_id
                                .as_ref()
                                .map(|id| format!("grafiki://capture/{id}"))
                                .unwrap_or_else(|| evidence.source_type.clone())
                        })
                    })
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            lines.push(format!(
                "- [{}:{}] {} ({}) — {} Evidence: {}",
                result.record_type,
                result.id,
                result.title,
                display_scope(&result.scope),
                result.snippet,
                evidence_note
            ));
        }
    }
    if let Some(fallback) = &search.fallback {
        lines.push(format!("Retrieval note: {fallback}"));
    }
    if pending_candidates > 0 {
        lines.push(format!(
            "There are {pending_candidates} pending candidate memories waiting for human review."
        ));
    }
    lines.push(
        "For new uncertain facts, propose memory instead of silently trusting it.".to_owned(),
    );

    lines.join("\n")
}

fn budget_search_results(results: Vec<SearchResult>, total_chars: usize) -> Vec<SearchResult> {
    let mut remaining = total_chars;
    let mut kept = Vec::new();
    for mut result in results {
        let fixed = format!("[{}:{}] {} ", result.record_type, result.id, result.title);
        let fixed_chars = fixed.chars().count();
        if remaining <= fixed_chars {
            break;
        }
        let snippet_cap = remaining
            .saturating_sub(fixed_chars)
            .min(crate::context_budget::DEFAULT_ITEM_BUDGET_CHARS);
        result.snippet = crate::context_budget::truncate_chars(&result.snippet, snippet_cap);
        result.evidence.truncate(2);
        for evidence in &mut result.evidence {
            evidence.excerpt = crate::context_budget::truncate_chars(&evidence.excerpt, 240);
        }
        remaining = remaining.saturating_sub(fixed_chars + result.snippet.chars().count() + 1);
        kept.push(result);
    }
    kept
}

fn record_agent_query(
    project_name: Option<String>,
    start_dir: PathBuf,
    grafiki_home: Option<PathBuf>,
    agent: &str,
    question: &str,
    scope: &str,
    returned_ids: &[String],
    mode: SearchMode,
    fallback: Option<&str>,
    latency_ms: i64,
) -> Result<String> {
    let (_project, connection) = resolve_and_open(project_name, start_dir, grafiki_home)?;
    let id = new_ulid();
    connection.execute(
        "
        INSERT INTO agent_queries
            (id, agent, question, scope, returned_ids, retrieval_mode, fallback, latency_ms)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
        ",
        params![
            id,
            agent,
            question,
            scope,
            serde_json::to_string(returned_ids)?,
            search_mode_label(mode),
            fallback,
            latency_ms
        ],
    )?;
    Ok(id)
}

pub fn list_agent_queries(options: ListAgentQueriesOptions) -> Result<Vec<AgentQueryLogItem>> {
    let scope = Scope::new(options.scope)?;
    let scope_chain = browse_scope_chain(&scope);
    let (_project, connection) = resolve_and_open(
        options.project_name,
        options.start_dir,
        options.grafiki_home,
    )?;
    let sql = scoped_query(
        "
        SELECT id, agent, question, scope, returned_ids, retrieval_mode, fallback,
               latency_ms, created_at
        FROM agent_queries
        WHERE scope IN ({scopes})
        ORDER BY created_at DESC, id DESC
        LIMIT ?
        ",
        scope_chain.len(),
    );
    let limit = options.limit.clamp(1, 200) as i64;
    let mut params: Vec<&dyn rusqlite::ToSql> = scope_chain
        .iter()
        .map(|scope| scope as &dyn rusqlite::ToSql)
        .collect();
    params.push(&limit);
    let mut statement = connection.prepare(&sql)?;
    let rows = collect_rows(statement.query_map(params.as_slice(), agent_query_from_row)?)?;
    Ok(rows)
}

fn agent_query_from_row(row: &Row<'_>) -> rusqlite::Result<AgentQueryLogItem> {
    let returned_ids_text: String = row.get(4)?;
    let returned_ids = serde_json::from_str(&returned_ids_text).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(4, Type::Text, Box::new(error))
    })?;
    Ok(AgentQueryLogItem {
        id: row.get(0)?,
        agent: row.get(1)?,
        question: row.get(2)?,
        scope: row.get(3)?,
        returned_ids,
        retrieval_mode: row.get(5)?,
        fallback: row.get(6)?,
        latency_ms: row.get(7)?,
        created_at: row.get(8)?,
    })
}

fn search_mode_label(mode: SearchMode) -> &'static str {
    match mode {
        SearchMode::Keyword => "keyword",
        SearchMode::Semantic => "semantic",
        SearchMode::Hybrid => "hybrid",
        SearchMode::Graph => "graph",
        SearchMode::Rerank => "rerank",
    }
}

pub fn get_embedding_status(options: EmbeddingStatusOptions) -> Result<EmbeddingStatusReport> {
    let scope = Scope::new(options.scope)?;
    let scope_chain = scope.chain().into_vec();
    let (project, connection) = resolve_and_open(
        options.project_name,
        options.start_dir,
        options.grafiki_home,
    )?;

    let sql = scoped_query(
        "
        SELECT status, COUNT(*)
        FROM embedding_jobs
        WHERE scope IN ({scopes})
        GROUP BY status
        ",
        scope_chain.len(),
    );
    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map(params_from_iter(scope_chain.iter()), |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
    })?;

    let mut pending = 0;
    let mut embedded = 0;
    let mut failed = 0;
    let mut skipped = 0;
    for row in rows {
        let (status, count) = row?;
        match status.as_str() {
            "pending" => pending = count,
            "embedded" => embedded = count,
            "failed" => failed = count,
            "skipped" => skipped = count,
            _ => {}
        }
    }

    let mut statement = connection.prepare(
        "
        SELECT provider, model, dimension, COUNT(*)
        FROM embedding_metadata
        GROUP BY provider, model, dimension
        ORDER BY provider ASC, model ASC, dimension ASC
        ",
    )?;
    let metadata = collect_rows(statement.query_map([], |row| {
        Ok(EmbeddingMetadataSummary {
            provider: row.get(0)?,
            model: row.get(1)?,
            dimension: row.get(2)?,
            records: row.get(3)?,
        })
    })?)?;

    Ok(EmbeddingStatusReport {
        project: project.project,
        scope: scope.as_str().to_owned(),
        runtime: embedding_runtime_summary(&connection, &scope_chain)?,
        pending,
        embedded,
        failed,
        skipped,
        metadata,
    })
}

fn embedding_runtime_summary(
    connection: &Connection,
    scope_chain: &[String],
) -> Result<EmbeddingRuntimeSummary> {
    let provider = configured_embedding_provider_summary();
    let dimension = provider.dimension.map(|dimension| dimension as i64);
    let availability = runtime_embedding_availability(
        connection,
        scope_chain,
        &provider.provider,
        &provider.model,
        dimension,
    )?;

    Ok(EmbeddingRuntimeSummary {
        requested_provider: provider.requested_provider,
        provider: provider.provider,
        model: provider.model,
        dimension,
        vector_backend: embedding_vector_backend_label().to_owned(),
        embeddable_records: availability.embeddable_records,
        indexed_records: availability.indexed_records,
        fresh_records: availability.fresh_records,
        missing_or_stale_records: availability.missing_or_stale_records,
        note: provider.note,
    })
}

#[derive(Debug, Clone, Copy)]
struct RuntimeEmbeddingAvailability {
    embeddable_records: i64,
    indexed_records: i64,
    fresh_records: i64,
    missing_or_stale_records: i64,
}

fn runtime_embedding_availability(
    connection: &Connection,
    scope_chain: &[String],
    provider: &str,
    model: &str,
    dimension: Option<i64>,
) -> Result<RuntimeEmbeddingAvailability> {
    let records = load_embeddable_records(connection, scope_chain)?;
    let embeddable_records = records.len() as i64;
    let Some(dimension) = dimension else {
        return Ok(RuntimeEmbeddingAvailability {
            embeddable_records,
            indexed_records: 0,
            fresh_records: 0,
            missing_or_stale_records: embeddable_records,
        });
    };

    let indexed_records =
        count_runtime_embedding_vectors(connection, provider, model, dimension, scope_chain)?;
    let mut fresh_records = 0;
    let mut statement = connection.prepare(
        "
        SELECT COUNT(*)
        FROM embedding_vectors
        WHERE record_type = ?1
          AND record_id = ?2
          AND provider = ?3
          AND model = ?4
          AND dimension = ?5
          AND content_hash = ?6
        ",
    )?;
    for record in records {
        let content_hash = checksum(record.content.trim());
        let count: i64 = statement.query_row(
            params![
                record.record_type,
                record.record_id,
                provider,
                model,
                dimension,
                content_hash
            ],
            |row| row.get(0),
        )?;
        if count > 0 {
            fresh_records += 1;
        }
    }

    Ok(RuntimeEmbeddingAvailability {
        embeddable_records,
        indexed_records,
        fresh_records,
        missing_or_stale_records: embeddable_records.saturating_sub(fresh_records),
    })
}

fn count_runtime_embedding_vectors(
    connection: &Connection,
    provider: &str,
    model: &str,
    dimension: i64,
    scope_chain: &[String],
) -> Result<i64> {
    let sql = scoped_query(
        "
        SELECT COUNT(*)
        FROM embedding_vectors
        WHERE provider = ?1
          AND model = ?2
          AND dimension = ?3
          AND scope IN ({scopes})
        ",
        scope_chain.len(),
    );
    let mut params: Vec<&dyn rusqlite::ToSql> = vec![&provider, &model, &dimension];
    params.extend(
        scope_chain
            .iter()
            .map(|scope| scope as &dyn rusqlite::ToSql),
    );
    Ok(connection.query_row(&sql, params.as_slice(), |row| row.get(0))?)
}

fn embedding_vector_backend_label() -> &'static str {
    #[cfg(feature = "sqlite-vec")]
    {
        "json+sqlite-vec"
    }
    #[cfg(not(feature = "sqlite-vec"))]
    {
        "json"
    }
}

/// Cheap "is there embedding work to do?" check — counts pending jobs WITHOUT
/// constructing the (expensive) embedding provider. The daemon worker gates on
/// this so an idle daemon does not reload the ONNX model every poll (C12): loading
/// the model is only worth it when there is actually a job to run.
pub fn pending_embedding_count(
    project_name: Option<String>,
    start_dir: PathBuf,
    grafiki_home: Option<PathBuf>,
) -> Result<usize> {
    let (_project, connection) = resolve_and_open(project_name, start_dir, grafiki_home)?;
    let count: i64 = connection.query_row(
        "SELECT COUNT(*) FROM embedding_jobs WHERE status = 'pending'",
        [],
        |row| row.get(0),
    )?;
    Ok(count.max(0) as usize)
}

pub fn process_embedding_jobs(
    options: ProcessEmbeddingsOptions,
) -> Result<ProcessEmbeddingsReport> {
    let requested_scope = options.scope.trim().to_owned();
    let (project, mut connection) = resolve_and_open(
        options.project_name,
        options.start_dir,
        options.grafiki_home,
    )?;
    let (scope_label, scope_chain) = if requested_scope == "*" {
        ("*".to_owned(), pending_embedding_scopes(&connection)?)
    } else {
        let scope = Scope::new(requested_scope)?;
        (scope.as_str().to_owned(), scope.chain().into_vec())
    };
    let provider = configured_embedding_provider()?;
    let limit = options.limit.clamp(1, 500);
    let enqueued = if options.rebuild && !scope_chain.is_empty() {
        // An explicit rebuild revives previously-failed jobs so they are not
        // permanent dead-letters; the user asked to retry.
        sweep_orphan_embedding_records(&mut connection, &scope_chain)?;
        requeue_failed_embedding_jobs(&connection, &scope_chain)?;
        enqueue_embeddable_records(&mut connection, &scope_chain)?
    } else {
        0
    };
    let jobs = if scope_chain.is_empty() {
        Vec::new()
    } else {
        pending_embedding_jobs(&connection, &scope_chain, limit)?
    };

    let mut processed = 0;
    let mut skipped = 0;
    let mut failed = 0;

    for job in jobs {
        match process_embedding_job(&mut connection, &provider, &job) {
            Ok(EmbeddingJobOutcome::Processed) => processed += 1,
            Ok(EmbeddingJobOutcome::Skipped) => skipped += 1,
            Err(error) => {
                failed += 1;
                mark_embedding_job_failed(&mut connection, &job.id, &error.to_string())?;
            }
        }
    }

    let pending_remaining = if scope_chain.is_empty() {
        0
    } else {
        count_pending_embedding_jobs(&connection, &scope_chain)?
    };

    Ok(ProcessEmbeddingsReport {
        project: project.project,
        scope: scope_label,
        provider: provider.provider_name().to_owned(),
        model: provider.model_name().to_owned(),
        dimension: provider.dimension(),
        enqueued,
        processed,
        skipped,
        failed,
        pending_remaining,
    })
}

pub fn get_graph(options: GraphOptions) -> Result<GraphReport> {
    let (project, connection) = resolve_and_open(
        options.project_name,
        options.start_dir,
        options.grafiki_home,
    )?;
    if !entity_exists(&connection, &options.entity_id)? {
        return Err(GrafikiError::EntityNotFound(options.entity_id));
    }

    let max_depth = options.depth.min(8);
    let mut visited_entities = HashSet::new();
    let mut queued = VecDeque::from([(options.entity_id.clone(), 0usize)]);
    let mut relation_ids = HashSet::new();
    let mut relations = Vec::new();

    while let Some((entity_id, depth)) = queued.pop_front() {
        if !visited_entities.insert(entity_id.clone()) {
            continue;
        }
        if depth >= max_depth {
            continue;
        }

        for relation in relations_for_entity(&connection, &entity_id)? {
            if relation_ids.insert(relation.id.clone()) {
                let neighbor = if relation.from_entity == entity_id {
                    relation.to_entity.clone()
                } else {
                    relation.from_entity.clone()
                };
                if !visited_entities.contains(&neighbor) {
                    queued.push_back((neighbor, depth + 1));
                }
                relations.push(relation);
            }
        }
    }

    let entities = load_graph_entities(&connection, &visited_entities)?;

    Ok(GraphReport {
        project: project.project,
        root: options.entity_id,
        depth: max_depth,
        entities,
        relations,
    })
}

pub fn generate_report(options: ProjectReportOptions) -> Result<ProjectReport> {
    let scope = Scope::new(options.scope)?;
    let scope_chain = scope.chain().into_vec();
    let (project, connection) = resolve_and_open(
        options.project_name,
        options.start_dir,
        options.grafiki_home,
    )?;
    let entity_count = count_scoped_entities(&connection, &scope_chain)?;
    let relation_count = count_scoped_relations(&connection, &scope_chain)?;
    let observation_count = count_scoped_observations(&connection, &scope_chain)?;
    let decision_count = count_scoped_decisions(&connection, &scope_chain)?;
    let active_session_count = count_scoped_active_sessions(&connection, &scope_chain)?;
    let god_nodes = query_god_nodes(&connection, &scope_chain, 10)?;
    let orphan_entities = query_orphan_entities(&connection, &scope_chain, 20)?;
    let suggested_queries = suggest_report_queries(
        entity_count,
        relation_count,
        decision_count,
        active_session_count,
        &god_nodes,
        &orphan_entities,
    );

    Ok(ProjectReport {
        project: project.project,
        scope: scope.as_str().to_owned(),
        entity_count,
        relation_count,
        observation_count,
        decision_count,
        active_session_count,
        god_nodes,
        orphan_entities,
        suggested_queries,
    })
}

pub fn export_memory(options: ExportOptions) -> Result<ExportBundle> {
    let scope = Scope::new(options.scope)?;
    let scope_chain = scope.chain().into_vec();
    let (project, connection) = resolve_and_open(
        options.project_name,
        options.start_dir,
        options.grafiki_home,
    )?;

    let sessions = export_sessions(&connection, &project.project, &scope_chain)?;

    Ok(ExportBundle {
        project: project.project,
        scope: scope.as_str().to_owned(),
        entities: export_entities(&connection, &scope_chain)?,
        relations: export_relations(&connection, &scope_chain)?,
        observations: export_observations(&connection, &scope_chain)?,
        decisions: export_decisions(&connection, &scope_chain)?,
        state: export_state(&connection, &scope_chain)?,
        context: export_context(&connection, &scope_chain)?,
        sessions,
    })
}

pub fn import_memory(options: ImportOptions) -> Result<ImportReport> {
    validate_import_scopes(&options.bundle)?;
    let (project, mut connection) = resolve_and_open(
        options.project_name,
        options.start_dir,
        options.grafiki_home,
    )?;
    let bundle = options.bundle;
    let source_project = bundle.project.clone();
    let import_scope = bundle.scope.clone();
    let tx = connection.transaction()?;

    let mut entities = 0;
    for entity in &bundle.entities {
        tx.execute(
            "
            INSERT INTO entities (id, name, entity_type, scope)
            VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                entity_type = excluded.entity_type,
                scope = excluded.scope,
                retired_at = NULL,
                updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
            ",
            params![entity.id, entity.name, entity.entity_type, entity.scope],
        )?;
        let entity_embedding_text = format!("{} {}", entity.name, entity.entity_type);
        enqueue_embedding_job(
            &tx,
            "entity",
            &entity.id,
            &entity.scope,
            &entity_embedding_text,
        )?;
        entities += 1;
    }

    let mut observations = 0;
    let mut skipped_observations = 0;
    for observation in &bundle.observations {
        if !entity_exists_in_tx(&tx, &observation.entity_id)? {
            skipped_observations += 1;
            continue;
        }
        tx.execute(
            "
            INSERT INTO observations (id, entity_id, content, category, source, confidence)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ON CONFLICT(id) DO UPDATE SET
                entity_id = excluded.entity_id,
                content = excluded.content,
                category = excluded.category,
                source = excluded.source,
                confidence = excluded.confidence
            ",
            params![
                observation.id,
                observation.entity_id,
                observation.content,
                observation.category,
                format!("import:{source_project}"),
                observation.confidence
            ],
        )?;
        enqueue_embedding_job(
            &tx,
            "observation",
            &observation.id,
            &observation.scope,
            &observation.content,
        )?;
        observations += 1;
    }

    let mut decisions = 0;
    for decision in &bundle.decisions {
        tx.execute(
            "
            INSERT INTO decisions (id, title, reasoning, scope, status)
            VALUES (?1, ?2, ?3, ?4, ?5)
            ON CONFLICT(id) DO UPDATE SET
                title = excluded.title,
                reasoning = excluded.reasoning,
                scope = excluded.scope,
                status = excluded.status,
                updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
            ",
            params![
                decision.id,
                decision.title,
                decision.reasoning,
                decision.scope,
                decision.status
            ],
        )?;
        let decision_embedding_text = format!(
            "{} {}",
            decision.title,
            decision.reasoning.as_deref().unwrap_or("")
        );
        enqueue_embedding_job(
            &tx,
            "decision",
            &decision.id,
            &decision.scope,
            &decision_embedding_text,
        )?;
        decisions += 1;
    }

    // Second pass: wire up decision supersession now that every decision exists
    // (superseded_by is a self-referential FK).
    for decision in &bundle.decisions {
        if let Some(superseded_by) = decision.superseded_by.as_deref() {
            let target_exists: i64 = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM decisions WHERE id = ?1)",
                [superseded_by],
                |row| row.get(0),
            )?;
            if target_exists == 1 {
                tx.execute(
                    "UPDATE decisions SET superseded_by = ?1 WHERE id = ?2",
                    params![superseded_by, decision.id],
                )?;
            }
        }
    }

    let mut state = 0;
    for item in &bundle.state {
        tx.execute(
            "
            INSERT INTO state (id, key, title, status, owner, details, blockers, depends_on, scope, priority)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            ON CONFLICT(key) DO UPDATE SET
                title = excluded.title,
                status = excluded.status,
                owner = excluded.owner,
                details = excluded.details,
                blockers = excluded.blockers,
                depends_on = excluded.depends_on,
                scope = excluded.scope,
                priority = excluded.priority,
                updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
            ",
            params![
                new_ulid(),
                item.key,
                item.title,
                item.status,
                item.owner,
                item.details,
                json_array(&item.blockers)?,
                json_array(&item.depends_on)?,
                item.scope,
                item.priority
            ],
        )?;
        state += 1;
    }

    let mut relations = 0;
    let mut skipped_relations = 0;
    for relation in &bundle.relations {
        if !entity_exists_in_tx(&tx, &relation.from_entity)?
            || !entity_exists_in_tx(&tx, &relation.to_entity)?
        {
            skipped_relations += 1;
            continue;
        }
        tx.execute(
            "
            INSERT INTO relations (id, from_entity, to_entity, relation, weight, confidence, source_type, source)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            ON CONFLICT(from_entity, to_entity, relation) DO UPDATE SET
                weight = excluded.weight,
                confidence = excluded.confidence,
                source_type = excluded.source_type,
                source = excluded.source
            ",
            params![
                relation.id,
                relation.from_entity,
                relation.to_entity,
                relation.relation,
                relation.weight,
                relation.confidence,
                relation.source_type,
                relation.source
            ],
        )?;
        relations += 1;
    }

    let mut context = 0;
    for item in &bundle.context {
        tx.execute(
            "
            INSERT INTO context (id, key, title, content, category, scope, version, checksum)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            ON CONFLICT(key) DO UPDATE SET
                title = excluded.title,
                content = excluded.content,
                category = excluded.category,
                scope = excluded.scope,
                version = excluded.version,
                checksum = excluded.checksum,
                retired_at = NULL,
                updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
            ",
            params![
                new_ulid(),
                item.key,
                item.title,
                item.content,
                item.category,
                item.scope,
                item.version,
                checksum(&item.content)
            ],
        )?;
        enqueue_embedding_job(
            &tx,
            "context",
            &item.key,
            &item.scope,
            &format!("{} {}", item.title, item.content),
        )?;
        context += 1;
    }

    let mut sessions = 0;
    for session in &bundle.sessions {
        // parent_session is a self-FK; set it in a second pass below.
        tx.execute(
            "
            INSERT INTO sessions
                (id, session_type, project, scope, status, goal, summary,
                 accomplishments, remaining, files_changed, decisions_made,
                 entities_touched, handoff_context, parent_session, child_session,
                 started_at, ended_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, NULL, ?14, ?15, ?16)
            ON CONFLICT(id) DO UPDATE SET
                session_type = excluded.session_type,
                status = excluded.status,
                scope = excluded.scope,
                goal = excluded.goal,
                summary = excluded.summary,
                accomplishments = excluded.accomplishments,
                remaining = excluded.remaining,
                files_changed = excluded.files_changed,
                decisions_made = excluded.decisions_made,
                entities_touched = excluded.entities_touched,
                handoff_context = excluded.handoff_context,
                child_session = excluded.child_session,
                ended_at = excluded.ended_at
            ",
            params![
                session.id,
                session.session_type,
                project.project,
                session.scope,
                session.status,
                session.goal,
                session.summary,
                serde_json::to_string(&session.accomplishments)?,
                serde_json::to_string(&session.remaining)?,
                serde_json::to_string(&session.files_changed)?,
                serde_json::to_string(&session.decisions_made)?,
                serde_json::to_string(&session.entities_touched)?,
                session.handoff_context,
                session.child_session,
                session.started_at,
                session.ended_at
            ],
        )?;
        sessions += 1;
    }
    for session in &bundle.sessions {
        if let Some(parent) = session.parent_session.as_deref() {
            let parent_exists: i64 = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM sessions WHERE id = ?1)",
                [parent],
                |row| row.get(0),
            )?;
            if parent_exists == 1 {
                tx.execute(
                    "UPDATE sessions SET parent_session = ?1 WHERE id = ?2",
                    params![parent, session.id],
                )?;
            }
        }
    }

    tx.execute(
        "
        INSERT INTO events (id, event_type, source_session, target_type, target_id, scope, summary)
        VALUES (?1, 'state_changed', NULL, 'import', ?2, ?3, ?4)
        ",
        params![
            new_ulid(),
            source_project,
            import_scope,
            format!(
                "Imported Grafiki memory from {}: {} entities, {} relations, {} observations, {} decisions",
                source_project, entities, relations, observations, decisions
            )
        ],
    )?;
    tx.commit()?;

    Ok(ImportReport {
        project: project.project,
        source_project,
        entities,
        relations,
        skipped_relations,
        observations,
        decisions,
        state,
        context,
        sessions,
        skipped_observations,
    })
}

/// Build the incoming `(subject, predicate, value)` slot for an observation
/// candidate, for deterministic Stage-1.2 conflict detection. Explicit
/// `predicate`/`attribute` + `value`/`object` payload fields take precedence;
/// otherwise the slot is recovered from a canonical leading `"<predicate>:
/// <value>"` content line. Returns `None` — disabling deterministic detection —
/// unless the predicate is a registered single-valued attribute.
fn incoming_observation_slot(
    payload: &serde_json::Value,
    subject: &str,
    content: Option<&str>,
) -> Option<crate::conflict::Slot> {
    use crate::conflict::{attribute_cardinality, Cardinality, Slot};
    let explicit = payload
        .get("predicate")
        .or_else(|| payload.get("attribute"))
        .and_then(|v| v.as_str())
        .zip(
            payload
                .get("value")
                .or_else(|| payload.get("object"))
                .and_then(|v| v.as_str()),
        )
        .map(|(predicate, value)| (predicate.to_owned(), value.to_owned()));
    let (predicate, value) = explicit.or_else(|| content.and_then(parse_observation_slot))?;
    if attribute_cardinality(&predicate) != Cardinality::Single {
        return None;
    }
    Some(Slot {
        subject: subject.to_owned(),
        predicate,
        value,
    })
}

/// Parse a canonical leading `"<predicate>: <value>"` (or `=`) slot line from an
/// observation's content. Returns it ONLY when the predicate is a registered
/// single-valued attribute — the closed registry is what stops ordinary prose
/// (`"note: shipped it"`) from being mistaken for a structured slot. The
/// predicate is bounded to a short key (≤3 words) AND the value must be a single
/// whitespace-free token, so that a prose sentence beginning with a registry word
/// (`"state: nodes are draining gracefully"`) is never read as a slot — the
/// registry alone can't tell a slot key from a sentence lead-in, so the
/// single-token value is the decisive prose guard. A real single-valued slot
/// value is a token (`active`, `us-east-1`, `v2.1`, `America/New_York`); a
/// multi-word value must be supplied via explicit `predicate`/`value` fields.
/// Because every *existing* observation is parsed through here, this also keeps
/// an explicit-payload incoming candidate from ever matching a prose note.
fn parse_observation_slot(content: &str) -> Option<(String, String)> {
    use crate::conflict::{attribute_cardinality, Cardinality};
    let first = content.lines().next()?.trim();
    let (raw_predicate, raw_value) = first.split_once(':').or_else(|| first.split_once('='))?;
    let predicate = raw_predicate.trim();
    let value = raw_value.trim();
    if predicate.is_empty()
        || value.is_empty()
        || predicate.split_whitespace().count() > 3
        || value.split_whitespace().count() != 1
        || attribute_cardinality(predicate) != Cardinality::Single
    {
        return None;
    }
    Some((predicate.to_owned(), value.to_owned()))
}

/// H2 deterministic (model-free) structural conflict detection — the Stage 1.2
/// single-valued-slot path. Given a new observation's single-valued slot, find a
/// same-entity active observation that asserts the same slot with a different
/// value (via the tested [`crate::conflict::slot_conflict`] decision). Returns
/// the prior observation id and the normalized predicate, or `None`. Needs no
/// embedding model, so it is the conflict-detection floor on the default build.
fn detect_structural_conflict(
    connection: &Connection,
    scope_chain: &[String],
    entity_name: &str,
    incoming: &crate::conflict::Slot,
) -> Result<Option<(String, String)>> {
    use crate::conflict::{normalize_predicate, slot_conflict, ConflictVerdict, Slot};

    let entity_sql = scoped_query(
        "SELECT id FROM entities WHERE retired_at IS NULL AND name = ? AND scope IN ({scopes}) LIMIT 1",
        scope_chain.len(),
    );
    let mut entity_params: Vec<&dyn rusqlite::ToSql> = vec![&entity_name];
    entity_params.extend(scope_chain.iter().map(|s| s as &dyn rusqlite::ToSql));
    let entity_id: Option<String> = connection
        .query_row(&entity_sql, entity_params.as_slice(), |row| row.get(0))
        .optional()?;
    let Some(entity_id) = entity_id else {
        return Ok(None);
    };

    let mut statement = connection.prepare(
        "SELECT id, content FROM observations WHERE entity_id = ?1 AND valid_to IS NULL",
    )?;
    let rows = statement.query_map(params![entity_id], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    for row in rows {
        let (old_id, existing_content) = row?;
        let Some((predicate, value)) = parse_observation_slot(&existing_content) else {
            continue;
        };
        let existing = Slot {
            subject: entity_name.to_owned(),
            predicate,
            value,
        };
        if slot_conflict(&existing, incoming) == ConflictVerdict::AutoSupersede {
            return Ok(Some((old_id, normalize_predicate(&incoming.predicate))));
        }
    }
    Ok(None)
}

/// H2 automated detection (real embedding model only): suggest a prior
/// observation that a newly-proposed observation about the **same entity** likely
/// supersedes, by embedding similarity above a threshold. The embedding gate only
/// narrows *which* facts are about the same thing — the supersession itself is
/// still arbitrated at approval and held for human review (the suggestion is a
/// `supersedes` hint on the candidate, never an auto-applied write). Returns the
/// best same-entity match and its cosine score.
#[cfg(feature = "fastembed")]
fn detect_observation_conflict(
    connection: &Connection,
    scope_chain: &[String],
    entity_name: &str,
    content: &str,
) -> Result<Option<(String, f32)>> {
    use crate::embeddings::{configured_embedding_provider, cosine_similarity, EmbeddingProvider};
    const THRESHOLD: f32 = 0.55;

    let entity_sql = scoped_query(
        "SELECT id FROM entities WHERE retired_at IS NULL AND name = ? AND scope IN ({scopes}) LIMIT 1",
        scope_chain.len(),
    );
    let mut entity_params: Vec<&dyn rusqlite::ToSql> = vec![&entity_name];
    entity_params.extend(scope_chain.iter().map(|s| s as &dyn rusqlite::ToSql));
    let entity_id: Option<String> = connection
        .query_row(&entity_sql, entity_params.as_slice(), |row| row.get(0))
        .optional()?;
    let Some(entity_id) = entity_id else {
        return Ok(None);
    };

    let provider = configured_embedding_provider()?;
    let query_embedding = provider.embed(content)?;
    let dimension = provider.dimension() as i64;

    let mut statement = connection.prepare(
        "
        SELECT v.record_id, v.embedding
        FROM embedding_vectors v
        JOIN observations o ON o.id = v.record_id
        WHERE v.record_type = 'observation'
          AND v.provider = ?1 AND v.model = ?2 AND v.dimension = ?3
          AND o.valid_to IS NULL AND o.entity_id = ?4
        ",
    )?;
    let rows = statement.query_map(
        params![
            provider.provider_name(),
            provider.model_name(),
            dimension,
            entity_id
        ],
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
    )?;

    let mut best: Option<(String, f32)> = None;
    for row in rows {
        let (id, embedding_json) = row?;
        let embedding: Vec<f32> = serde_json::from_str(&embedding_json)?;
        if embedding.len() != query_embedding.len() {
            continue;
        }
        let score = cosine_similarity(&query_embedding, &embedding);
        if score >= THRESHOLD && best.as_ref().is_none_or(|(_, s)| score > *s) {
            best = Some((id, score));
        }
    }
    Ok(best)
}

pub fn propose_candidate(options: ProposeCandidateOptions) -> Result<CandidateMutationReport> {
    let record_type = validate_candidate_record_type(options.record_type.trim())?;
    let source_type = validate_candidate_source_type(options.source_type.trim())?;
    let scope = Scope::new(options.scope)?;
    let confidence = validate_candidate_confidence(options.confidence)?;
    if !options.payload.is_object() {
        return Err(GrafikiError::InvalidCandidate(
            "candidate payload must be a JSON object".to_owned(),
        ));
    }

    let (_project, mut connection) = resolve_and_open(
        options.project_name,
        options.start_dir,
        options.grafiki_home,
    )?;
    let id = new_ulid();
    // Redact secrets before persistence. This is the central trust boundary for
    // directly-proposed candidates (MCP propose, auto-capture, and `grafiki init`
    // memory import all route through here), mirroring ingest_capture_event.
    let mut payload = options.payload;
    redact_json_value(&mut payload);

    // H2 conflict detection: if a new observation about an existing entity isn't
    // already marked as superseding a prior fact, suggest the prior fact it
    // supersedes as a `supersedes` hint. The candidate always stays pending for
    // human review (`conflict_kind = "review"`); arbitration runs at approval —
    // this is never an auto-applied write. Best-effort: detection must never fail
    // the proposal.
    if record_type == "observation" && payload.get("supersedes").and_then(|v| v.as_str()).is_none()
    {
        let entity_name = payload
            .get("entity_name")
            .or_else(|| payload.get("name"))
            .or_else(|| payload.get("title"))
            .and_then(|v| v.as_str())
            .map(str::to_owned);
        let content = payload
            .get("content")
            .or_else(|| payload.get("observe"))
            .and_then(|v| v.as_str())
            .map(str::to_owned);

        if let Some(name) = entity_name {
            // Stage 1.2 — deterministic single-valued-slot detection (Ritter
            // EMNLP'08 functional predicates). Model-free, so it runs on the
            // DEFAULT build. Fires only when the new observation expresses a
            // registered single-valued slot — explicit `predicate`/`value` fields
            // or a canonical `"<predicate>: <value>"` content line — and a
            // same-entity active observation asserts that slot with a different
            // value. The closed cardinality registry is the false-positive guard.
            if let Some(slot) = incoming_observation_slot(&payload, &name, content.as_deref()) {
                if let Ok(Some((old_id, predicate))) =
                    detect_structural_conflict(&connection, scope.chain().as_slice(), &name, &slot)
                {
                    payload["supersedes"] = serde_json::json!(old_id);
                    payload["conflict_kind"] = serde_json::json!("review");
                    payload["conflict_detector"] = serde_json::json!("slot");
                    payload["conflict_predicate"] = serde_json::json!(predicate);
                }
            }

            // Stage 0 fallback — embedding gate (real model builds only). Runs
            // only if the deterministic path found nothing (supersedes still
            // unset), narrowing the free-text / paraphrased observations the
            // structural layer cannot see.
            #[cfg(feature = "fastembed")]
            if payload.get("supersedes").and_then(|v| v.as_str()).is_none() {
                if let Some(content) = content.as_deref() {
                    if !content.is_empty() {
                        if let Ok(Some((old_id, score))) = detect_observation_conflict(
                            &connection,
                            scope.chain().as_slice(),
                            &name,
                            content,
                        ) {
                            payload["supersedes"] = serde_json::json!(old_id);
                            payload["conflict_kind"] = serde_json::json!("review");
                            payload["conflict_similarity"] = serde_json::json!(score);
                        }
                    }
                }
            }
        }
    }

    let rationale = options.rationale.map(|mut value| {
        redact_sensitive_text(&mut value);
        value
    });
    // Candidate and provenance are one logical write. Invalid evidence or any
    // later insert failure must roll the candidate back instead of leaving a
    // pending, evidence-less row in the review queue.
    let tx = connection.transaction()?;
    tx.execute(
        "
        INSERT INTO extraction_candidates
            (id, source_type, source, proposed_record_type, payload, scope, confidence, rationale)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
        ",
        params![
            id,
            source_type,
            options.source,
            record_type,
            serde_json::to_string(&payload)?,
            scope.as_str(),
            confidence,
            rationale
        ],
    )?;
    insert_candidate_evidence(&tx, &id, &options.evidence)?;
    tx.commit()?;

    let candidate = load_extraction_candidate(&connection, &id)?;
    Ok(CandidateMutationReport {
        candidate,
        message: "Candidate proposed for review.".to_owned(),
    })
}

pub fn list_candidates(options: ListCandidatesOptions) -> Result<Vec<ExtractionCandidate>> {
    let status = options
        .status
        .as_deref()
        .filter(|status| !status.trim().is_empty() && status.trim() != "all")
        .map(validate_candidate_status)
        .transpose()?;
    let scope = Scope::new(options.scope)?;
    // An empty scope means "the whole review queue", not "root-scoped only": the
    // queue is a human inbox, and a candidate parked in a sub-scope must never be
    // invisible by default (the Home ledger badge counts every pending candidate,
    // so the default listing has to agree with it). A non-empty scope still
    // narrows to that scope's chain.
    let all_scopes = scope.as_str().is_empty();
    let scope_chain = scope.chain().into_vec();
    let (_project, connection) = resolve_and_open(
        options.project_name,
        options.start_dir,
        options.grafiki_home,
    )?;
    let requested_limit = options.limit.clamp(1, 200) as i64;
    // Active learning prioritizes across the pending pool (not just the newest window), so it fetches
    // the whole in-scope/status set up to a generous safety cap, then re-ranks by priority and
    // truncates to the requested limit. A human review queue is far below this cap in practice.
    const ACTIVE_LEARNING_POOL_CAP: i64 = 10_000;
    let limit = if options.order == CandidateOrder::ActiveLearning {
        ACTIVE_LEARNING_POOL_CAP
    } else {
        requested_limit
    };

    let mut rows = {
        let mut sql = String::from(
            "
            SELECT id, source_type, source, proposed_record_type, payload, scope,
                   confidence, status, rationale, trusted_record_type, trusted_record_id,
                   created_at, reviewed_at
            FROM extraction_candidates
            WHERE 1 = 1
            ",
        );
        let mut params: Vec<&dyn rusqlite::ToSql> = Vec::new();
        if let Some(status) = &status {
            sql.push_str(" AND status = ?");
            params.push(status);
        }
        if !all_scopes {
            sql.push_str(&format!(
                " AND scope IN ({})",
                vec!["?"; scope_chain.len()].join(", ")
            ));
            params.extend(
                scope_chain
                    .iter()
                    .map(|scope| scope as &dyn rusqlite::ToSql),
            );
        }
        sql.push_str(" ORDER BY created_at DESC, id DESC LIMIT ?");
        params.push(&limit);
        let mut statement = connection.prepare(&sql)?;
        let candidates =
            collect_rows(statement.query_map(params.as_slice(), extraction_candidate_from_row)?)?;
        candidates
    };

    for candidate in &mut rows {
        attach_candidate_evidence(&connection, candidate)?;
        // M-E3: refine the calibrated confidence + active-learning review priority with the real
        // corroboration count (shared with the single-fetch path for cross-surface consistency).
        refine_candidate_confidence(candidate);
    }
    if options.order == CandidateOrder::ActiveLearning {
        // Most informative first; stable tiebreak by recency then id so order is deterministic.
        rows.sort_by(|a, b| {
            b.review_priority
                .partial_cmp(&a.review_priority)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| b.created_at.cmp(&a.created_at))
                .then_with(|| b.id.cmp(&a.id))
        });
        rows.truncate(requested_limit as usize);
    }
    Ok(rows)
}

pub fn approve_candidate(options: ApproveCandidateOptions) -> Result<CandidateMutationReport> {
    let (_project, connection) = resolve_and_open(
        options.project_name.clone(),
        options.start_dir.clone(),
        options.grafiki_home.clone(),
    )?;
    // CLAIM before creating the trusted record. The old check-then-act pattern
    // let two concurrent approvals (double-click, bulk + single) both pass the
    // pending check and each create a trusted record. `reviewed_at` doubles as
    // the claim marker (status stays 'pending' — the schema CHECK only allows
    // pending/approved/rejected): SQLite serializes this UPDATE, so exactly one
    // caller matches `reviewed_at IS NULL`. A claim older than 15 minutes is
    // abandoned (crash mid-approval) and may be retaken.
    let claimed = connection.execute(
        "
        UPDATE extraction_candidates
        SET reviewed_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
        WHERE id = ?1
          AND status = 'pending'
          AND (reviewed_at IS NULL
               OR reviewed_at < strftime('%Y-%m-%dT%H:%M:%SZ', 'now', '-15 minutes'))
        ",
        params![&options.id],
    )?;
    if claimed == 0 {
        let candidate = load_extraction_candidate(&connection, &options.id)?;
        let state = if candidate.status == "pending" {
            "being approved".to_owned()
        } else {
            format!("already {}", candidate.status)
        };
        return Err(GrafikiError::InvalidCandidate(format!(
            "candidate {} is {state}",
            candidate.id
        )));
    }
    let candidate = match load_extraction_candidate(&connection, &options.id) {
        Ok(candidate) => candidate,
        Err(error) => {
            drop(connection);
            release_candidate_approval_claim(&options);
            return Err(error);
        }
    };
    if let Err(error) = snapshot_candidate_supersession(&connection, &candidate) {
        drop(connection);
        release_candidate_approval_claim(&options);
        return Err(error);
    }
    drop(connection);

    // A previous attempt may have created the trusted row and failed before the
    // final candidate/evidence transaction. Reuse its provisional pointer rather
    // than duplicating trusted memory on retry.
    let existing_pointer = match (
        candidate.trusted_record_type.clone(),
        candidate.trusted_record_id.clone(),
    ) {
        (Some(record_type), Some(record_id)) => Some((record_type, record_id)),
        (None, None) => None,
        _ => {
            release_candidate_approval_claim(&options);
            return Err(GrafikiError::InvalidCandidate(format!(
                "candidate {} has an incomplete approval recovery pointer",
                candidate.id
            )));
        }
    };
    let (trusted_record_type, trusted_record_id) = match existing_pointer {
        Some(pointer) => pointer,
        None => {
            let created = match approve_candidate_payload(
                &candidate,
                options.project_name.clone(),
                options.start_dir.clone(),
                options.grafiki_home.clone(),
            ) {
                Ok(created) => created,
                Err(error) => {
                    release_candidate_approval_claim(&options);
                    return Err(error);
                }
            };
            // Persist a recovery pointer immediately after creation. This cannot
            // be fully atomic with the trusted write while record APIs open their
            // own connections. An OS/process crash or database failure in the few
            // instructions after `approve_candidate_payload` commits and before
            // this UPDATE can still orphan one trusted row; eliminating that final
            // window requires the record writers to accept a caller-owned
            // transaction. Once the pointer lands, every ordinary error/retry is
            // idempotent.
            let pointer_result = (|| -> Result<()> {
                let (_project, pointer_connection) = resolve_and_open(
                    options.project_name.clone(),
                    options.start_dir.clone(),
                    options.grafiki_home.clone(),
                )?;
                let recorded = pointer_connection.execute(
                    "UPDATE extraction_candidates
                     SET trusted_record_type = ?1, trusted_record_id = ?2
                     WHERE id = ?3 AND status = 'pending' AND reviewed_at IS NOT NULL",
                    params![&created.0, &created.1, &options.id],
                )?;
                if recorded == 0 {
                    return Err(GrafikiError::InvalidCandidate(format!(
                        "candidate {} changed while approval created {} {}; recovery pointer could not be recorded",
                        options.id, created.0, created.1
                    )));
                }
                Ok(())
            })();
            if let Err(error) = pointer_result {
                release_candidate_approval_claim(&options);
                return Err(error);
            }
            created
        }
    };

    let (_project, mut connection) = match resolve_and_open(
        options.project_name.clone(),
        options.start_dir.clone(),
        options.grafiki_home.clone(),
    ) {
        Ok(resolved) => resolved,
        Err(error) => {
            release_candidate_approval_claim(&options);
            return Err(error);
        }
    };
    // Flip the candidate status and promote its evidence atomically so a failure
    // between the two cannot leave evidence pointing at a not-yet-approved
    // candidate. (The trusted record was created above; the pending-status guard
    // makes re-approval a no-op, preventing duplicates on the common retry path.)
    let finalize = (|| -> Result<()> {
        let tx = connection.transaction()?;
        let approved = tx.execute(
            "
            UPDATE extraction_candidates
            SET status = 'approved',
                trusted_record_type = ?1,
                trusted_record_id = ?2,
                reviewed_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
            WHERE id = ?3 AND status = 'pending'
            ",
            params![&trusted_record_type, &trusted_record_id, &options.id],
        )?;
        if approved == 0 {
            return Err(GrafikiError::InvalidCandidate(format!(
                "candidate {} changed concurrently during approval; recovery pointer is {} {}",
                options.id, trusted_record_type, trusted_record_id
            )));
        }
        promote_candidate_evidence(&tx, &options.id, &trusted_record_type, &trusted_record_id)?;
        tx.commit()?;
        Ok(())
    })();
    if let Err(error) = finalize {
        release_candidate_approval_claim(&options);
        return Err(error);
    }
    let candidate = load_extraction_candidate(&connection, &options.id)?;
    Ok(CandidateMutationReport {
        candidate,
        message: "Candidate approved into trusted memory.".to_owned(),
    })
}

#[derive(Debug, Serialize, Deserialize)]
struct ApprovalRestoreSnapshot {
    record_type: String,
    record_id: String,
    prior_status: Option<String>,
    prior_superseded_by: Option<String>,
    prior_valid_to: Option<String>,
    /// Full prior contents of a key/name-collision record that approval
    /// overwrote via UPSERT. Present ⇒ the record PRE-EXISTED and undo must
    /// restore these contents (context/state) or refuse (entity) rather than
    /// delete — otherwise undo destroys data the candidate never owned.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    prior_upsert: Option<PriorUpsert>,
}

/// The pre-approval row of a keyed record an upsert-approval would overwrite.
#[derive(Debug, Serialize, Deserialize)]
struct PriorUpsert {
    /// "context" | "state" | "entity".
    record_type: String,
    /// context/state `key`, or the entity `id`.
    key: String,
    /// Prior column values (null for entity — we only refuse, not restore).
    row: Option<serde_json::Value>,
}

/// Persist the predecessor's exact state before approval mutates it. Captures
/// two kinds of loss: a supersession target (decision/observation) whose status
/// approval flips, AND a key/name-collision record (context/state/entity) whose
/// full contents an upsert-approval would overwrite. The snapshot lives on the
/// candidate so approval retry and later undo share one durable restoration
/// source.
fn snapshot_candidate_supersession(
    connection: &Connection,
    candidate: &ExtractionCandidate,
) -> Result<()> {
    let already_snapshotted: bool = connection.query_row(
        "SELECT approval_restore IS NOT NULL FROM extraction_candidates WHERE id = ?1",
        [&candidate.id],
        |row| row.get(0),
    )?;
    if already_snapshotted {
        return Ok(());
    }

    // (a) Supersession target, if the payload names one.
    let supersession = match candidate_payload_optional_string(&candidate.payload, &["supersedes"])
    {
        Some(record_id) => match candidate.record_type.as_str() {
            "decision" => connection
                .query_row(
                    "SELECT status, superseded_by FROM decisions WHERE id = ?1",
                    [&record_id],
                    |row| {
                        Ok((
                            Some(row.get::<_, String>(0)?),
                            row.get::<_, Option<String>>(1)?,
                        ))
                    },
                )
                .optional()?
                .map(|(status, superseded_by)| (record_id.clone(), status, superseded_by, None)),
            "observation" => connection
                .query_row(
                    "SELECT valid_to FROM observations WHERE id = ?1",
                    [&record_id],
                    |row| row.get::<_, Option<String>>(0),
                )
                .optional()?
                .map(|valid_to| (record_id.clone(), None, None, Some(valid_to))),
            _ => None,
        },
        None => None,
    };

    // (b) Key/name-collision record whose contents an upsert would overwrite.
    let prior_upsert = snapshot_prior_upsert(connection, candidate)?;

    let Some(mut snapshot) = supersession
        .map(
            |(record_id, prior_status, prior_superseded_by, prior_valid_to)| {
                ApprovalRestoreSnapshot {
                    record_type: candidate.record_type.clone(),
                    record_id,
                    prior_status,
                    prior_superseded_by,
                    prior_valid_to: prior_valid_to.flatten(),
                    prior_upsert: None,
                }
            },
        )
        .or_else(|| {
            prior_upsert.as_ref().map(|upsert| ApprovalRestoreSnapshot {
                record_type: candidate.record_type.clone(),
                record_id: upsert.key.clone(),
                prior_status: None,
                prior_superseded_by: None,
                prior_valid_to: None,
                prior_upsert: None,
            })
        })
    else {
        return Ok(());
    };
    snapshot.prior_upsert = prior_upsert;

    connection.execute(
        "UPDATE extraction_candidates SET approval_restore = ?1 WHERE id = ?2",
        params![serde_json::to_string(&snapshot)?, candidate.id],
    )?;
    Ok(())
}

/// Capture the prior row of a context/state key (or entity name) the candidate
/// would upsert-overwrite. Returns None when nothing with that key exists (a
/// genuinely new record — undo can safely delete it).
fn snapshot_prior_upsert(
    connection: &Connection,
    candidate: &ExtractionCandidate,
) -> Result<Option<PriorUpsert>> {
    match candidate.record_type.as_str() {
        "context" => {
            let Some(key) = candidate_payload_optional_string(&candidate.payload, &["key"]) else {
                return Ok(None);
            };
            connection
                .query_row(
                    "SELECT title, content, category, scope, checksum, version
                     FROM context WHERE key = ?1",
                    [&key],
                    |row| {
                        Ok(serde_json::json!({
                            "title": row.get::<_, String>(0)?,
                            "content": row.get::<_, String>(1)?,
                            "category": row.get::<_, String>(2)?,
                            "scope": row.get::<_, String>(3)?,
                            "checksum": row.get::<_, String>(4)?,
                            "version": row.get::<_, i64>(5)?,
                        }))
                    },
                )
                .optional()
                .map(|row| {
                    row.map(|row| PriorUpsert {
                        record_type: "context".to_owned(),
                        key,
                        row: Some(row),
                    })
                })
                .map_err(Into::into)
        }
        "state" => {
            let Some(key) = candidate_payload_optional_string(&candidate.payload, &["key"]) else {
                return Ok(None);
            };
            connection
                .query_row(
                    "SELECT title, status, owner, details, blockers, depends_on, scope, priority
                     FROM state WHERE key = ?1",
                    [&key],
                    |row| {
                        Ok(serde_json::json!({
                            "title": row.get::<_, String>(0)?,
                            "status": row.get::<_, String>(1)?,
                            "owner": row.get::<_, Option<String>>(2)?,
                            "details": row.get::<_, Option<String>>(3)?,
                            "blockers": row.get::<_, String>(4)?,
                            "depends_on": row.get::<_, String>(5)?,
                            "scope": row.get::<_, String>(6)?,
                            "priority": row.get::<_, String>(7)?,
                        }))
                    },
                )
                .optional()
                .map(|row| {
                    row.map(|row| PriorUpsert {
                        record_type: "state".to_owned(),
                        key,
                        row: Some(row),
                    })
                })
                .map_err(Into::into)
        }
        "entity" => {
            // Entities upsert by slug(name). If one already exists, undo can't
            // cleanly unwind (the added observation/relation and the merge are
            // entangled), so we only record that it pre-existed and REFUSE undo.
            let Some(name) =
                candidate_payload_optional_string(&candidate.payload, &["name", "title"])
            else {
                return Ok(None);
            };
            let entity_id = entity_id_for_name(connection, &name)?;
            if entity_exists(connection, &entity_id)? {
                Ok(Some(PriorUpsert {
                    record_type: "entity".to_owned(),
                    key: entity_id,
                    row: None,
                }))
            } else {
                Ok(None)
            }
        }
        _ => Ok(None),
    }
}

fn release_candidate_approval_claim(options: &ApproveCandidateOptions) {
    if let Ok((_project, connection)) = resolve_and_open(
        options.project_name.clone(),
        options.start_dir.clone(),
        options.grafiki_home.clone(),
    ) {
        let _ = connection.execute(
            "UPDATE extraction_candidates
             SET reviewed_at = NULL,
                 approval_restore = CASE
                     WHEN trusted_record_id IS NULL THEN NULL
                     ELSE approval_restore
                 END
             WHERE id = ?1 AND status = 'pending'",
            params![&options.id],
        );
    }
}

/// Restore a pre-existing context/state record's prior contents that an
/// upsert-approval overwrote. Returns whether it restored anything (⇒ the
/// caller must NOT delete the record). Entity prior-upserts are refused earlier,
/// never restored here.
fn restore_prior_upsert(
    tx: &rusqlite::Transaction<'_>,
    prior_upsert: Option<&PriorUpsert>,
    trusted_record_id: &str,
) -> Result<bool> {
    let Some(upsert) = prior_upsert else {
        return Ok(false);
    };
    let Some(row) = &upsert.row else {
        return Ok(false);
    };
    // The overwritten record IS the trusted record (same key); a mismatch means
    // something else changed it since — don't clobber that.
    if upsert.key != trusted_record_id {
        return Ok(false);
    }
    let get = |field: &str| {
        row.get(field)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_owned()
    };
    match upsert.record_type.as_str() {
        "context" => {
            let content = get("content");
            tx.execute(
                "UPDATE context
                 SET title = ?1, content = ?2, category = ?3, scope = ?4,
                     checksum = ?5, version = ?6, retired_at = NULL,
                     updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
                 WHERE key = ?7",
                params![
                    get("title"),
                    content,
                    get("category"),
                    get("scope"),
                    get("checksum"),
                    row.get("version").and_then(|v| v.as_i64()).unwrap_or(1),
                    upsert.key,
                ],
            )?;
            let scope = get("scope");
            enqueue_embedding_job(
                tx,
                "context",
                &upsert.key,
                &scope,
                &format!("{} {}", get("title"), content),
            )?;
            Ok(true)
        }
        "state" => {
            tx.execute(
                "UPDATE state
                 SET title = ?1, status = ?2, owner = ?3, details = ?4,
                     blockers = ?5, depends_on = ?6, scope = ?7, priority = ?8,
                     updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
                 WHERE key = ?9",
                params![
                    get("title"),
                    get("status"),
                    row.get("owner").and_then(|v| v.as_str()),
                    row.get("details").and_then(|v| v.as_str()),
                    get("blockers"),
                    get("depends_on"),
                    get("scope"),
                    get("priority"),
                    upsert.key,
                ],
            )?;
            Ok(true)
        }
        _ => Ok(false),
    }
}

fn restore_candidate_supersession(
    tx: &rusqlite::Transaction<'_>,
    candidate_id: &str,
    trusted_record_type: &str,
    trusted_record_id: &str,
) -> Result<()> {
    let snapshot: Option<String> = tx.query_row(
        "SELECT approval_restore FROM extraction_candidates WHERE id = ?1",
        [candidate_id],
        |row| row.get(0),
    )?;
    let Some(snapshot) = snapshot else {
        return Ok(());
    };
    let snapshot: ApprovalRestoreSnapshot = serde_json::from_str(&snapshot)?;
    match snapshot.record_type.as_str() {
        "decision" if trusted_record_type == "decision" => {
            let Some(prior_status) = snapshot.prior_status else {
                return Ok(());
            };
            let restored = tx.execute(
                "UPDATE decisions
                 SET status = ?1, superseded_by = ?2,
                     updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
                 WHERE id = ?3 AND status = 'superseded' AND superseded_by = ?4",
                params![
                    prior_status,
                    snapshot.prior_superseded_by,
                    snapshot.record_id,
                    trusted_record_id
                ],
            )?;
            if restored > 0 && prior_status != "revoked" {
                if let Some((scope, content)) = tx
                    .query_row(
                        "SELECT scope, title || ' ' || coalesce(reasoning, '')
                         FROM decisions WHERE id = ?1",
                        [&snapshot.record_id],
                        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                    )
                    .optional()?
                {
                    enqueue_embedding_job(tx, "decision", &snapshot.record_id, &scope, &content)?;
                }
            }
        }
        "observation" if trusted_record_type == "observation" => {
            let new_valid_from: Option<String> = tx
                .query_row(
                    "SELECT valid_from FROM observations WHERE id = ?1",
                    [trusted_record_id],
                    |row| row.get(0),
                )
                .optional()?;
            let current_valid_to: Option<Option<String>> = tx
                .query_row(
                    "SELECT valid_to FROM observations WHERE id = ?1",
                    [&snapshot.record_id],
                    |row| row.get(0),
                )
                .optional()?;
            if current_valid_to.flatten() == new_valid_from {
                tx.execute(
                    "UPDATE observations SET valid_to = ?1 WHERE id = ?2",
                    params![snapshot.prior_valid_to, snapshot.record_id],
                )?;
                if snapshot.prior_valid_to.is_none() {
                    if let Some((scope, content)) = tx
                        .query_row(
                            "SELECT e.scope, o.content
                             FROM observations o JOIN entities e ON e.id = o.entity_id
                             WHERE o.id = ?1 AND e.retired_at IS NULL",
                            [&snapshot.record_id],
                            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                        )
                        .optional()?
                    {
                        enqueue_embedding_job(
                            tx,
                            "observation",
                            &snapshot.record_id,
                            &scope,
                            &content,
                        )?;
                    }
                }
            }
        }
        _ => {}
    }
    Ok(())
}

/// Undo a wrong approve: delete the trusted record it created and return the
/// candidate to `pending`. Audit finding (2026-07-04, don-norman-design-critic):
/// approve was the highest-blast-radius action in the review queue (it briefs
/// future agent sessions) yet had no way back. This is that way back — meant to
/// be called from a short-lived "Undo" affordance right after approval, not as
/// a general-purpose retraction long after the fact (the trusted record may by
/// then have been read, cited, or superseded by other memory).
pub fn revert_candidate_approval(
    options: RevertApprovalOptions,
) -> Result<CandidateMutationReport> {
    let (_project, mut connection) = resolve_and_open(
        options.project_name.clone(),
        options.start_dir.clone(),
        options.grafiki_home.clone(),
    )?;
    let candidate = load_extraction_candidate(&connection, &options.id)?;
    if candidate.status != "approved" {
        return Err(GrafikiError::InvalidCandidate(format!(
            "candidate {} is {}, not approved — nothing to undo",
            candidate.id, candidate.status
        )));
    }
    let (Some(trusted_record_type), Some(trusted_record_id)) = (
        candidate.trusted_record_type.clone(),
        candidate.trusted_record_id.clone(),
    ) else {
        return Err(GrafikiError::InvalidCandidate(format!(
            "candidate {} is approved but has no trusted record to undo",
            candidate.id
        )));
    };

    // Did approval OVERWRITE a pre-existing keyed record? If so, undo must
    // restore its prior contents (context/state) or refuse (entity) — deleting
    // the record would destroy data the candidate never owned.
    let restore_snapshot: Option<ApprovalRestoreSnapshot> = connection
        .query_row(
            "SELECT approval_restore FROM extraction_candidates WHERE id = ?1",
            [&options.id],
            |row| row.get::<_, Option<String>>(0),
        )?
        .and_then(|raw| serde_json::from_str(&raw).ok());
    let prior_upsert = restore_snapshot.and_then(|snapshot| snapshot.prior_upsert);
    if let Some(upsert) = &prior_upsert {
        if upsert.record_type == "entity" {
            return Err(GrafikiError::InvalidCandidate(format!(
                "candidate {} approved into an entity that already existed before approval — \
                 undo can't cleanly separate the merge; retire the specific fact from Browse \
                 instead",
                options.id
            )));
        }
    }

    // Entities are upserted by (name, scope) across approvals (`save_entity`),
    // so a DIFFERENT, already-approved candidate's observation/relation can
    // point at this SAME entity row. Deleting it would cascade away someone
    // else's trusted memory (observations/relations are `ON DELETE CASCADE`).
    // Refuse rather than silently corrupt shared data (2026-07-04 adversarial
    // review finding) — the reviewer can retire the specific fact from Browse.
    if trusted_record_type == "entity" {
        let (observations, relations): (i64, i64) = connection.query_row(
            "
            SELECT
                (SELECT COUNT(*) FROM observations WHERE entity_id = ?1),
                (SELECT COUNT(*) FROM relations WHERE from_entity = ?1 OR to_entity = ?1)
            ",
            params![&trusted_record_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        if observations > 0 || relations > 0 {
            return Err(GrafikiError::InvalidCandidate(format!(
                "candidate {} approved an entity that other memory now depends on \
                 ({observations} observation(s), {relations} relation(s)) — undo isn't \
                 safe here; retire the specific fact from Browse instead",
                options.id
            )));
        }
    }

    // CLAIM by reverting status FIRST, guarded on still being 'approved'. SQLite
    // serializes this UPDATE, so exactly one of two racing Undo calls succeeds
    // (mirrors approve_candidate's claim pattern — same prior audit finding,
    // same fix shape). Reordering this ahead of the trusted-record delete also
    // means a record that's already gone (deleted independently via Browse)
    // can no longer wedge the candidate forever: once this succeeds the
    // candidate IS correctly reverted, and cleaning up the underlying record
    // below is best-effort.
    let tx = connection.transaction()?;
    let claimed = tx.execute(
        "
        UPDATE extraction_candidates
        SET status = 'pending',
            trusted_record_type = NULL,
            trusted_record_id = NULL,
            reviewed_at = NULL
        WHERE id = ?1 AND status = 'approved'
        ",
        params![&options.id],
    )?;
    if claimed == 0 {
        drop(tx);
        let candidate = load_extraction_candidate(&connection, &options.id)?;
        return Err(GrafikiError::InvalidCandidate(format!(
            "candidate {} was reviewed concurrently; now {}",
            options.id, candidate.status
        )));
    }
    // Un-promote evidence back to candidate-only (mirrors promote_candidate_evidence).
    tx.execute(
        "
        UPDATE evidence_links
        SET trusted_record_type = NULL, trusted_record_id = NULL
        WHERE candidate_id = ?1
        ",
        params![&options.id],
    )?;
    restore_candidate_supersession(&tx, &options.id, &trusted_record_type, &trusted_record_id)?;
    // If approval overwrote a pre-existing context/state record, restore its
    // prior contents in the same transaction and SKIP the delete below — the
    // record must survive undo with the data it held before approval.
    let restored_prior = restore_prior_upsert(&tx, prior_upsert.as_ref(), &trusted_record_id)?;
    tx.execute(
        "UPDATE extraction_candidates SET approval_restore = NULL WHERE id = ?1",
        [&options.id],
    )?;
    tx.commit()?;
    drop(connection);

    if restored_prior {
        let (_project, connection) = resolve_and_open(
            options.project_name,
            options.start_dir,
            options.grafiki_home,
        )?;
        let candidate = load_extraction_candidate(&connection, &options.id)?;
        return Ok(CandidateMutationReport {
            candidate,
            message: "Approval undone — the record's previous contents were restored.".to_owned(),
        });
    }

    // Best-effort delete of the underlying trusted record. The candidate is
    // ALREADY correctly reverted above regardless of what happens here — an
    // error deleting a record that's independently gone (or any other delete
    // failure) must not re-wedge a candidate that just got successfully
    // un-stuck. A stray leftover trusted record is a far smaller problem than
    // the permanent-wedge bug this replaces.
    let _ = match trusted_record_type.as_str() {
        "entity" => delete_entity(DeleteEntityOptions {
            project_name: options.project_name.clone(),
            start_dir: options.start_dir.clone(),
            grafiki_home: options.grafiki_home.clone(),
            id: trusted_record_id.clone(),
        })
        .map(|_| ()),
        "observation" => delete_observation(DeleteObservationOptions {
            project_name: options.project_name.clone(),
            start_dir: options.start_dir.clone(),
            grafiki_home: options.grafiki_home.clone(),
            id: trusted_record_id.clone(),
        })
        .map(|_| ()),
        "decision" => delete_decision(DeleteDecisionOptions {
            project_name: options.project_name.clone(),
            start_dir: options.start_dir.clone(),
            grafiki_home: options.grafiki_home.clone(),
            id: trusted_record_id.clone(),
        })
        .map(|_| ()),
        "context" => delete_context(DeleteContextOptions {
            project_name: options.project_name.clone(),
            start_dir: options.start_dir.clone(),
            grafiki_home: options.grafiki_home.clone(),
            key: trusted_record_id.clone(),
        })
        .map(|_| ()),
        "state" => delete_state(DeleteStateOptions {
            project_name: options.project_name.clone(),
            start_dir: options.start_dir.clone(),
            grafiki_home: options.grafiki_home.clone(),
            key: trusted_record_id.clone(),
        })
        .map(|_| ()),
        other => Err(GrafikiError::InvalidRecordType(other.to_owned())),
    };

    let (_project, connection) = resolve_and_open(
        options.project_name,
        options.start_dir,
        options.grafiki_home,
    )?;
    let candidate = load_extraction_candidate(&connection, &options.id)?;
    Ok(CandidateMutationReport {
        candidate,
        message: "Approval undone — candidate returned to review.".to_owned(),
    })
}

/// Return a rejected candidate to `pending` so it can be reconsidered. Audit
/// finding: reject was the only terminal, unrecoverable action in the queue
/// (every button disables once `status != "pending"`) — a wrong reject had no
/// way back at all.
pub fn reopen_candidate(options: ReopenCandidateOptions) -> Result<CandidateMutationReport> {
    let (_project, connection) = resolve_and_open(
        options.project_name,
        options.start_dir,
        options.grafiki_home,
    )?;
    let candidate = load_extraction_candidate(&connection, &options.id)?;
    if candidate.status != "rejected" {
        return Err(GrafikiError::InvalidCandidate(format!(
            "candidate {} is {}, not rejected — nothing to reopen",
            candidate.id, candidate.status
        )));
    }
    connection.execute(
        "
        UPDATE extraction_candidates
        SET status = 'pending', reviewed_at = NULL
        WHERE id = ?1 AND status = 'rejected'
        ",
        params![&options.id],
    )?;
    let candidate = load_extraction_candidate(&connection, &options.id)?;
    Ok(CandidateMutationReport {
        candidate,
        message: "Candidate reopened for review.".to_owned(),
    })
}

pub fn edit_candidate(options: EditCandidateOptions) -> Result<CandidateMutationReport> {
    let (_project, connection) = resolve_and_open(
        options.project_name,
        options.start_dir,
        options.grafiki_home,
    )?;
    let candidate = load_extraction_candidate(&connection, &options.id)?;
    if candidate.status != "pending" {
        return Err(GrafikiError::InvalidCandidate(format!(
            "candidate {} is already {}",
            candidate.id, candidate.status
        )));
    }
    if candidate.reviewed_at.is_some() {
        return Err(GrafikiError::InvalidCandidate(format!(
            "candidate {} is being approved and cannot be edited",
            candidate.id
        )));
    }

    let record_type = options
        .record_type
        .as_deref()
        .map(str::trim)
        .filter(|record_type| !record_type.is_empty())
        .map(validate_candidate_record_type)
        .transpose()?
        .unwrap_or(candidate.record_type);
    let mut payload = options.payload.unwrap_or(candidate.payload);
    if !payload.is_object() {
        return Err(GrafikiError::InvalidCandidate(
            "candidate payload must be a JSON object".to_owned(),
        ));
    }
    // Same redaction as the propose path — an edited candidate must not be able
    // to persist a secret the original proposal would have scrubbed.
    redact_json_value(&mut payload);
    let scope = match options.scope {
        Some(scope) => Scope::new(scope)?.as_str().to_owned(),
        None => candidate.scope,
    };
    let confidence = options
        .confidence
        .map(validate_candidate_confidence)
        .transpose()?
        .unwrap_or(candidate.confidence);
    let rationale = options
        .rationale
        .map(|mut value| {
            redact_sensitive_text(&mut value);
            value
        })
        .or(candidate.rationale);

    let updated = connection.execute(
        "
        UPDATE extraction_candidates
        SET proposed_record_type = ?1,
            payload = ?2,
            scope = ?3,
            confidence = ?4,
            rationale = ?5
        WHERE id = ?6 AND status = 'pending' AND reviewed_at IS NULL
        ",
        params![
            record_type,
            serde_json::to_string(&payload)?,
            scope,
            confidence,
            rationale,
            &options.id
        ],
    )?;
    if updated == 0 {
        return Err(GrafikiError::InvalidCandidate(format!(
            "candidate {} changed while it was being edited",
            options.id
        )));
    }

    let candidate = load_extraction_candidate(&connection, &options.id)?;
    Ok(CandidateMutationReport {
        candidate,
        message: "Candidate updated for review.".to_owned(),
    })
}

pub fn reject_candidate(options: RejectCandidateOptions) -> Result<CandidateMutationReport> {
    let (_project, connection) = resolve_and_open(
        options.project_name,
        options.start_dir,
        options.grafiki_home,
    )?;
    let candidate = load_extraction_candidate(&connection, &options.id)?;
    if candidate.status != "pending" {
        return Err(GrafikiError::InvalidCandidate(format!(
            "candidate {} is already {}",
            candidate.id, candidate.status
        )));
    }
    if candidate.reviewed_at.is_some() {
        return Err(GrafikiError::InvalidCandidate(format!(
            "candidate {} is being approved and cannot be rejected",
            candidate.id
        )));
    }
    let rationale = options.rationale.or(candidate.rationale);
    let rejected = connection.execute(
        "
        UPDATE extraction_candidates
        SET status = 'rejected',
            rationale = ?1,
            reviewed_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
        WHERE id = ?2 AND status = 'pending' AND reviewed_at IS NULL
        ",
        params![rationale, options.id],
    )?;
    if rejected == 0 {
        return Err(GrafikiError::InvalidCandidate(format!(
            "candidate {} changed while it was being rejected",
            options.id
        )));
    }
    let candidate = load_extraction_candidate(&connection, &options.id)?;
    Ok(CandidateMutationReport {
        candidate,
        message: "Candidate rejected.".to_owned(),
    })
}

pub fn bulk_review_candidates(
    options: BulkCandidateReviewOptions,
) -> Result<BulkCandidateReviewReport> {
    let action = validate_candidate_review_action(options.action.trim())?.to_owned();
    if options.ids.is_empty() {
        return Err(GrafikiError::InvalidCandidate(
            "bulk review requires at least one candidate id".to_owned(),
        ));
    }

    let mut results = Vec::new();
    let mut errors = Vec::new();
    for id in &options.ids {
        let result = match action.as_str() {
            "approve" => approve_candidate(ApproveCandidateOptions {
                project_name: options.project_name.clone(),
                start_dir: options.start_dir.clone(),
                grafiki_home: options.grafiki_home.clone(),
                id: id.clone(),
            }),
            "reject" => reject_candidate(RejectCandidateOptions {
                project_name: options.project_name.clone(),
                start_dir: options.start_dir.clone(),
                grafiki_home: options.grafiki_home.clone(),
                id: id.clone(),
                rationale: options.rationale.clone(),
            }),
            _ => unreachable!("candidate review action was validated"),
        };

        match result {
            Ok(report) => results.push(report),
            Err(error) => errors.push(CandidateReviewError {
                id: id.clone(),
                error: error.to_string(),
            }),
        }
    }

    Ok(BulkCandidateReviewReport {
        action,
        requested: options.ids.len(),
        succeeded: results.len(),
        failed: errors.len(),
        results,
        errors,
    })
}

pub fn start_capture_session(options: StartCaptureOptions) -> Result<CaptureSessionReport> {
    let scope = Scope::new(options.scope)?;
    let (project, connection) = resolve_and_open(
        options.project_name,
        options.start_dir,
        options.grafiki_home,
    )?;
    let id = new_ulid();
    let consent_profile = options
        .consent_profile
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "local-explicit".to_owned());
    // Validate the profile at the write boundary too (the config path already
    // does), so an invalid value is rejected up front rather than silently
    // normalized to Default only at read time.
    let redaction_profile = match options
        .redaction_profile
        .filter(|value| !value.trim().is_empty())
    {
        Some(value) => {
            let normalized = value.trim().to_ascii_lowercase();
            if !matches!(normalized.as_str(), "none" | "default" | "strict") {
                return Err(GrafikiError::InvalidCaptureConfig(
                    "redaction_profile must be one of: none, default, strict".to_owned(),
                ));
            }
            normalized
        }
        None => "default".to_owned(),
    };

    connection.execute(
        "
        INSERT INTO capture_sessions
            (id, project, scope, source_app, consent_profile, redaction_profile)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6)
        ",
        params![
            id,
            project.project,
            scope.as_str(),
            options.source_app,
            consent_profile,
            redaction_profile
        ],
    )?;

    let capture = load_capture_session(&connection, &id)?;
    Ok(CaptureSessionReport {
        capture,
        message: "Capture session started.".to_owned(),
    })
}

pub fn stop_capture_session(options: StopCaptureOptions) -> Result<CaptureSessionReport> {
    let (_project, connection) = resolve_and_open(
        options.project_name,
        options.start_dir,
        options.grafiki_home,
    )?;
    connection.execute(
        "
        UPDATE capture_sessions
        SET status = 'stopped',
            ended_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
        WHERE id = ?1
        ",
        [&options.capture_id],
    )?;
    let capture = load_capture_session(&connection, &options.capture_id)?;
    Ok(CaptureSessionReport {
        capture,
        message: "Capture session stopped.".to_owned(),
    })
}

pub fn ingest_capture_event(options: IngestCaptureEventOptions) -> Result<CaptureEventReport> {
    let source_type = validate_capture_source_type(&options.source_type)?;
    let privacy_level = validate_privacy_level(
        options
            .privacy_level
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or("internal"),
    )?;
    let scope = Scope::new(options.scope)?;
    let (project, connection) = resolve_and_open(
        options.project_name,
        options.start_dir,
        options.grafiki_home,
    )?;
    let capture_id = match options.capture_id {
        Some(id) if !id.trim().is_empty() => {
            ensure_capture_session_exists(&connection, id.trim())?;
            id.trim().to_owned()
        }
        _ => match latest_active_capture_session(&connection, &project.project, scope.as_str())? {
            Some(id) => id,
            None => {
                let id = new_ulid();
                connection.execute(
                    "
                    INSERT INTO capture_sessions
                        (id, project, scope, source_app, consent_profile, redaction_profile)
                    VALUES (?1, ?2, ?3, ?4, 'local-explicit', 'default')
                    ",
                    params![id, project.project, scope.as_str(), options.source],
                )?;
                id
            }
        },
    };

    let id = new_ulid();
    // C13: honor the capture session's stored redaction profile (was inert). An
    // unreadable/legacy value falls back to Default — never to None.
    let redaction_profile = RedactionProfile::parse(
        &connection
            .query_row(
                "SELECT redaction_profile FROM capture_sessions WHERE id = ?1",
                [&capture_id],
                |row| row.get::<_, String>(0),
            )
            .unwrap_or_else(|_| "default".to_owned()),
    );
    let mut title = options.title;
    let mut text = options.text;
    // Structured fields must be redacted while they are still structured. A
    // serialized blob loses the key/value relationship, so values such as
    // `{ "client_secret": "plain-value" }` evade assignment-style text
    // detection. Preserve JSON shape, apply the capture session's profile at
    // every depth, and only then serialize for storage.
    let mut payload_value = options.payload;
    let mut metadata_value = options.metadata;
    let mut redacted = options.redacted;
    if let Some(value) = title.as_mut() {
        if redact_text_with_profile(value, redaction_profile) {
            redacted = true;
        }
    }
    if let Some(value) = text.as_mut() {
        if redact_text_with_profile(value, redaction_profile) {
            redacted = true;
        }
    }
    if let Some(value) = payload_value.as_mut() {
        if redact_json_value_with_profile(value, redaction_profile) {
            redacted = true;
        }
    }
    if let Some(value) = metadata_value.as_mut() {
        if redact_json_value_with_profile(value, redaction_profile) {
            redacted = true;
        }
    }
    let payload = payload_value
        .map(|payload| serde_json::to_string(&payload))
        .transpose()?;
    let metadata = metadata_value
        .map(|metadata| serde_json::to_string(&metadata))
        .transpose()?;
    let privacy_level = if redacted && matches!(privacy_level.as_str(), "public" | "internal") {
        "sensitive".to_owned()
    } else {
        privacy_level
    };
    // Per-source-type, scope-wide dedup: re-imported transcript/file/git/ide
    // snapshots with identical content are not re-inserted, but legitimately
    // repeated terminal/manual/agent events always are.
    let content_hash = capture_content_hash(
        &source_type,
        options.source.as_deref(),
        title.as_deref(),
        text.as_deref(),
        payload.as_deref(),
        metadata.as_deref(),
    );
    if is_dedup_source_type(&source_type) {
        if let Some(existing_id) =
            find_duplicate_capture_event(&connection, scope.as_str(), &source_type, &content_hash)?
        {
            let event = load_capture_event(&connection, &existing_id)?;
            return Ok(CaptureEventReport {
                event,
                deduplicated: true,
                message: "Capture event already ingested; skipped duplicate.".to_owned(),
            });
        }
    }

    let captured_at = options
        .captured_at
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "now".to_owned());
    let captured_at_sql = if captured_at == "now" {
        "strftime('%Y-%m-%dT%H:%M:%SZ', 'now')".to_owned()
    } else {
        "?13".to_owned()
    };
    let sql = format!(
        "
        INSERT INTO capture_events
            (id, capture_session, source_type, source, title, text, payload, metadata,
             privacy_level, redacted, scope, content_hash, captured_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, {captured_at_sql})
        "
    );

    if captured_at == "now" {
        connection.execute(
            &sql,
            params![
                id,
                capture_id,
                source_type,
                options.source,
                title,
                text,
                payload,
                metadata,
                privacy_level,
                if redacted { 1 } else { 0 },
                scope.as_str(),
                content_hash
            ],
        )?;
    } else {
        connection.execute(
            &sql,
            params![
                id,
                capture_id,
                source_type,
                options.source,
                title,
                text,
                payload,
                metadata,
                privacy_level,
                if redacted { 1 } else { 0 },
                scope.as_str(),
                content_hash,
                captured_at
            ],
        )?;
    }

    let event = load_capture_event(&connection, &id)?;
    Ok(CaptureEventReport {
        event,
        deduplicated: false,
        message: "Capture event ingested.".to_owned(),
    })
}

/// Source types whose re-ingest is idempotent (re-imports / re-snapshots should
/// not duplicate). Terminal/manual/agent/system/etc. are intentionally excluded
/// because identical content there is a legitimate repeat (e.g. running the same
/// command twice).
fn is_dedup_source_type(source_type: &str) -> bool {
    matches!(source_type, "transcript" | "file" | "git" | "ide")
}

/// Deterministic content hash for dedup (excludes captured_at). Computed after
/// redaction so a re-import hashes identically.
fn capture_content_hash(
    source_type: &str,
    source: Option<&str>,
    title: Option<&str>,
    text: Option<&str>,
    payload: Option<&str>,
    metadata: Option<&str>,
) -> String {
    let combined = format!(
        "{}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}",
        source_type,
        source.unwrap_or(""),
        title.unwrap_or(""),
        text.unwrap_or(""),
        payload.unwrap_or(""),
        metadata.unwrap_or(""),
    );
    checksum(&combined)
}

fn find_duplicate_capture_event(
    connection: &Connection,
    scope: &str,
    source_type: &str,
    content_hash: &str,
) -> Result<Option<String>> {
    connection
        .query_row(
            "
            SELECT id FROM capture_events
            WHERE scope = ?1 AND source_type = ?2 AND content_hash = ?3
            ORDER BY created_at ASC, id ASC
            LIMIT 1
            ",
            params![scope, source_type, content_hash],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(Into::into)
}

pub fn list_capture_events(options: ListCaptureEventsOptions) -> Result<Vec<CaptureEvent>> {
    let scope = Scope::new(options.scope)?;
    let scope_chain = browse_scope_chain(&scope);
    let source_type = options
        .source_type
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(validate_capture_source_type)
        .transpose()?;
    let (_project, connection) = resolve_and_open(
        options.project_name,
        options.start_dir,
        options.grafiki_home,
    )?;
    let limit = options.limit.clamp(1, 500) as i64;

    let mut clauses = vec![if scope_chain.is_empty() {
        "scope LIKE '%'".to_owned()
    } else {
        format!("scope IN ({})", placeholders(scope_chain.len()))
    }];
    if options.capture_id.is_some() {
        clauses.push("capture_session = ?".to_owned());
    }
    if source_type.is_some() {
        clauses.push("source_type = ?".to_owned());
    }
    let sql = format!(
        "
        SELECT id, capture_session, source_type, source, title, text, payload, metadata,
               privacy_level, redacted, scope, captured_at, created_at
        FROM capture_events
        WHERE {}
        ORDER BY captured_at DESC, id DESC
        LIMIT ?
        ",
        clauses.join(" AND ")
    );
    let mut params: Vec<&dyn rusqlite::ToSql> = scope_chain
        .iter()
        .map(|scope| scope as &dyn rusqlite::ToSql)
        .collect();
    if let Some(capture_id) = &options.capture_id {
        params.push(capture_id);
    }
    if let Some(source_type) = &source_type {
        params.push(source_type);
    }
    params.push(&limit);
    let mut statement = connection.prepare(&sql)?;
    let events = collect_rows(statement.query_map(params.as_slice(), capture_event_from_row)?)?;
    Ok(events)
}

/// Options for [`capture_ledger`] — the Home-screen session ledger.
pub struct CaptureLedgerOptions {
    pub project_name: Option<String>,
    pub start_dir: PathBuf,
    pub grafiki_home: Option<PathBuf>,
    pub limit: usize,
}

/// One session row in the Home ledger: when it ran, which agent, how much was
/// captured, and how many memories came out of it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerSessionItem {
    pub id: String,
    pub source_app: Option<String>,
    pub status: String,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub event_count: i64,
    /// Distinct candidates whose evidence points into this session.
    pub memory_count: i64,
    /// Up to 2 most-recent memory titles this session produced (newest first).
    /// Lets the Home ledger say what was LEARNED, not just how many raw events
    /// were captured (2026-07-04 don-norman-design-critic finding).
    #[serde(default)]
    pub recent_memory_titles: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureLedgerReport {
    pub sessions: Vec<LedgerSessionItem>,
    pub pending_candidates: i64,
    /// Titles of the newest pending candidates (for the review strip).
    pub pending_titles: Vec<String>,
    pub sessions_week: i64,
    pub memories_week: i64,
}

/// The session ledger that Home renders: recent NON-EMPTY capture sessions with
/// their extraction yield, plus review-queue and this-week counters. Read-only.
/// Up to `limit` most-recent memory titles produced by one capture session
/// (newest first), via the same evidence_links -> capture_events join
/// `capture_session_detail` uses to scope memories to a session. Untitled
/// candidates (empty '$.title') are dropped, mirroring `pending_titles` below.
fn session_recent_memory_titles(
    connection: &Connection,
    capture_id: &str,
    limit: i64,
) -> Result<Vec<String>> {
    let mut statement = connection.prepare(
        "
        SELECT COALESCE(json_extract(payload, '$.title'), '')
        FROM extraction_candidates
        WHERE id IN (
            SELECT el.candidate_id
            FROM evidence_links el
            JOIN capture_events ce ON ce.id = el.source_event_id
            WHERE ce.capture_session = ?1
        )
        ORDER BY created_at DESC
        LIMIT ?2
        ",
    )?;
    let titles = collect_rows(
        statement.query_map(params![capture_id, limit], |row| row.get::<_, String>(0))?,
    )?
    .into_iter()
    .filter(|title| !title.trim().is_empty())
    .collect();
    Ok(titles)
}

pub fn capture_ledger(options: CaptureLedgerOptions) -> Result<CaptureLedgerReport> {
    let (project, connection) = resolve_and_open(
        options.project_name,
        options.start_dir,
        options.grafiki_home,
    )?;
    let limit = options.limit.clamp(1, 200) as i64;

    let mut statement = connection.prepare(
        "
        SELECT cs.id, cs.source_app, cs.status, cs.started_at, cs.ended_at,
               COUNT(ce.id) AS event_count,
               (
                   SELECT COUNT(DISTINCT el.candidate_id)
                   FROM evidence_links el
                   JOIN capture_events linked ON linked.id = el.source_event_id
                   WHERE linked.capture_session = cs.id
               ) AS memory_count
        FROM capture_sessions cs
        JOIN capture_events ce ON ce.capture_session = cs.id
        WHERE cs.project = ?1
        GROUP BY cs.id
        ORDER BY cs.started_at DESC
        LIMIT ?2
        ",
    )?;
    let mut sessions =
        collect_rows(statement.query_map(params![project.project, limit], |row| {
            Ok(LedgerSessionItem {
                id: row.get(0)?,
                source_app: row.get(1)?,
                status: row.get(2)?,
                started_at: row.get(3)?,
                ended_at: row.get(4)?,
                event_count: row.get(5)?,
                memory_count: row.get(6)?,
                recent_memory_titles: Vec::new(),
            })
        })?)?;
    for session in &mut sessions {
        session.recent_memory_titles = session_recent_memory_titles(&connection, &session.id, 2)?;
    }

    let pending_candidates: i64 = connection.query_row(
        "SELECT COUNT(*) FROM extraction_candidates WHERE status = 'pending'",
        [],
        |row| row.get(0),
    )?;
    let mut titles_statement = connection.prepare(
        "
        SELECT COALESCE(json_extract(payload, '$.title'), '')
        FROM extraction_candidates
        WHERE status = 'pending'
        ORDER BY created_at DESC
        LIMIT 3
        ",
    )?;
    let pending_titles: Vec<String> =
        collect_rows(titles_statement.query_map([], |row| row.get::<_, String>(0))?)?
            .into_iter()
            .filter(|title| !title.trim().is_empty())
            .collect();

    // Timestamps are stored as '%Y-%m-%dT%H:%M:%SZ' strings, so the comparison
    // bound must be rendered in the SAME format (plain datetime() uses a space).
    let sessions_week: i64 = connection.query_row(
        "
        SELECT COUNT(DISTINCT cs.id)
        FROM capture_sessions cs
        JOIN capture_events ce ON ce.capture_session = cs.id
        WHERE cs.project = ?1
          AND cs.started_at >= strftime('%Y-%m-%dT%H:%M:%SZ', 'now', '-7 days')
        ",
        params![project.project],
        |row| row.get(0),
    )?;
    let memories_week: i64 = connection.query_row(
        "
        SELECT COUNT(*)
        FROM extraction_candidates
        WHERE created_at >= strftime('%Y-%m-%dT%H:%M:%SZ', 'now', '-7 days')
        ",
        [],
        |row| row.get(0),
    )?;

    Ok(CaptureLedgerReport {
        sessions,
        pending_candidates,
        pending_titles,
        sessions_week,
        memories_week,
    })
}

/// Options for [`capture_session_detail`] — one past session, fully opened.
pub struct CaptureSessionDetailOptions {
    pub project_name: Option<String>,
    pub start_dir: PathBuf,
    pub grafiki_home: Option<PathBuf>,
    pub capture_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureSessionDetail {
    pub session: LedgerSessionItem,
    /// The memories this session produced (candidates whose evidence points
    /// into it), any review status.
    pub memories: Vec<ExtractionCandidate>,
    /// The raw, redacted capture events — the audit trail.
    pub events: Vec<CaptureEvent>,
}

/// Open one session from the Home ledger: its header, the memories it
/// produced, and its raw event trail. Read-only.
pub fn capture_session_detail(
    options: CaptureSessionDetailOptions,
) -> Result<CaptureSessionDetail> {
    let (project, connection) = resolve_and_open(
        options.project_name.clone(),
        options.start_dir.clone(),
        options.grafiki_home.clone(),
    )?;

    let session = connection.query_row(
        "
        SELECT cs.id, cs.source_app, cs.status, cs.started_at, cs.ended_at,
               (SELECT COUNT(*) FROM capture_events ce WHERE ce.capture_session = cs.id),
               (
                   SELECT COUNT(DISTINCT el.candidate_id)
                   FROM evidence_links el
                   JOIN capture_events linked ON linked.id = el.source_event_id
                   WHERE linked.capture_session = cs.id
               )
        FROM capture_sessions cs
        WHERE cs.id = ?1 AND cs.project = ?2
        ",
        params![options.capture_id, project.project],
        |row| {
            Ok(LedgerSessionItem {
                id: row.get(0)?,
                source_app: row.get(1)?,
                status: row.get(2)?,
                started_at: row.get(3)?,
                ended_at: row.get(4)?,
                event_count: row.get(5)?,
                memory_count: row.get(6)?,
                // The detail view returns full ExtractionCandidate rows below
                // (with real titles) — no need to duplicate them here.
                recent_memory_titles: Vec::new(),
            })
        },
    )?;

    let mut memories_statement = connection.prepare(
        "
        SELECT DISTINCT ec.id, ec.source_type, ec.source, ec.proposed_record_type, ec.payload,
               ec.scope, ec.confidence, ec.status, ec.rationale, ec.trusted_record_type,
               ec.trusted_record_id, ec.created_at, ec.reviewed_at
        FROM extraction_candidates ec
        JOIN evidence_links el ON el.candidate_id = ec.id
        JOIN capture_events ce ON ce.id = el.source_event_id
        WHERE ce.capture_session = ?1
        ORDER BY ec.created_at DESC
        LIMIT 100
        ",
    )?;
    let mut memories = collect_rows(
        memories_statement.query_map(params![options.capture_id], extraction_candidate_from_row)?,
    )?;
    for memory in &mut memories {
        attach_candidate_evidence(&connection, memory)?;
    }

    let events = list_capture_events(ListCaptureEventsOptions {
        project_name: options.project_name,
        start_dir: options.start_dir,
        grafiki_home: options.grafiki_home,
        capture_id: Some(options.capture_id),
        source_type: None,
        scope: String::new(),
        limit: 200,
    })?;

    Ok(CaptureSessionDetail {
        session,
        memories,
        events,
    })
}

pub fn get_capture_status(options: CaptureStatusOptions) -> Result<CaptureStatusReport> {
    let scope = Scope::new(options.scope)?;
    let scope_chain = scope.chain().into_vec();
    let (project, connection) = resolve_and_open(
        options.project_name,
        options.start_dir,
        options.grafiki_home,
    )?;
    let active_sessions =
        scoped_capture_sessions(&connection, &project.project, &scope_chain, true)?;
    let recent_events = list_capture_events(ListCaptureEventsOptions {
        project_name: Some(project.project.clone()),
        start_dir: project.project_dir.clone(),
        grafiki_home: None,
        capture_id: None,
        source_type: None,
        scope: scope.as_str().to_owned(),
        limit: 20,
    })?;
    let sql = scoped_query(
        "
        SELECT COUNT(*)
        FROM capture_events
        WHERE scope IN ({scopes})
        ",
        scope_chain.len(),
    );
    let event_count: i64 =
        connection.query_row(&sql, params_from_iter(scope_chain.iter()), |row| row.get(0))?;

    Ok(CaptureStatusReport {
        project: project.project,
        scope: scope.as_str().to_owned(),
        active_sessions,
        recent_events,
        event_count,
    })
}

pub fn propose_capture_candidates(
    options: ProposeCaptureCandidatesOptions,
) -> Result<CaptureCandidateReport> {
    let capture_id = options.capture_id.clone();
    let events = list_capture_events(ListCaptureEventsOptions {
        project_name: options.project_name.clone(),
        start_dir: options.start_dir.clone(),
        grafiki_home: options.grafiki_home.clone(),
        capture_id: capture_id.clone(),
        source_type: None,
        scope: options.scope.clone(),
        limit: options.limit,
    })?;
    if events.is_empty() {
        return Ok(CaptureCandidateReport {
            capture_id,
            events_summarized: 0,
            candidates: Vec::new(),
            message: "No captured events found to summarize.".to_owned(),
        });
    }

    let summary = summarize_capture_events(&events);
    let evidence = events
        .iter()
        .take(30)
        .map(evidence_from_capture_event)
        .collect::<Vec<_>>();
    let source = capture_id
        .clone()
        .unwrap_or_else(|| "recent-capture".to_owned());
    let context_candidate = propose_candidate(ProposeCandidateOptions {
        project_name: options.project_name.clone(),
        start_dir: options.start_dir.clone(),
        grafiki_home: options.grafiki_home.clone(),
        source_type: "capture:auto".to_owned(),
        source: Some(source.clone()),
        record_type: "context".to_owned(),
        payload: serde_json::json!({
            "key": format!("capture-summary-{}", events[0].id),
            "title": "Automatic coding capture summary",
            "category": "audit",
            "content": summary,
        }),
        scope: options.scope.clone(),
        confidence: 0.7,
        rationale: Some(
            "Generated from raw transcript/IDE/screen capture events; review before trusting."
                .to_owned(),
        ),
        evidence: evidence.clone(),
    })?;
    let state_candidate = propose_candidate(ProposeCandidateOptions {
        project_name: options.project_name,
        start_dir: options.start_dir,
        grafiki_home: options.grafiki_home,
        source_type: "capture:auto".to_owned(),
        source: Some(source),
        record_type: "state".to_owned(),
        payload: serde_json::json!({
            "key": format!("capture-review-{}", events[0].id),
            "title": "Review automatic coding capture",
            "status": "needs-review",
            "priority": "medium",
            "details": summary,
        }),
        scope: options.scope,
        confidence: 0.64,
        rationale: Some(
            "Generated from raw transcript/IDE/screen capture events; review before trusting."
                .to_owned(),
        ),
        evidence,
    })?;

    Ok(CaptureCandidateReport {
        capture_id,
        events_summarized: events.len(),
        candidates: vec![context_candidate, state_candidate],
        message: "Capture events summarized into pending memory candidates.".to_owned(),
    })
}

/// Options for [`extract_capture_memory`] — LLM auto-extraction of durable memory
/// from a captured coding session ("Granola for agents").
pub struct ExtractCaptureOptions {
    pub project_name: Option<String>,
    pub start_dir: PathBuf,
    pub grafiki_home: Option<PathBuf>,
    /// Capture session to read; `None` = the latest active session.
    pub capture_id: Option<String>,
    pub scope: String,
    pub limit: usize,
    /// Local model (via Ollama) that reads the transcript. Default `gemma3:1b`.
    pub model: Option<String>,
    pub ollama_url: Option<String>,
    /// `true` = ignore `capture_id` and read every session event (transcript +
    /// terminal) newer than the durable extraction cursor, oldest first; the
    /// cursor advances ONLY after the whole pass succeeds, so a failed model
    /// call is retried instead of silently consuming the session.
    pub unextracted_only: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureExtractReport {
    pub events_read: usize,
    pub proposed: usize,
    pub message: String,
}

/// Auto-EXTRACT durable memory from a captured coding session using a local model,
/// then PROPOSE each item for the user's review (never blind-store — the guardrail
/// against a wrong extraction becoming a trusted fact). This is the intelligent
/// half of "Granola for agents": unlike [`propose_capture_candidates`] (which
/// bundles raw events into one heuristic summary), the model reads the transcript
/// and pulls out the actual decisions / conventions / gotchas as separate,
/// reviewable candidates. Best-effort: needs a running local model, so it is not a
/// default-build CI gate (the pure prompt/parse logic in [`crate::extract`] is).
pub fn extract_capture_memory(options: ExtractCaptureOptions) -> Result<CaptureExtractReport> {
    let events = if options.unextracted_only {
        list_unextracted_session_events(
            options.project_name.clone(),
            options.start_dir.clone(),
            options.grafiki_home.clone(),
            &options.scope,
            options.limit,
        )?
    } else {
        list_capture_events(ListCaptureEventsOptions {
            project_name: options.project_name.clone(),
            start_dir: options.start_dir.clone(),
            grafiki_home: options.grafiki_home.clone(),
            capture_id: options.capture_id.clone(),
            source_type: None,
            scope: options.scope.clone(),
            limit: options.limit,
        })?
    };
    if events.is_empty() {
        return Ok(CaptureExtractReport {
            events_read: 0,
            proposed: 0,
            message: "No captured events found to extract from.".to_owned(),
        });
    }

    // Assemble the transcript the model reads (already redacted at capture time).
    // Agent-TUI chrome (trust prompts, welcome banners) is scrubbed first — the
    // model turns it into junk candidates if it gets through; an event that was
    // ONLY chrome is dropped entirely.
    let transcript = events
        .iter()
        .filter_map(|event| {
            let title = event.title.as_deref().unwrap_or("");
            let text = crate::extract::scrub_agent_chrome(event.text.as_deref().unwrap_or(""));
            if text.is_empty()
                && event
                    .text
                    .as_deref()
                    .is_some_and(|raw| !raw.trim().is_empty())
            {
                return None;
            }
            Some(format!("[{}] {}\n{}", event.source_type, title, text))
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    // A session that was nothing but chrome has nothing durable in it: skip the
    // model call, but still advance the cursor below so it isn't re-read forever.
    let items = if transcript.trim().is_empty() {
        Vec::new()
    } else {
        let provider = crate::chat::OllamaProvider::new(options.ollama_url, options.model);
        let response =
            provider.complete(&crate::extract::build_extraction_messages(&transcript))?;
        crate::extract::parse_extracted_memories(&response)
            .into_iter()
            .filter(|item| !crate::extract::is_agent_chrome_memory(item))
            .collect()
    };

    let all_evidence = events
        .iter()
        .take(30)
        .map(evidence_from_capture_event)
        .collect::<Vec<_>>();
    let source = options
        .capture_id
        .clone()
        .unwrap_or_else(|| "recent-capture".to_owned());

    // Per-candidate provenance: attach the events this item plausibly came
    // from (word overlap with the item's title+content), not the whole batch.
    // Attaching all 30 events to every candidate made citations point at trust
    // screens and smalltalk — for a product whose pitch is auditable memory,
    // evidence that points at the wrong source is worse than less evidence.
    let evidence_for_item = |item: &crate::extract::ExtractedMemory| -> Vec<EvidenceInput> {
        let item_text = format!("{} {}", item.title, item.content).to_lowercase();
        let item_words: std::collections::HashSet<&str> = item_text
            .split(|c: char| !c.is_alphanumeric())
            .filter(|word| word.chars().count() >= 4)
            .collect();
        let mut scored: Vec<(usize, usize)> = events
            .iter()
            .take(30)
            .enumerate()
            .map(|(index, event)| {
                let event_text = format!(
                    "{} {}",
                    event.title.as_deref().unwrap_or(""),
                    event.text.as_deref().unwrap_or("")
                )
                .to_lowercase();
                let overlap = event_text
                    .split(|c: char| !c.is_alphanumeric())
                    .filter(|word| word.chars().count() >= 4)
                    .collect::<std::collections::HashSet<&str>>()
                    .intersection(&item_words)
                    .count();
                (index, overlap)
            })
            .collect();
        scored.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        let matched: Vec<EvidenceInput> = scored
            .iter()
            .filter(|(_, overlap)| *overlap >= 2)
            .take(5)
            .map(|(index, _)| all_evidence[*index].clone())
            .collect();
        if matched.is_empty() {
            all_evidence.clone() // no clear match: keep full provenance over none
        } else {
            matched
        }
    };

    let mut proposed = 0;
    for item in &items {
        let payload = if item.kind == "decision" {
            serde_json::json!({ "title": item.title, "reasoning": item.content })
        } else {
            serde_json::json!({
                "key": format!("capture-{}", slugify(&item.title)),
                "title": item.title,
                "category": "reference",
                "content": item.content,
            })
        };
        let record_type = if item.kind == "decision" {
            "decision"
        } else {
            "context"
        };
        propose_candidate(ProposeCandidateOptions {
            project_name: options.project_name.clone(),
            start_dir: options.start_dir.clone(),
            grafiki_home: options.grafiki_home.clone(),
            source_type: "capture:llm".to_owned(),
            source: Some(source.clone()),
            record_type: record_type.to_owned(),
            payload,
            scope: options.scope.clone(),
            confidence: 0.6,
            rationale: Some(
                "Auto-extracted from your coding session by the local model; review before trusting."
                    .to_owned(),
            ),
            evidence: evidence_for_item(item),
        })?;
        proposed += 1;
    }

    // Advance the durable cursor only now — model call AND every proposal
    // succeeded, so nothing can be lost. On any earlier error the cursor stays
    // put and the next pass re-reads these events (fix for consume-on-failure).
    if options.unextracted_only {
        if let Some(last) = events.last() {
            advance_extraction_cursor(
                options.project_name,
                options.start_dir,
                options.grafiki_home,
                &last.id,
            )?;
        }
    }

    Ok(CaptureExtractReport {
        events_read: events.len(),
        proposed,
        message: format!(
            "Read {} event(s); proposed {} memory candidate(s) for review.",
            events.len(),
            proposed
        ),
    })
}

/// Name of the single durable auto-extraction cursor in `capture_cursors`.
const EXTRACTION_CURSOR: &str = "auto-extract";

/// Session events (agent transcript turns + hosted-terminal output) newer than
/// the extraction cursor, OLDEST FIRST so the cursor can advance monotonically.
fn list_unextracted_session_events(
    project_name: Option<String>,
    start_dir: PathBuf,
    grafiki_home: Option<PathBuf>,
    scope: &str,
    limit: usize,
) -> Result<Vec<CaptureEvent>> {
    let scope = Scope::new(scope)?;
    let scope_chain = browse_scope_chain(&scope);
    let (_project, connection) = resolve_and_open(project_name, start_dir, grafiki_home)?;
    let limit = limit.clamp(1, 500) as i64;
    let cursor: String = connection
        .query_row(
            "SELECT last_event_id FROM capture_cursors WHERE name = ?1",
            params![EXTRACTION_CURSOR],
            |row| row.get(0),
        )
        .optional()?
        .unwrap_or_default();

    // The root (empty) scope resolves to an empty chain; `scope IN ()` is always
    // false in SQLite, which would silently hide every default-scope event from
    // extraction (the same guard list_capture_events applies above).
    let scope_clause = if scope_chain.is_empty() {
        "scope LIKE '%'".to_owned()
    } else {
        format!("scope IN ({})", placeholders(scope_chain.len()))
    };
    let sql = format!(
        "
        SELECT id, capture_session, source_type, source, title, text, payload, metadata,
               privacy_level, redacted, scope, captured_at, created_at
        FROM capture_events
        WHERE {scope_clause} AND source_type IN ('transcript', 'terminal') AND id > ?
        ORDER BY id ASC
        LIMIT ?
        "
    );
    let mut params: Vec<&dyn rusqlite::ToSql> = scope_chain
        .iter()
        .map(|scope| scope as &dyn rusqlite::ToSql)
        .collect();
    params.push(&cursor);
    params.push(&limit);
    let mut statement = connection.prepare(&sql)?;
    let events = collect_rows(statement.query_map(params.as_slice(), capture_event_from_row)?)?;
    Ok(events)
}

/// Persist extraction progress (ULID ids are time-ordered, so `MAX` semantics
/// hold with plain string comparison).
fn advance_extraction_cursor(
    project_name: Option<String>,
    start_dir: PathBuf,
    grafiki_home: Option<PathBuf>,
    last_event_id: &str,
) -> Result<()> {
    let (_project, connection) = resolve_and_open(project_name, start_dir, grafiki_home)?;
    connection.execute(
        "
        INSERT INTO capture_cursors (name, last_event_id)
        VALUES (?1, ?2)
        ON CONFLICT(name) DO UPDATE SET
            last_event_id = MAX(last_event_id, excluded.last_event_id),
            updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
        ",
        params![EXTRACTION_CURSOR, last_event_id],
    )?;
    Ok(())
}

/// Options for [`run_capture_watch`] — one pass of the passive "Granola for
/// agents" watcher.
pub struct RunCaptureWatchOptions {
    pub project_name: Option<String>,
    pub start_dir: PathBuf,
    pub grafiki_home: Option<PathBuf>,
    /// Agent whose transcript to read (default `claude-code`).
    pub agent: String,
    /// Explicit transcript path; `None` auto-discovers **this project's** dir
    /// (scoped — not every project under `~/.claude/projects`).
    pub input: Option<PathBuf>,
    pub scope: String,
    pub limit: usize,
    pub model: Option<String>,
    pub ollama_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureWatchReport {
    pub events_imported: usize,
    pub proposed: usize,
    pub message: String,
}

/// One pass of the passive "Granola for agents" watcher: import new transcript
/// activity for THIS project (incremental — per-source dedup skips already-seen
/// events) and, only if something new arrived, run the local-model extraction to
/// propose durable memory for review. No model call when nothing is new. This is
/// the reusable tick shared by the CLI (`grafiki capture extract`) and the
/// background daemon worker (`spawn_capture_worker`). It NEVER trusts anything —
/// extracted items land in the candidate review gate.
pub fn run_capture_watch(options: RunCaptureWatchOptions) -> Result<CaptureWatchReport> {
    // Re-read consent on EVERY tick. This is intentionally before transcript
    // discovery/import and uses the non-creating loader: disabling capture,
    // deleting the consent file, or corrupting it takes effect immediately and
    // cannot fail open into passive collection.
    let capture_config =
        crate::project::load_existing_capture_config(crate::project::CaptureConfigOptions {
            project_name: options.project_name.clone(),
            start_dir: options.start_dir.clone(),
            grafiki_home: options.grafiki_home.clone(),
        })?;
    if !capture_config.config.sources.transcripts {
        return Err(GrafikiError::InvalidCaptureConfig(
            "passive transcript capture is disabled for this project".to_owned(),
        ));
    }

    // Scope Claude Code auto-discovery to THIS project's transcript directory, so
    // the watcher never imports other projects' sessions.
    let input = match (options.input, options.agent.as_str()) {
        (Some(path), _) => Some(path),
        (None, "claude-code") => crate::transcript::claude_code_project_dir(&options.start_dir),
        (None, _) => None,
    };

    let import = crate::transcript::import_agent_transcripts(
        crate::transcript::ImportAgentTranscriptsOptions {
            project_name: options.project_name.clone(),
            start_dir: options.start_dir.clone(),
            grafiki_home: options.grafiki_home.clone(),
            agent: options.agent,
            input,
            scope: options.scope.clone(),
            limit: options.limit,
            summarize: false,
        },
    )?;

    // Extract everything the cursor hasn't covered yet — NOT just this pass's
    // import. That includes hosted-terminal output and, crucially, events a
    // previous pass imported but failed to extract (model down): import-dedup
    // makes re-imports 0, so gating on events_imported would consume them.
    let extract = extract_capture_memory(ExtractCaptureOptions {
        project_name: options.project_name,
        start_dir: options.start_dir,
        grafiki_home: options.grafiki_home,
        capture_id: None,
        scope: options.scope,
        limit: options.limit,
        model: options.model,
        ollama_url: options.ollama_url,
        unextracted_only: true,
    })?;

    if extract.events_read == 0 {
        return Ok(CaptureWatchReport {
            events_imported: import.events_imported,
            proposed: 0,
            message: "No new session activity to capture.".to_owned(),
        });
    }

    Ok(CaptureWatchReport {
        events_imported: import.events_imported,
        proposed: extract.proposed,
        message: format!(
            "Captured {} new event(s); proposed {} memory candidate(s) for review.",
            extract.events_read, extract.proposed
        ),
    })
}

pub fn get_status(options: StatusOptions) -> Result<StatusReport> {
    let scope = Scope::new(options.scope)?;
    let scope_chain = scope.chain().into_vec();
    let (project, connection) = resolve_and_open(
        options.project_name,
        options.start_dir,
        options.grafiki_home,
    )?;

    Ok(StatusReport {
        project: project.project,
        scope: scope.as_str().to_owned(),
        active_sessions: status_active_sessions(&connection, &scope_chain)?,
        active_state: status_active_state(&connection, &scope_chain)?,
        recent_decisions: status_recent_decisions(&connection, &scope_chain)?,
        recent_events: status_recent_events(&connection, &scope_chain)?,
    })
}

pub fn upsert_state(options: UpsertStateOptions) -> Result<StateReport> {
    let status = validate_state_status(options.status.trim())?;
    let priority = validate_state_priority(options.priority.trim())?;
    let scope = Scope::new(options.scope)?;
    let (project, mut connection) = resolve_and_open(
        options.project_name,
        options.start_dir,
        options.grafiki_home,
    )?;
    let active_session = latest_active_session(&connection, &project.project)?;
    let existing_id = state_id_for_key(&connection, &options.key)?;
    let id = existing_id.unwrap_or_else(new_ulid);

    let tx = connection.transaction()?;
    tx.execute(
        "
        INSERT INTO state (id, key, title, status, owner, details, blockers, depends_on, scope, priority)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
        ON CONFLICT(key) DO UPDATE SET
            title = excluded.title,
            status = excluded.status,
            owner = excluded.owner,
            details = excluded.details,
            blockers = excluded.blockers,
            depends_on = excluded.depends_on,
            scope = excluded.scope,
            priority = excluded.priority,
            updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
        ",
        params![
            id,
            options.key.trim(),
            clamp_field(options.title.trim(), MAX_TITLE_CHARS),
            status,
            options.owner,
            options.details.map(|value| clamp_field(&value, MAX_BODY_CHARS)),
            json_array(&options.blockers)?,
            json_array(&options.depends_on)?,
            scope.as_str(),
            priority
        ],
    )?;
    tx.execute(
        "
        INSERT INTO events (id, event_type, source_session, target_type, target_id, scope, summary)
        VALUES (?1, 'state_changed', ?2, 'state', ?3, ?4, ?5)
        ",
        params![
            new_ulid(),
            active_session,
            options.key,
            scope.as_str(),
            format!("State {} is now {}", options.key.trim(), status)
        ],
    )?;
    tx.commit()?;

    Ok(StateReport {
        id,
        key: options.key,
        title: options.title,
        status,
        scope: scope.as_str().to_owned(),
        priority,
    })
}

pub fn list_state(options: StateListOptions) -> Result<Vec<StateItem>> {
    let scope = Scope::new(options.scope)?;
    let scope_chain = browse_scope_chain(&scope);
    let (_project, connection) = resolve_and_open(
        options.project_name,
        options.start_dir,
        options.grafiki_home,
    )?;

    match options.status {
        Some(status) => {
            let status = validate_state_status(status.trim())?;
            let sql = scoped_query(
                "
                SELECT key, title, status, priority, owner, scope, details, blockers, depends_on
                FROM state
                WHERE status = ? AND scope IN ({scopes})
                ORDER BY updated_at DESC
                ",
                scope_chain.len(),
            );
            let mut params: Vec<&dyn rusqlite::ToSql> = vec![&status];
            params.extend(
                scope_chain
                    .iter()
                    .map(|scope| scope as &dyn rusqlite::ToSql),
            );
            let mut statement = connection.prepare(&sql)?;
            let rows = statement.query_map(params.as_slice(), state_item_from_row)?;
            collect_rows(rows)
        }
        None => query_scoped_rows(
            &connection,
            "
            SELECT key, title, status, priority, owner, scope, details, blockers, depends_on
            FROM state
            WHERE status != 'abandoned' AND scope IN ({scopes})
            ORDER BY updated_at DESC
            ",
            &scope_chain,
            state_item_from_row,
        ),
    }
}

pub fn delete_state(options: DeleteStateOptions) -> Result<StateReport> {
    let (project, mut connection) = resolve_and_open(
        options.project_name,
        options.start_dir,
        options.grafiki_home,
    )?;
    let active_session = latest_active_session(&connection, &project.project)?;
    let mut existing = load_state_report(&connection, &options.key)?;

    let tx = connection.transaction()?;
    tx.execute(
        "UPDATE state
         SET status = 'abandoned', updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
         WHERE key = ?1",
        [&options.key],
    )?;
    tx.execute(
        "
        INSERT INTO events (id, event_type, source_session, target_type, target_id, scope, summary)
        VALUES (?1, 'state_changed', ?2, 'state', ?3, ?4, ?5)
        ",
        params![
            new_ulid(),
            active_session,
            options.key,
            existing.scope,
            format!("Retired state {} as abandoned", existing.key)
        ],
    )?;
    tx.commit()?;

    existing.status = "abandoned".to_owned();
    Ok(existing)
}

pub fn list_events(options: EventListOptions) -> Result<EventListReport> {
    let scope = Scope::new(options.scope)?;
    let scope_chain = browse_scope_chain(&scope);
    let (project, connection) = resolve_and_open(
        options.project_name,
        options.start_dir,
        options.grafiki_home,
    )?;
    let limit = options.limit.clamp(1, 200) as i64;

    let events = match options.since {
        Some(since) => {
            let sql = scoped_query(
                "
                SELECT id, event_type, source_session, target_type, target_id, scope, summary, created_at
                FROM events
                WHERE id > ? AND scope IN ({scopes})
                ORDER BY id DESC
                LIMIT ?
                ",
                scope_chain.len(),
            );
            let mut params: Vec<&dyn rusqlite::ToSql> = vec![&since];
            params.extend(
                scope_chain
                    .iter()
                    .map(|scope| scope as &dyn rusqlite::ToSql),
            );
            params.push(&limit);
            let mut statement = connection.prepare(&sql)?;
            let rows = statement.query_map(params.as_slice(), event_item_from_row)?;
            collect_rows(rows)?
        }
        None => {
            let sql = scoped_query(
                "
                SELECT id, event_type, source_session, target_type, target_id, scope, summary, created_at
                FROM events
                WHERE scope IN ({scopes})
                ORDER BY id DESC
                LIMIT ?
                ",
                scope_chain.len(),
            );
            let mut params: Vec<&dyn rusqlite::ToSql> = scope_chain
                .iter()
                .map(|scope| scope as &dyn rusqlite::ToSql)
                .collect();
            params.push(&limit);
            let mut statement = connection.prepare(&sql)?;
            let rows = statement.query_map(params.as_slice(), event_item_from_row)?;
            collect_rows(rows)?
        }
    };

    Ok(EventListReport {
        project: project.project,
        events,
    })
}

pub fn list_sessions(options: SessionLogOptions) -> Result<SessionLogReport> {
    let scope = Scope::new(options.scope)?;
    let scope_chain = browse_scope_chain(&scope);
    let (project, connection) = resolve_and_open(
        options.project_name,
        options.start_dir,
        options.grafiki_home,
    )?;
    let limit = options.limit.clamp(1, 200) as i64;

    let sessions = match options.session_type {
        Some(session_type) => {
            let session_type = validate_session_type_for_filter(session_type.trim())?;
            let sql = scoped_query(
                "
                SELECT id, session_type, status, scope, goal, summary, accomplishments, remaining,
                       files_changed, decisions_made, entities_touched, handoff_context,
                       parent_session, child_session, started_at, ended_at
                FROM sessions
                WHERE project = ? AND session_type = ? AND scope IN ({scopes})
                ORDER BY started_at DESC, id DESC
                LIMIT ?
                ",
                scope_chain.len(),
            );
            let mut params: Vec<&dyn rusqlite::ToSql> = vec![&project.project, &session_type];
            params.extend(
                scope_chain
                    .iter()
                    .map(|scope| scope as &dyn rusqlite::ToSql),
            );
            params.push(&limit);
            let mut statement = connection.prepare(&sql)?;
            let rows = statement.query_map(params.as_slice(), session_log_item_from_row)?;
            collect_rows(rows)?
        }
        None => {
            let sql = scoped_query(
                "
                SELECT id, session_type, status, scope, goal, summary, accomplishments, remaining,
                       files_changed, decisions_made, entities_touched, handoff_context,
                       parent_session, child_session, started_at, ended_at
                FROM sessions
                WHERE project = ? AND scope IN ({scopes})
                ORDER BY started_at DESC, id DESC
                LIMIT ?
                ",
                scope_chain.len(),
            );
            let mut params: Vec<&dyn rusqlite::ToSql> = vec![&project.project];
            params.extend(
                scope_chain
                    .iter()
                    .map(|scope| scope as &dyn rusqlite::ToSql),
            );
            params.push(&limit);
            let mut statement = connection.prepare(&sql)?;
            let rows = statement.query_map(params.as_slice(), session_log_item_from_row)?;
            collect_rows(rows)?
        }
    };

    Ok(SessionLogReport {
        project: project.project,
        sessions,
    })
}

pub fn add_context(options: AddContextOptions) -> Result<ContextReport> {
    let category = validate_context_category(options.category.trim())?;
    let scope = Scope::new(options.scope)?;
    let (project, mut connection) = resolve_and_open(
        options.project_name,
        options.start_dir,
        options.grafiki_home,
    )?;
    let active_session = latest_active_session(&connection, &project.project)?;
    let id = new_ulid();
    let key = options.key.trim().to_owned();
    let title = clamp_field(options.title.trim(), MAX_TITLE_CHARS);
    // Clamp BEFORE the checksum so the stored checksum matches the stored body.
    let content = clamp_field(&options.content, MAX_BODY_CHARS);
    let checksum = checksum(&content);
    let context_embedding_text = format!("{} {}", title, content.trim());

    // `context.key` is globally UNIQUE. Re-adding an existing key UPSERTS (bumps
    // the version) instead of failing the constraint — "add" is create-or-update,
    // mirroring `upsert_state`. This also turns the candidate-approval path
    // (`approve_candidate_payload`'s `context` arm) into a graceful update rather
    // than a crash on a colliding key.
    let existing_version: Option<i64> = connection
        .query_row(
            "SELECT version FROM context WHERE key = ?1",
            [&key],
            |row| row.get(0),
        )
        .optional()?;
    let (version, event_type, verb) = match existing_version {
        Some(version) => (version + 1, "context_updated", "Updated"),
        None => (1, "context_added", "Added"),
    };

    let tx = connection.transaction()?;
    tx.execute(
        "
        INSERT INTO context (id, key, title, content, category, scope, checksum)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        ON CONFLICT(key) DO UPDATE SET
            title = excluded.title,
            content = excluded.content,
            category = excluded.category,
            scope = excluded.scope,
            checksum = excluded.checksum,
            version = context.version + 1,
            retired_at = NULL,
            updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
        ",
        params![id, key, title, content, category, scope.as_str(), checksum],
    )?;
    enqueue_embedding_job(
        &tx,
        "context",
        &key,
        scope.as_str(),
        &context_embedding_text,
    )?;
    tx.execute(
        "
        INSERT INTO events (id, event_type, source_session, target_type, target_id, scope, summary)
        VALUES (?1, ?2, ?3, 'context', ?4, ?5, ?6)
        ",
        params![
            new_ulid(),
            event_type,
            active_session,
            key,
            scope.as_str(),
            format!("{verb} context {title}")
        ],
    )?;
    tx.commit()?;

    Ok(ContextReport {
        key,
        title,
        category,
        scope: scope.as_str().to_owned(),
        version,
    })
}

pub fn get_context(options: GetContextOptions) -> Result<ContextDocument> {
    let (_project, connection) = resolve_and_open(
        options.project_name,
        options.start_dir,
        options.grafiki_home,
    )?;
    load_context_document(&connection, &options.key)
}

pub fn get_memory_record_detail(options: GetMemoryRecordOptions) -> Result<MemoryRecordDetail> {
    let record_type = normalize_memory_record_type(&options.record_type)?;
    let id = options.id.trim().to_owned();
    if id.is_empty() {
        return Err(GrafikiError::InvalidRecordType(
            "record id is required".to_owned(),
        ));
    }

    let scope = Scope::new(options.scope.trim())?;
    let scope_text = scope.as_str().to_owned();
    let project_name = options.project_name.clone();
    let start_dir = options.start_dir.clone();
    let grafiki_home = options.grafiki_home.clone();
    let bundle = export_memory(ExportOptions {
        project_name: options.project_name.clone(),
        start_dir: options.start_dir.clone(),
        grafiki_home: options.grafiki_home.clone(),
        scope: scope_text.clone(),
    })?;
    let events = detail_events(
        project_name.clone(),
        start_dir.clone(),
        grafiki_home.clone(),
        &scope_text,
        &record_type,
        &id,
    );

    match record_type.as_str() {
        "entity" => {
            let entity = bundle
                .entities
                .iter()
                .find(|entity| entity.id == id)
                .ok_or_else(|| GrafikiError::EntityNotFound(id.clone()))?;
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
                    detail_metadata("entity type", &entity.entity_type),
                    detail_metadata("scope", display_scope(&entity.scope)),
                    detail_metadata("relations", related.len().to_string()),
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
                .ok_or_else(|| GrafikiError::ObservationNotFound(id.clone()))?;
            let entity_title = entity_title(&bundle, &observation.entity_id);
            Ok(MemoryRecordDetail {
                record_type,
                id: observation.id.clone(),
                title: entity_title.clone(),
                scope: observation.scope.clone(),
                body: observation.content.clone(),
                metadata: vec![
                    detail_metadata("entity", &observation.entity_id),
                    detail_metadata("category", &observation.category),
                    detail_metadata("confidence", format!("{:.2}", observation.confidence)),
                    detail_metadata("scope", display_scope(&observation.scope)),
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
                .ok_or_else(|| GrafikiError::DecisionNotFound(id.clone()))?;
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
                    detail_metadata("status", &decision.status),
                    detail_metadata("scope", display_scope(&decision.scope)),
                ],
                related: Vec::new(),
                events,
                focus_entity_id: None,
            })
        }
        "context" => {
            let document = get_context(GetContextOptions {
                project_name,
                start_dir,
                grafiki_home,
                key: id,
            })?;
            Ok(MemoryRecordDetail {
                record_type,
                id: document.key.clone(),
                title: document.title.clone(),
                scope: document.scope.clone(),
                body: document.content.clone(),
                metadata: vec![
                    detail_metadata("category", &document.category),
                    detail_metadata("version", document.version.to_string()),
                    detail_metadata("scope", display_scope(&document.scope)),
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
                .ok_or_else(|| GrafikiError::StateNotFound(id.clone()))?;
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
                    detail_metadata("status", &state.status),
                    detail_metadata("priority", &state.priority),
                    detail_metadata("owner", state.owner.as_deref().unwrap_or("unassigned")),
                    detail_metadata("scope", display_scope(&state.scope)),
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
                .ok_or_else(|| GrafikiError::RelationNotFound(id.clone()))?;
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
                    detail_metadata("relation", &relation.relation),
                    detail_metadata("weight", format!("{:.2}", relation.weight)),
                    detail_metadata("confidence", format!("{:.2}", relation.confidence)),
                    detail_metadata("source type", &relation.source_type),
                    detail_metadata("source", relation.source.as_deref().unwrap_or("")),
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
                .ok_or_else(|| GrafikiError::SessionNotFound(id.clone()))?;
            let mut metadata_items = vec![
                detail_metadata("type", &session.session_type),
                detail_metadata("status", &session.status),
                detail_metadata("started", &session.started_at),
                detail_metadata("ended", session.ended_at.as_deref().unwrap_or("active")),
                detail_metadata("scope", display_scope(&session.scope)),
                detail_metadata("accomplishments", session.accomplishments.join(", ")),
                detail_metadata("remaining", session.remaining.join(", ")),
                detail_metadata("files changed", session.files_changed.join(", ")),
                detail_metadata("decisions made", session.decisions_made.join(", ")),
                detail_metadata("entities touched", session.entities_touched.join(", ")),
            ];
            if let Some(parent_session) = &session.parent_session {
                metadata_items.push(detail_metadata("parent session", parent_session));
            }
            if let Some(child_session) = &session.child_session {
                metadata_items.push(detail_metadata("child session", child_session));
            }
            if let Some(handoff_context) = &session.handoff_context {
                metadata_items.push(detail_metadata("handoff context", handoff_context));
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
        other => Err(GrafikiError::InvalidRecordType(other.to_owned())),
    }
}

pub fn list_context(options: ContextListOptions) -> Result<Vec<ContextSummary>> {
    let scope = Scope::new(options.scope)?;
    let scope_chain = browse_scope_chain(&scope);
    let (_project, connection) = resolve_and_open(
        options.project_name,
        options.start_dir,
        options.grafiki_home,
    )?;

    match options.category {
        Some(category) => {
            let category = validate_context_category(category.trim())?;
            let sql = scoped_query(
                "
                SELECT key, title, category, scope, version
                FROM context
                WHERE retired_at IS NULL AND category = ? AND scope IN ({scopes})
                ORDER BY updated_at DESC
                ",
                scope_chain.len(),
            );
            let mut params: Vec<&dyn rusqlite::ToSql> = vec![&category];
            params.extend(
                scope_chain
                    .iter()
                    .map(|scope| scope as &dyn rusqlite::ToSql),
            );
            let mut statement = connection.prepare(&sql)?;
            let rows = statement.query_map(params.as_slice(), context_summary_from_row)?;
            collect_rows(rows)
        }
        None => query_scoped_rows(
            &connection,
            "
            SELECT key, title, category, scope, version
            FROM context
            WHERE retired_at IS NULL AND scope IN ({scopes})
            ORDER BY updated_at DESC
            ",
            &scope_chain,
            context_summary_from_row,
        ),
    }
}

pub fn update_context(options: UpdateContextOptions) -> Result<ContextReport> {
    let scope = options.scope.as_deref().map(Scope::new).transpose()?;
    let category = options
        .category
        .as_deref()
        .map(|category| validate_context_category(category.trim()))
        .transpose()?;
    let (project, mut connection) = resolve_and_open(
        options.project_name,
        options.start_dir,
        options.grafiki_home,
    )?;
    let active_session = latest_active_session(&connection, &project.project)?;
    let existing = load_context_document(&connection, &options.key)?;
    let title = options
        .title
        .map(|value| clamp_field(&value, MAX_TITLE_CHARS))
        .unwrap_or(existing.title);
    let category = category.unwrap_or(existing.category);
    let scope = scope
        .map(|scope| scope.as_str().to_owned())
        .unwrap_or(existing.scope);
    let content = options
        .content
        .map(|value| clamp_field(&value, MAX_BODY_CHARS))
        .unwrap_or(existing.content);
    let checksum = checksum(&content);
    let next_version = existing.version + 1;
    let context_embedding_text = format!("{} {}", title.trim(), content.trim());

    let tx = connection.transaction()?;
    tx.execute(
        "
        UPDATE context
        SET title = ?1,
            content = ?2,
            category = ?3,
            scope = ?4,
            version = ?5,
            checksum = ?6,
            updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
        WHERE key = ?7
        ",
        params![
            title,
            content,
            category,
            scope,
            next_version,
            checksum,
            options.key
        ],
    )?;
    tx.execute(
        "
        INSERT INTO events (id, event_type, source_session, target_type, target_id, scope, summary)
        VALUES (?1, 'context_updated', ?2, 'context', ?3, ?4, ?5)
        ",
        params![
            new_ulid(),
            active_session,
            options.key,
            scope,
            format!("Updated context {}", title)
        ],
    )?;
    enqueue_embedding_job(
        &tx,
        "context",
        &options.key,
        &scope,
        &context_embedding_text,
    )?;
    tx.commit()?;

    Ok(ContextReport {
        key: options.key,
        title,
        category,
        scope,
        version: next_version,
    })
}

pub fn delete_context(options: DeleteContextOptions) -> Result<ContextReport> {
    let (project, mut connection) = resolve_and_open(
        options.project_name,
        options.start_dir,
        options.grafiki_home,
    )?;
    let active_session = latest_active_session(&connection, &project.project)?;
    let existing = load_context_document(&connection, &options.key)?;

    let tx = connection.transaction()?;
    delete_embedding_records(&tx, "context", &options.key)?;
    tx.execute(
        "UPDATE context
         SET retired_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now'),
             updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
         WHERE key = ?1",
        [&options.key],
    )?;
    tx.execute(
        "
        INSERT INTO events (id, event_type, source_session, target_type, target_id, scope, summary)
        VALUES (?1, 'context_deleted', ?2, 'context', ?3, ?4, ?5)
        ",
        params![
            new_ulid(),
            active_session,
            options.key,
            existing.scope,
            format!("Deleted context {}", existing.title)
        ],
    )?;
    tx.commit()?;

    Ok(ContextReport {
        key: options.key,
        title: existing.title,
        category: existing.category,
        scope: existing.scope,
        version: existing.version,
    })
}

pub(crate) fn resolve_and_open(
    project_name: Option<String>,
    start_dir: PathBuf,
    grafiki_home: Option<PathBuf>,
) -> Result<(ProjectContext, Connection)> {
    let project = resolve_project(ProjectResolveOptions {
        project_name,
        start_dir,
        grafiki_home,
    })?;
    #[cfg(feature = "sqlite-vec")]
    crate::embeddings::register_sqlite_vec()?;
    let mut connection = open_project_database(&project.db_path)?;
    initialize_schema(&mut connection)?;
    Ok((project, connection))
}

fn latest_active_session(connection: &Connection, project: &str) -> Result<Option<String>> {
    connection
        .query_row(
            "
            SELECT id
            FROM sessions
            WHERE project = ?1 AND status = 'active'
            ORDER BY started_at DESC, id DESC
            LIMIT 1
            ",
            [project],
            |row| row.get(0),
        )
        .optional()
        .map_err(Into::into)
}

fn ensure_session_exists(connection: &Connection, session_id: &str) -> Result<String> {
    connection
        .query_row(
            "SELECT id FROM sessions WHERE id = ?1",
            [session_id],
            |row| row.get(0),
        )
        .optional()?
        .ok_or_else(|| GrafikiError::SessionNotFound(session_id.to_owned()))
}

fn entity_exists(connection: &Connection, entity_id: &str) -> Result<bool> {
    let count: i64 = connection.query_row(
        "SELECT COUNT(*) FROM entities WHERE id = ?1 AND retired_at IS NULL",
        [entity_id],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

fn state_id_for_key(connection: &Connection, key: &str) -> Result<Option<String>> {
    connection
        .query_row("SELECT id FROM state WHERE key = ?1", [key], |row| {
            row.get(0)
        })
        .optional()
        .map_err(Into::into)
}

fn load_state_report(connection: &Connection, key: &str) -> Result<StateReport> {
    connection
        .query_row(
            "
            SELECT id, key, title, status, scope, priority
            FROM state
            WHERE key = ?1
            ",
            [key],
            |row| {
                Ok(StateReport {
                    id: row.get(0)?,
                    key: row.get(1)?,
                    title: row.get(2)?,
                    status: row.get(3)?,
                    scope: row.get(4)?,
                    priority: row.get(5)?,
                })
            },
        )
        .optional()?
        .ok_or_else(|| GrafikiError::StateNotFound(key.to_owned()))
}

fn load_decision_item(connection: &Connection, id: &str) -> Result<DecisionItem> {
    connection
        .query_row(
            "
            SELECT id, title, status, scope, reasoning
            FROM decisions
            WHERE id = ?1
            ",
            [id],
            decision_item_from_row,
        )
        .optional()?
        .ok_or_else(|| GrafikiError::DecisionNotFound(id.to_owned()))
}

fn load_observation_item(connection: &Connection, id: &str) -> Result<ObservationItem> {
    connection
        .query_row(
            "
            SELECT o.id, o.entity_id, e.name, o.content, o.category, o.confidence, e.scope
            FROM observations o
            JOIN entities e ON e.id = o.entity_id
            WHERE o.id = ?1 AND o.valid_to IS NULL AND e.retired_at IS NULL
            ",
            [id],
            observation_item_from_row,
        )
        .optional()?
        .ok_or_else(|| GrafikiError::ObservationNotFound(id.to_owned()))
}

fn observation_ids_for_entity(connection: &Connection, entity_id: &str) -> Result<Vec<String>> {
    let mut statement = connection.prepare(
        "
        SELECT id
        FROM observations
        WHERE entity_id = ?1
        ",
    )?;
    let rows = statement.query_map([entity_id], |row| row.get(0))?;
    collect_rows(rows)
}

trait BoolExt {
    fn not(self) -> bool;
}

impl BoolExt for bool {
    fn not(self) -> bool {
        !self
    }
}

fn validate_end_status(raw: &str) -> Result<String> {
    match raw {
        "completed" | "abandoned" | "handed-off" => Ok(raw.to_owned()),
        _ => Err(GrafikiError::InvalidSessionStatus(raw.to_owned())),
    }
}

fn validate_session_status(raw: &str) -> Result<String> {
    match raw {
        "active" | "completed" | "abandoned" | "handed-off" => Ok(raw.to_owned()),
        _ => Err(GrafikiError::InvalidSessionStatus(raw.to_owned())),
    }
}

fn validate_session_type_for_filter(raw: &str) -> Result<String> {
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

fn validate_state_status(raw: &str) -> Result<String> {
    const STATUSES: &[&str] = &[
        "planned",
        "in-progress",
        "blocked",
        "needs-review",
        "done",
        "abandoned",
    ];
    if STATUSES.contains(&raw) {
        Ok(raw.to_owned())
    } else {
        Err(GrafikiError::InvalidStateStatus(raw.to_owned()))
    }
}

fn validate_state_priority(raw: &str) -> Result<String> {
    const PRIORITIES: &[&str] = &["critical", "high", "medium", "low"];
    if PRIORITIES.contains(&raw) {
        Ok(raw.to_owned())
    } else {
        Err(GrafikiError::InvalidStatePriority(raw.to_owned()))
    }
}

fn validate_decision_status(raw: &str) -> Result<String> {
    const STATUSES: &[&str] = &["active", "superseded", "revisit", "revoked"];
    if STATUSES.contains(&raw) {
        Ok(raw.to_owned())
    } else {
        Err(GrafikiError::InvalidDecisionStatus(raw.to_owned()))
    }
}

fn validate_entity_type(raw: &str) -> Result<String> {
    const ENTITY_TYPES: &[&str] = &[
        "person", "service", "file", "module", "concept", "api", "tool", "library", "config",
        "endpoint",
    ];
    if ENTITY_TYPES.contains(&raw) {
        Ok(raw.to_owned())
    } else {
        Err(GrafikiError::InvalidEntityType(raw.to_owned()))
    }
}

fn validate_observation_category(raw: &str) -> Result<String> {
    const CATEGORIES: &[&str] = &[
        "general",
        "architecture",
        "decision",
        "blocker",
        "pattern",
        "progress",
        "gotcha",
        "learned",
        "preference",
        "convention",
        "dependency",
        "risk",
    ];
    if CATEGORIES.contains(&raw) {
        Ok(raw.to_owned())
    } else {
        Err(GrafikiError::InvalidObservationCategory(raw.to_owned()))
    }
}

fn validate_context_category(raw: &str) -> Result<String> {
    const CATEGORIES: &[&str] = &[
        "architecture",
        "audit",
        "spec",
        "reference",
        "onboarding",
        "runbook",
        "postmortem",
        "guide",
    ];
    if CATEGORIES.contains(&raw) {
        Ok(raw.to_owned())
    } else {
        Err(GrafikiError::InvalidContextCategory(raw.to_owned()))
    }
}

fn validate_relation_type(raw: &str) -> Result<String> {
    const RELATIONS: &[&str] = &[
        "owns",
        "depends_on",
        "blocks",
        "unblocks",
        "works_with",
        "part_of",
        "uses",
        "produces",
        "consumes",
        "calls",
        "extends",
        "replaces",
        "tests",
        "deploys_to",
        "related_to",
    ];
    if RELATIONS.contains(&raw) {
        Ok(raw.to_owned())
    } else {
        Err(GrafikiError::InvalidRelationType(raw.to_owned()))
    }
}

fn validate_relation_source_type(raw: &str) -> Result<String> {
    let normalized = raw.trim().to_ascii_uppercase();
    const SOURCE_TYPES: &[&str] = &["EXTRACTED", "INFERRED", "AMBIGUOUS"];
    if SOURCE_TYPES.contains(&normalized.as_str()) {
        Ok(normalized)
    } else {
        Err(GrafikiError::InvalidRelationSourceType(raw.to_owned()))
    }
}

fn validate_relation_confidence(value: f64) -> Result<f64> {
    if (0.0..=1.0).contains(&value) {
        Ok(value)
    } else {
        Err(GrafikiError::InvalidRelationConfidence(value))
    }
}

fn validate_candidate_record_type(raw: &str) -> Result<String> {
    const RECORD_TYPES: &[&str] = &["entity", "observation", "decision", "context", "state"];
    let normalized = raw.trim().to_ascii_lowercase();
    if RECORD_TYPES.contains(&normalized.as_str()) {
        Ok(normalized)
    } else {
        Err(GrafikiError::InvalidRecordType(raw.to_owned()))
    }
}

fn validate_candidate_status(raw: &str) -> Result<String> {
    const STATUSES: &[&str] = &["pending", "approved", "rejected"];
    let normalized = raw.trim().to_ascii_lowercase();
    if STATUSES.contains(&normalized.as_str()) {
        Ok(normalized)
    } else {
        Err(GrafikiError::InvalidCandidate(format!(
            "invalid candidate status: {raw}"
        )))
    }
}

fn validate_candidate_review_action(raw: &str) -> Result<String> {
    const ACTIONS: &[&str] = &["approve", "reject"];
    let normalized = raw.trim().to_ascii_lowercase();
    if ACTIONS.contains(&normalized.as_str()) {
        Ok(normalized)
    } else {
        Err(GrafikiError::InvalidCandidate(format!(
            "invalid candidate review action: {raw}"
        )))
    }
}

fn validate_candidate_source_type(raw: &str) -> Result<String> {
    let source_type = raw.trim();
    if source_type.is_empty() {
        Err(GrafikiError::InvalidCandidate(
            "candidate source_type is required".to_owned(),
        ))
    } else {
        Ok(source_type.to_owned())
    }
}

fn validate_capture_source_type(raw: &str) -> Result<String> {
    const SOURCE_TYPES: &[&str] = &[
        "transcript",
        "screen",
        "ide",
        "file",
        "terminal",
        "browser",
        "agent",
        "system",
        "git",
    ];
    let normalized = raw.trim().to_ascii_lowercase();
    if SOURCE_TYPES.contains(&normalized.as_str()) {
        Ok(normalized)
    } else {
        Err(GrafikiError::InvalidRecordType(raw.to_owned()))
    }
}

fn validate_privacy_level(raw: &str) -> Result<String> {
    const LEVELS: &[&str] = &["public", "internal", "sensitive", "secret"];
    let normalized = raw.trim().to_ascii_lowercase();
    if LEVELS.contains(&normalized.as_str()) {
        Ok(normalized)
    } else {
        Err(GrafikiError::InvalidCandidate(format!(
            "invalid privacy level: {raw}"
        )))
    }
}

fn validate_candidate_confidence(value: f64) -> Result<f64> {
    if (0.0..=1.0).contains(&value) {
        Ok(value)
    } else {
        Err(GrafikiError::InvalidCandidate(format!(
            "candidate confidence must be between 0 and 1: {value}"
        )))
    }
}

fn load_context_document(connection: &Connection, key: &str) -> Result<ContextDocument> {
    connection
        .query_row(
            "
            SELECT key, title, category, scope, version, content
            FROM context
            WHERE key = ?1 AND retired_at IS NULL
            ",
            [key],
            |row| {
                Ok(ContextDocument {
                    key: row.get(0)?,
                    title: row.get(1)?,
                    category: row.get(2)?,
                    scope: row.get(3)?,
                    version: row.get(4)?,
                    content: row.get(5)?,
                })
            },
        )
        .optional()?
        .ok_or_else(|| GrafikiError::ContextNotFound(key.to_owned()))
}

fn context_summary_from_row(row: &Row<'_>) -> rusqlite::Result<ContextSummary> {
    Ok(ContextSummary {
        key: row.get(0)?,
        title: row.get(1)?,
        category: row.get(2)?,
        scope: row.get(3)?,
        version: row.get(4)?,
    })
}

fn decision_item_from_row(row: &Row<'_>) -> rusqlite::Result<DecisionItem> {
    Ok(DecisionItem {
        id: row.get(0)?,
        title: row.get(1)?,
        status: row.get(2)?,
        scope: row.get(3)?,
        reasoning: row.get(4)?,
    })
}

fn graph_entity_from_row(row: &Row<'_>) -> rusqlite::Result<GraphEntity> {
    Ok(GraphEntity {
        id: row.get(0)?,
        name: row.get(1)?,
        entity_type: row.get(2)?,
        scope: row.get(3)?,
    })
}

fn observation_item_from_row(row: &Row<'_>) -> rusqlite::Result<ObservationItem> {
    Ok(ObservationItem {
        id: row.get(0)?,
        entity_id: row.get(1)?,
        entity_name: row.get(2)?,
        content: row.get(3)?,
        category: row.get(4)?,
        confidence: row.get(5)?,
        scope: row.get(6)?,
    })
}

fn state_item_from_row(row: &Row<'_>) -> rusqlite::Result<StateItem> {
    Ok(StateItem {
        key: row.get(0)?,
        title: row.get(1)?,
        status: row.get(2)?,
        priority: row.get(3)?,
        owner: row.get(4)?,
        scope: row.get(5)?,
        details: row.get(6)?,
        blockers: parse_json_list(row.get::<_, Option<String>>(7)?.as_deref()),
        depends_on: parse_json_list(row.get::<_, Option<String>>(8)?.as_deref()),
    })
}

fn event_item_from_row(row: &Row<'_>) -> rusqlite::Result<EventItem> {
    Ok(EventItem {
        id: row.get(0)?,
        event_type: row.get(1)?,
        source_session: row.get(2)?,
        target_type: row.get(3)?,
        target_id: row.get(4)?,
        scope: row.get(5)?,
        summary: row.get(6)?,
        created_at: row.get(7)?,
    })
}

fn extraction_candidate_from_row(row: &Row<'_>) -> rusqlite::Result<ExtractionCandidate> {
    let payload_text: String = row.get(4)?;
    let payload = serde_json::from_str(&payload_text).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(4, Type::Text, Box::new(error))
    })?;
    Ok(ExtractionCandidate {
        id: row.get(0)?,
        source_type: row.get(1)?,
        source: row.get(2)?,
        record_type: row.get(3)?,
        payload,
        scope: row.get(5)?,
        confidence: row.get(6)?,
        status: row.get(7)?,
        rationale: row.get(8)?,
        trusted_record_type: row.get(9)?,
        trusted_record_id: row.get(10)?,
        created_at: row.get(11)?,
        reviewed_at: row.get(12)?,
        evidence: Vec::new(),
        // Calibrated from the source prior alone here (evidence not yet loaded); list_candidates
        // refines both fields once corroboration is attached.
        calibrated_confidence: crate::confidence::calibrated_confidence(
            &row.get::<_, String>(1)?,
            0,
        ),
        review_priority: 0.0,
    })
}

fn evidence_link_from_row(row: &Row<'_>) -> rusqlite::Result<EvidenceLink> {
    Ok(EvidenceLink {
        id: row.get(0)?,
        candidate_id: row.get(1)?,
        trusted_record_type: row.get(2)?,
        trusted_record_id: row.get(3)?,
        source_event_id: row.get(4)?,
        source_type: row.get(5)?,
        source: row.get(6)?,
        title: row.get(7)?,
        excerpt: row.get(8)?,
        uri: row.get(9)?,
        byte_start: row.get(10)?,
        byte_end: row.get(11)?,
        line_start: row.get(12)?,
        line_end: row.get(13)?,
        captured_at: row.get(14)?,
        created_at: row.get(15)?,
    })
}

fn capture_session_from_row(row: &Row<'_>) -> rusqlite::Result<CaptureSession> {
    Ok(CaptureSession {
        id: row.get(0)?,
        project: row.get(1)?,
        scope: row.get(2)?,
        status: row.get(3)?,
        source_app: row.get(4)?,
        consent_profile: row.get(5)?,
        redaction_profile: row.get(6)?,
        started_at: row.get(7)?,
        ended_at: row.get(8)?,
    })
}

fn capture_event_from_row(row: &Row<'_>) -> rusqlite::Result<CaptureEvent> {
    let payload_text: Option<String> = row.get(6)?;
    let metadata_text: Option<String> = row.get(7)?;
    let payload = parse_optional_json_column(payload_text, 6)?;
    let metadata = parse_optional_json_column(metadata_text, 7)?;
    let redacted: i64 = row.get(9)?;
    Ok(CaptureEvent {
        id: row.get(0)?,
        capture_session: row.get(1)?,
        source_type: row.get(2)?,
        source: row.get(3)?,
        title: row.get(4)?,
        text: row.get(5)?,
        payload,
        metadata,
        privacy_level: row.get(8)?,
        redacted: redacted != 0,
        scope: row.get(10)?,
        captured_at: row.get(11)?,
        created_at: row.get(12)?,
    })
}

fn parse_optional_json_column(
    value: Option<String>,
    column_index: usize,
) -> rusqlite::Result<Option<serde_json::Value>> {
    value
        .map(|value| {
            serde_json::from_str(&value).map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(column_index, Type::Text, Box::new(error))
            })
        })
        .transpose()
}

fn load_extraction_candidate(connection: &Connection, id: &str) -> Result<ExtractionCandidate> {
    let mut candidate: ExtractionCandidate = connection
        .query_row(
            "
            SELECT id, source_type, source, proposed_record_type, payload, scope,
                   confidence, status, rationale, trusted_record_type, trusted_record_id,
                   created_at, reviewed_at
            FROM extraction_candidates
            WHERE id = ?1
            ",
            [id],
            extraction_candidate_from_row,
        )
        .optional()?
        .ok_or_else(|| GrafikiError::CandidateNotFound(id.to_owned()))?;
    attach_candidate_evidence(connection, &mut candidate)?;
    refine_candidate_confidence(&mut candidate);
    Ok(candidate)
}

/// M-E3: (re)compute the calibrated confidence + active-learning review priority from the
/// candidate's source tier and its currently-attached corroborating evidence. Must run AFTER
/// `attach_candidate_evidence` so the evidence count is real. Pure/deterministic; applied on every
/// surface that returns a candidate so the fields are consistent across list and mutation reports.
fn refine_candidate_confidence(candidate: &mut ExtractionCandidate) {
    let evidence_count = candidate.evidence.len() as u64;
    candidate.calibrated_confidence =
        crate::confidence::calibrated_confidence(&candidate.source_type, evidence_count);
    let uncertainty = crate::confidence::uncertainty(candidate.calibrated_confidence);
    candidate.review_priority = crate::confidence::review_priority(uncertainty, evidence_count);
}

fn attach_candidate_evidence(
    connection: &Connection,
    candidate: &mut ExtractionCandidate,
) -> Result<()> {
    candidate.evidence = list_evidence_for_candidate(connection, &candidate.id)?;
    Ok(())
}

fn list_evidence_for_candidate(
    connection: &Connection,
    candidate_id: &str,
) -> Result<Vec<EvidenceLink>> {
    let mut statement = connection.prepare(
        "
        SELECT id, candidate_id, trusted_record_type, trusted_record_id, source_event_id,
               source_type, source, title, excerpt, uri, byte_start, byte_end,
               line_start, line_end, captured_at, created_at
        FROM evidence_links
        WHERE candidate_id = ?1
        ORDER BY coalesce(captured_at, created_at) ASC,
                 line_start IS NULL ASC, line_start ASC,
                 byte_start IS NULL ASC, byte_start ASC,
                 coalesce(source, '') ASC, coalesce(uri, '') ASC,
                 excerpt ASC, id ASC
        ",
    )?;
    let rows = collect_rows(statement.query_map([candidate_id], evidence_link_from_row)?)?;
    Ok(rows)
}

fn list_evidence_for_record(
    connection: &Connection,
    record_type: &str,
    record_id: &str,
) -> Result<Vec<EvidenceLink>> {
    let mut statement = connection.prepare(
        "
        SELECT id, candidate_id, trusted_record_type, trusted_record_id, source_event_id,
               source_type, source, title, excerpt, uri, byte_start, byte_end,
               line_start, line_end, captured_at, created_at
        FROM evidence_links
        WHERE trusted_record_type = ?1 AND trusted_record_id = ?2
        ORDER BY coalesce(captured_at, created_at) ASC,
                 line_start IS NULL ASC, line_start ASC,
                 byte_start IS NULL ASC, byte_start ASC,
                 coalesce(source, '') ASC, coalesce(uri, '') ASC,
                 excerpt ASC, id ASC
        ",
    )?;
    let rows = collect_rows(
        statement.query_map(params![record_type, record_id], evidence_link_from_row)?,
    )?;
    Ok(rows)
}

fn attach_search_evidence(connection: &Connection, results: &mut [SearchResult]) -> Result<()> {
    for result in results {
        result.evidence = list_evidence_for_record(connection, &result.record_type, &result.id)?;
    }
    Ok(())
}

fn insert_candidate_evidence(
    connection: &Connection,
    candidate_id: &str,
    evidence: &[EvidenceInput],
) -> Result<()> {
    // Redact the free-text evidence fields too. The candidate payload is
    // redacted at the trust boundary, but evidence excerpt/title/source/uri are
    // raw provenance lifted straight from captured output — a secret in the
    // source line (reproducible via the HTTP propose API) would otherwise
    // persist in the clear alongside a redacted payload.
    let redact_opt = |value: &Option<String>| -> Option<String> {
        value.as_ref().map(|text| redact_text(text).0)
    };
    for item in evidence {
        let source_type = validate_evidence_source_type(&item.source_type)?;
        connection.execute(
            "
            INSERT INTO evidence_links
                (id, candidate_id, source_event_id, source_type, source, title, excerpt,
                 uri, byte_start, byte_end, line_start, line_end, captured_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
            ",
            params![
                new_ulid(),
                candidate_id,
                item.source_event_id,
                source_type,
                redact_opt(&item.source),
                redact_opt(&item.title),
                redact_text(&compact_excerpt(&item.excerpt, 800)).0,
                redact_opt(&item.uri),
                item.byte_start,
                item.byte_end,
                item.line_start,
                item.line_end,
                item.captured_at
            ],
        )?;
    }
    Ok(())
}

fn promote_candidate_evidence(
    connection: &Connection,
    candidate_id: &str,
    trusted_record_type: &str,
    trusted_record_id: &str,
) -> Result<()> {
    connection.execute(
        "
        UPDATE evidence_links
        SET trusted_record_type = ?1,
            trusted_record_id = ?2
        WHERE candidate_id = ?3
        ",
        params![trusted_record_type, trusted_record_id, candidate_id],
    )?;
    Ok(())
}

fn validate_evidence_source_type(raw: &str) -> Result<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(GrafikiError::InvalidCandidate(
            "evidence source_type is required".to_owned(),
        ));
    }
    Ok(trimmed.to_ascii_lowercase())
}

fn load_capture_session(connection: &Connection, id: &str) -> Result<CaptureSession> {
    connection
        .query_row(
            "
            SELECT id, project, scope, status, source_app, consent_profile,
                   redaction_profile, started_at, ended_at
            FROM capture_sessions
            WHERE id = ?1
            ",
            [id],
            capture_session_from_row,
        )
        .optional()?
        .ok_or_else(|| GrafikiError::InvalidCandidate(format!("capture session not found: {id}")))
}

fn load_capture_event(connection: &Connection, id: &str) -> Result<CaptureEvent> {
    connection
        .query_row(
            "
            SELECT id, capture_session, source_type, source, title, text, payload, metadata,
                   privacy_level, redacted, scope, captured_at, created_at
            FROM capture_events
            WHERE id = ?1
            ",
            [id],
            capture_event_from_row,
        )
        .optional()?
        .ok_or_else(|| GrafikiError::InvalidCandidate(format!("capture event not found: {id}")))
}

fn ensure_capture_session_exists(connection: &Connection, id: &str) -> Result<()> {
    let exists: Option<String> = connection
        .query_row(
            "SELECT id FROM capture_sessions WHERE id = ?1",
            [id],
            |row| row.get(0),
        )
        .optional()?;
    if exists.is_some() {
        Ok(())
    } else {
        Err(GrafikiError::InvalidCandidate(format!(
            "capture session not found: {id}"
        )))
    }
}

fn latest_active_capture_session(
    connection: &Connection,
    project: &str,
    scope: &str,
) -> Result<Option<String>> {
    connection
        .query_row(
            "
            SELECT id
            FROM capture_sessions
            WHERE project = ?1 AND scope = ?2 AND status = 'active'
            ORDER BY started_at DESC, id DESC
            LIMIT 1
            ",
            params![project, scope],
            |row| row.get(0),
        )
        .optional()
        .map_err(Into::into)
}

fn scoped_capture_sessions(
    connection: &Connection,
    project: &str,
    scope_chain: &[String],
    active_only: bool,
) -> Result<Vec<CaptureSession>> {
    let status_clause = if active_only {
        "AND status = 'active'"
    } else {
        ""
    };
    let sql = scoped_query(
        &format!(
            "
            SELECT id, project, scope, status, source_app, consent_profile,
                   redaction_profile, started_at, ended_at
            FROM capture_sessions
            WHERE project = ? AND scope IN ({{scopes}}) {status_clause}
            ORDER BY started_at DESC, id DESC
            LIMIT 50
            "
        ),
        scope_chain.len(),
    );
    let mut params: Vec<&dyn rusqlite::ToSql> = vec![&project];
    params.extend(
        scope_chain
            .iter()
            .map(|scope| scope as &dyn rusqlite::ToSql),
    );
    let mut statement = connection.prepare(&sql)?;
    let sessions = collect_rows(statement.query_map(params.as_slice(), capture_session_from_row)?)?;
    Ok(sessions)
}

/// H2: finalize a just-approved observation. Stamps the candidate's logical time
/// (`captured_at` → `valid_from`) and `source_type` onto the new observation, then
/// — if it carries a `supersedes` pointer — closes the prior fact's validity
/// window when metadata arbitration says the new fact wins.
///
/// Guards: the prior observation must belong to the **same entity and scope** as
/// the new one (a `supersedes` id can otherwise invalidate an unrelated fact);
/// the cut is `old.valid_to = new.valid_from` (the bitemporal abutting-windows
/// rule); and a strictly-higher-trust prior fact is never auto-superseded. A
/// missing/already-invalidated/cross-entity target is a silent no-op.
fn finalize_observation_candidate(
    project_name: Option<String>,
    start_dir: PathBuf,
    grafiki_home: Option<PathBuf>,
    new_id: &str,
    candidate: &ExtractionCandidate,
    supersedes: Option<String>,
) -> Result<()> {
    let (_project, mut connection) = resolve_and_open(project_name, start_dir, grafiki_home)?;

    // New observation's entity + scope, and the logical valid_from to stamp.
    let (new_entity, new_scope, new_default_valid_from): (String, String, String) = connection
        .query_row(
            "
            SELECT o.entity_id, e.scope, o.valid_from
            FROM observations o JOIN entities e ON e.id = o.entity_id
            WHERE o.id = ?1
            ",
            [new_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;
    let new_valid_from = candidate_payload_optional_string(&candidate.payload, &["captured_at"])
        .unwrap_or(new_default_valid_from);

    // Stamp logical time + source-type so windows abut and future arbitration
    // sees a real source tier (not the `session:` provenance link).
    connection.execute(
        "UPDATE observations SET valid_from = ?1, source_type = ?2 WHERE id = ?3",
        params![new_valid_from, candidate.source_type, new_id],
    )?;

    let Some(old_id) = supersedes else {
        return Ok(());
    };

    let old: Option<(String, String, Option<String>, String, f64)> = connection
        .query_row(
            "
            SELECT o.entity_id, e.scope, o.source_type, o.valid_from, o.confidence
            FROM observations o JOIN entities e ON e.id = o.entity_id
            WHERE o.id = ?1 AND o.valid_to IS NULL
            ",
            [&old_id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )
        .optional()?;
    let Some((old_entity, old_scope, old_source_type, old_valid_from, old_confidence)) = old else {
        return Ok(());
    };
    // Guard: never supersede across entities or scopes.
    if old_entity != new_entity || old_scope != new_scope {
        return Ok(());
    }

    let trusted = crate::conflict::FactMeta {
        valid_from: old_valid_from,
        source_type: old_source_type.unwrap_or_default(),
        confidence: old_confidence,
    };
    let incoming = crate::conflict::FactMeta {
        valid_from: new_valid_from.clone(),
        source_type: candidate.source_type.clone(),
        confidence: candidate.confidence,
    };
    let (winner, basis) = crate::conflict::arbitrate(&trusted, &incoming);
    if winner != crate::conflict::Winner::Incoming {
        return Ok(());
    }

    let tx = connection.transaction()?;
    tx.execute(
        "UPDATE observations SET valid_to = ?1 WHERE id = ?2 AND valid_to IS NULL",
        params![new_valid_from, old_id],
    )?;
    delete_embedding_records(&tx, "observation", &old_id)?;
    tx.execute(
        "
        INSERT INTO events (id, event_type, source_session, target_type, target_id, scope, summary)
        VALUES (?1, 'observation_invalidated', NULL, 'observation', ?2, ?3, ?4)
        ",
        params![
            new_ulid(),
            old_id,
            old_scope,
            format!("Superseded by observation {new_id} (basis={basis:?})")
        ],
    )?;
    tx.commit()?;
    Ok(())
}

fn approve_candidate_payload(
    candidate: &ExtractionCandidate,
    project_name: Option<String>,
    start_dir: PathBuf,
    grafiki_home: Option<PathBuf>,
) -> Result<(String, String)> {
    let payload = &candidate.payload;
    match candidate.record_type.as_str() {
        "entity" => {
            let name = candidate_payload_string(payload, &["name", "title"])?;
            let report = save_entity(SaveEntityOptions {
                project_name,
                start_dir,
                grafiki_home,
                name,
                entity_type: candidate_payload_optional_string(payload, &["entity_type"])
                    .unwrap_or_else(|| "concept".to_owned()),
                observe: candidate_payload_optional_string(payload, &["observe", "content"]),
                category: candidate_payload_optional_string(payload, &["category"])
                    .unwrap_or_else(|| "general".to_owned()),
                scope: candidate.scope.clone(),
                relate: candidate_payload_optional_string(payload, &["relate"]),
            })?;
            Ok(("entity".to_owned(), report.entity_id))
        }
        "observation" => {
            let name = candidate_payload_string(payload, &["entity_name", "name", "title"])?;
            let content = candidate_payload_string(payload, &["content", "observe"])?;
            let supersedes = candidate_payload_optional_string(payload, &["supersedes"]);
            let report = save_entity(SaveEntityOptions {
                project_name: project_name.clone(),
                start_dir: start_dir.clone(),
                grafiki_home: grafiki_home.clone(),
                name,
                entity_type: candidate_payload_optional_string(payload, &["entity_type"])
                    .unwrap_or_else(|| "concept".to_owned()),
                observe: Some(content),
                category: candidate_payload_optional_string(payload, &["category"])
                    .unwrap_or_else(|| "general".to_owned()),
                scope: candidate.scope.clone(),
                relate: candidate_payload_optional_string(payload, &["relate"]),
            })?;
            let observation_id = report.observation_id.ok_or_else(|| {
                GrafikiError::InvalidCandidate(
                    "approved observation did not create an observation".to_owned(),
                )
            })?;
            // H2: stamp the candidate's logical time + source-type onto the new
            // observation and apply any supersession (arbitrated on metadata).
            // Best-effort: a failure here must not fail the approval (the new fact
            // is already valid) nor leave a pending candidate whose retry would
            // duplicate the observation.
            let _ = finalize_observation_candidate(
                project_name,
                start_dir,
                grafiki_home,
                &observation_id,
                candidate,
                supersedes,
            );
            Ok(("observation".to_owned(), observation_id))
        }
        "decision" => {
            let report = log_decision(LogDecisionOptions {
                project_name,
                start_dir,
                grafiki_home,
                title: candidate_payload_string(payload, &["title"])?,
                reasoning: candidate_payload_optional_string(payload, &["reasoning", "content"]),
                alternatives: candidate_payload_vec(payload, "alternatives"),
                tags: candidate_payload_vec(payload, "tags"),
                scope: candidate.scope.clone(),
                supersedes: candidate_payload_optional_string(payload, &["supersedes"]),
            })?;
            Ok(("decision".to_owned(), report.decision_id))
        }
        "context" => {
            let report = add_context(AddContextOptions {
                project_name,
                start_dir,
                grafiki_home,
                key: candidate_payload_string(payload, &["key", "id"])?,
                title: candidate_payload_string(payload, &["title"])?,
                category: candidate_payload_optional_string(payload, &["category"])
                    .unwrap_or_else(|| "reference".to_owned()),
                scope: candidate.scope.clone(),
                content: candidate_payload_string(payload, &["content", "body"])?,
            })?;
            Ok(("context".to_owned(), report.key))
        }
        "state" => {
            let report = upsert_state(UpsertStateOptions {
                project_name,
                start_dir,
                grafiki_home,
                key: candidate_payload_string(payload, &["key", "id"])?,
                title: candidate_payload_string(payload, &["title"])?,
                status: candidate_payload_optional_string(payload, &["status"])
                    .unwrap_or_else(|| "needs-review".to_owned()),
                owner: candidate_payload_optional_string(payload, &["owner"]),
                details: candidate_payload_optional_string(payload, &["details", "content"]),
                blockers: candidate_payload_vec(payload, "blockers"),
                depends_on: candidate_payload_vec(payload, "depends_on"),
                scope: candidate.scope.clone(),
                priority: candidate_payload_optional_string(payload, &["priority"])
                    .unwrap_or_else(|| "medium".to_owned()),
            })?;
            Ok(("state".to_owned(), report.key))
        }
        other => Err(GrafikiError::InvalidRecordType(other.to_owned())),
    }
}

fn candidate_payload_string(payload: &serde_json::Value, keys: &[&str]) -> Result<String> {
    candidate_payload_optional_string(payload, keys)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            GrafikiError::InvalidCandidate(format!(
                "candidate payload missing required field: {}",
                keys.join("|")
            ))
        })
}

fn candidate_payload_optional_string(payload: &serde_json::Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        payload.get(key).and_then(|value| match value {
            serde_json::Value::String(value) => Some(value.clone()),
            serde_json::Value::Number(value) => Some(value.to_string()),
            serde_json::Value::Bool(value) => Some(value.to_string()),
            _ => None,
        })
    })
}

fn candidate_payload_vec(payload: &serde_json::Value, key: &str) -> Vec<String> {
    match payload.get(key) {
        Some(serde_json::Value::Array(items)) => items
            .iter()
            .filter_map(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .collect(),
        Some(serde_json::Value::String(value)) => value
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .collect(),
        _ => Vec::new(),
    }
}

fn summarize_capture_events(events: &[CaptureEvent]) -> String {
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for event in events {
        *counts.entry(event.source_type.as_str()).or_default() += 1;
    }
    let mut source_counts = counts
        .into_iter()
        .map(|(source_type, count)| format!("{source_type}: {count}"))
        .collect::<Vec<_>>();
    source_counts.sort();

    let mut lines = vec![
        "Automatic coding capture summary.".to_owned(),
        format!("Captured events summarized: {}", events.len()),
        format!("Source mix: {}", source_counts.join(", ")),
        String::new(),
        "Recent captured evidence:".to_owned(),
    ];

    for event in events.iter().take(30) {
        let title = event
            .title
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or("Untitled event");
        let body = event
            .text
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .map(compact_capture_text)
            .or_else(|| event.payload.as_ref().map(compact_capture_payload))
            .unwrap_or_else(|| "No event text.".to_owned());
        lines.push(format!(
            "- [{}] {} at {}: {}",
            event.source_type, title, event.captured_at, body
        ));
    }
    if events.len() > 30 {
        lines.push(format!("- ... {} more events", events.len() - 30));
    }
    lines.join("\n")
}

fn evidence_from_capture_event(event: &CaptureEvent) -> EvidenceInput {
    let excerpt = event
        .text
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(|value| compact_excerpt(value, 420))
        .or_else(|| event.payload.as_ref().map(compact_capture_payload))
        .unwrap_or_else(|| {
            event
                .title
                .clone()
                .unwrap_or_else(|| "Captured event".to_owned())
        });
    EvidenceInput {
        source_event_id: Some(event.id.clone()),
        source_type: event.source_type.clone(),
        source: event.source.clone(),
        title: event.title.clone(),
        excerpt,
        uri: Some(format!("grafiki://capture/{}", event.id)),
        byte_start: None,
        byte_end: None,
        line_start: None,
        line_end: None,
        captured_at: Some(event.captured_at.clone()),
    }
}

fn compact_capture_text(text: &str) -> String {
    compact_excerpt(text, 360)
}

fn compact_excerpt(text: &str, limit: usize) -> String {
    let compact = text.split_whitespace().collect::<Vec<_>>().join(" ");
    truncate_with_ellipsis(&compact, limit)
}

/// Truncate `text` to at most `max_chars` characters, never splitting a
/// multibyte character, appending an ellipsis when truncation occurred.
fn truncate_with_ellipsis(text: &str, max_chars: usize) -> String {
    if text.chars().count() > max_chars {
        let truncated: String = text.chars().take(max_chars).collect();
        format!("{truncated}...")
    } else {
        text.to_owned()
    }
}

/// Public eval/test seam over the redaction trust boundary.
///
/// Runs the same substitution redactor used on the ingest path
/// ([`redact_sensitive_text`]) and returns the redacted text together with a
/// flag indicating whether any redaction fired. This lets the `grafiki-eval`
/// harness (and tests) score the redactor directly — input→output diff —
/// without round-tripping a record through the capture/ingest pipeline.
///
/// The redactor *substitutes* secrets with `[REDACTED_…]` markers rather than
/// emitting spans, so callers measuring precision/recall should diff `input`
/// against the returned string.
pub fn redact_text(input: &str) -> (String, bool) {
    let mut text = input.to_string();
    let changed = redact_sensitive_text(&mut text);
    (text, changed)
}

/// Key names whose value is treated as a secret, both in assignment-style text
/// and in JSON candidate payloads. Hoisted to module scope so the text redactor
/// and the (key-aware) JSON redactor share one source of truth.
const SECRET_KEYS: &[&str] = &[
    "api_key",
    "apikey",
    "access_token",
    "auth_token",
    "client_secret",
    "secret",
    "password",
    "passwd",
    "passphrase",
    "private_key",
    "github_token",
    "openai_api_key",
    "anthropic_api_key",
    // M-E6: broadened, still key-gated (high precision). `secret` already covers `*_secret`.
    "token",
    "credential",
    "bearer",
    "access_key",
];

/// True when `key` names a secret (case-insensitive substring match).
fn key_looks_secret(key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    SECRET_KEYS.iter().any(|k| lower.contains(k))
}

/// Public eval/test seam over the structured (JSON) redaction path used for
/// candidate payloads — the `propose_candidate` sink, which redacts each JSON
/// value rather than a serialized blob. Returns the redacted document and
/// whether anything changed. Complements [`redact_text`] (the `ingest_capture_event`
/// path); the two cover both primary secret sinks.
pub fn redact_json(value: &serde_json::Value) -> (serde_json::Value, bool) {
    let mut redacted = value.clone();
    let changed = redact_json_value(&mut redacted);
    (redacted, changed)
}

fn redact_sensitive_text(text: &mut String) -> bool {
    let original = text.clone();
    let mut redacted = redact_assignment_like_secrets(&original);
    redacted = redact_private_key_blocks(&redacted);
    redacted = redact_token_prefixes(&redacted);
    redacted = redact_keyword_adjacent_entropy(&redacted);
    let changed = redacted != original;
    if changed {
        *text = redacted;
    }
    changed
}

/// Catch bare high-entropy secrets with no known prefix and no `key=value`
/// shape — "the webhook signing key is a3f9c2e8…" leaked verbatim while the
/// JWT beside it was caught. Scoped to lines that NAME a secret-ish word, and
/// to tokens that look like key material (≥24 chars of alnum/base64 charset
/// with both letters and digits), so prose and identifiers survive.
fn redact_keyword_adjacent_entropy(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for line in input.split_inclusive('\n') {
        if line_names_secret(line) {
            out.push_str(&redact_entropy_runs(line));
        } else {
            out.push_str(line);
        }
    }
    out
}

fn line_names_secret(line: &str) -> bool {
    const KEYWORDS: &[&str] = &[
        "key",
        "keys",
        "secret",
        "secrets",
        "token",
        "tokens",
        "password",
        "passwords",
        "passwd",
        "credential",
        "credentials",
        "apikey",
        "bearer",
        "passphrase",
    ];
    let lower = line.to_ascii_lowercase();
    lower
        .split(|c: char| !c.is_ascii_alphanumeric())
        .any(|word| KEYWORDS.contains(&word))
}

fn redact_entropy_runs(line: &str) -> String {
    fn is_run_char(c: char) -> bool {
        c.is_ascii_alphanumeric() || matches!(c, '+' | '/' | '=' | '_' | '-')
    }
    fn looks_like_key_material(run: &str) -> bool {
        run.chars().count() >= 24
            && run.chars().any(|c| c.is_ascii_digit())
            && run.chars().any(|c| c.is_ascii_alphabetic())
    }
    fn flush(run: &mut String, out: &mut String) {
        if looks_like_key_material(run) {
            out.push_str("[REDACTED_SECRET]");
        } else {
            out.push_str(run);
        }
        run.clear();
    }
    let mut out = String::with_capacity(line.len());
    let mut run = String::new();
    for c in line.chars() {
        if is_run_char(c) {
            run.push(c);
        } else {
            flush(&mut run, &mut out);
            out.push(c);
        }
    }
    flush(&mut run, &mut out);
    out
}

/// The redaction policy for a capture session (C13). Stored per `capture_session`
/// (`redaction_profile` column) and honored by [`ingest_capture_event`] — until
/// now it was advertised but inert.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RedactionProfile {
    /// No redaction of the raw capture log. The always-on candidate gate still
    /// redacts at promotion (`redact_json_value`), so trusted memory is unaffected.
    None,
    /// Standard secret redaction (assignments, private-key blocks, token prefixes).
    Default,
    /// `Default` plus email-address (PII) redaction — an opt-in, more aggressive
    /// pass that accepts occasional over-redaction.
    Strict,
}

impl RedactionProfile {
    /// Parse a stored profile string. Unknown/empty falls back to `Default` — a
    /// typo (or a legacy value) must never silently DISABLE redaction.
    fn parse(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "none" => RedactionProfile::None,
            "strict" => RedactionProfile::Strict,
            _ => RedactionProfile::Default,
        }
    }
}

/// Apply a capture profile's redaction policy to `text`, returning whether it
/// changed. `Default` matches the legacy behavior byte-for-byte, so existing
/// captures and the redaction eval are unaffected.
fn redact_text_with_profile(text: &mut String, profile: RedactionProfile) -> bool {
    match profile {
        RedactionProfile::None => false,
        RedactionProfile::Default => redact_sensitive_text(text),
        RedactionProfile::Strict => {
            let mut changed = redact_sensitive_text(text);
            let with_emails = redact_emails(text);
            if &with_emails != text {
                *text = with_emails;
                changed = true;
            }
            changed
        }
    }
}

/// Redact email addresses (the `strict` profile's PII pass). Deterministic and
/// regex-free, mirroring the other redactors: for each `@`, expand to the maximal
/// local/domain character runs and, if they form a well-formed `local@domain.tld`
/// (non-empty local; dotted domain; ≥2-char alphabetic TLD), replace the span
/// with `[REDACTED_EMAIL]`. The local part is matched with **Unicode-aware** chars
/// (`char::is_alphanumeric`), so RFC 6531 addresses like `café@x.com` / `用户@x.com`
/// are covered, not partially leaked. Conservative — `@handle`, `user@`,
/// `a@localhost`, and IP-literal domains are left alone — but, being the aggressive
/// opt-in profile, it over-redacts lookalikes such as `image@2x.png`. Known limit:
/// the domain side is ASCII-only, so a Unicode (non-punycode) domain under-redacts.
fn redact_emails(input: &str) -> String {
    let bytes = input.as_bytes();
    // Local-part chars are Unicode-aware so a non-ASCII local part is captured
    // whole (a partial match would leak a fragment); domain chars stay ASCII.
    let is_local = |c: char| c.is_alphanumeric() || matches!(c, '.' | '_' | '%' | '+' | '-');
    let is_domain = |b: u8| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-');

    let mut spans: Vec<(usize, usize)> = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'@' {
            let mut end = i + 1;
            while end < bytes.len() && is_domain(bytes[end]) {
                end += 1;
            }
            if is_valid_email_domain(&bytes[i + 1..end]) {
                // Local part: scan chars left of `@` from the right (char_indices is
                // boundary-safe, so the resulting byte offset is always a char
                // boundary even across multi-byte locals).
                let mut start = i;
                for (idx, ch) in input[..i].char_indices().rev() {
                    if is_local(ch) {
                        start = idx;
                    } else {
                        break;
                    }
                }
                if start < i {
                    spans.push((start, end));
                    i = end;
                    continue;
                }
            }
        }
        i += 1;
    }
    if spans.is_empty() {
        return input.to_owned();
    }
    let mut out = String::with_capacity(input.len());
    let mut cursor = 0;
    for (start, end) in spans {
        out.push_str(&input[cursor..start]);
        out.push_str("[REDACTED_EMAIL]");
        cursor = end;
    }
    out.push_str(&input[cursor..]);
    out
}

/// Whether a domain byte-run is a plausible email domain: contains a dot, doesn't
/// start/end with `.`/`-`, and ends in a ≥2-char all-alphabetic TLD.
fn is_valid_email_domain(domain: &[u8]) -> bool {
    if domain.len() < 4 || domain[0] == b'.' || domain[0] == b'-' {
        return false;
    }
    let last = *domain.last().unwrap();
    if last == b'.' || last == b'-' {
        return false;
    }
    let Ok(domain) = std::str::from_utf8(domain) else {
        return false;
    };
    if !domain.contains('.') {
        return false;
    }
    let tld = domain.rsplit('.').next().unwrap_or("");
    tld.len() >= 2 && tld.bytes().all(|b| b.is_ascii_alphabetic())
}

/// Recursively redact secrets from every string value in a JSON document,
/// preserving structure. Used for candidate payloads before persistence.
fn redact_json_value(value: &mut serde_json::Value) -> bool {
    redact_json_value_with_profile(value, RedactionProfile::Default)
}

/// Profile-aware structured redaction used by raw capture payloads/metadata.
/// Secret-named keys redact scalar values even when the value alone has no
/// recognizable token shape; nested arrays/objects retain their JSON shape.
fn redact_json_value_with_profile(
    value: &mut serde_json::Value,
    profile: RedactionProfile,
) -> bool {
    if profile == RedactionProfile::None {
        return false;
    }

    match value {
        serde_json::Value::String(text) => {
            let mut owned = std::mem::take(text);
            let mut changed = redact_sensitive_text(&mut owned);
            if profile == RedactionProfile::Strict {
                let strict = redact_emails(&owned);
                if strict != owned {
                    owned = strict;
                    changed = true;
                }
            }
            *text = owned;
            changed
        }
        serde_json::Value::Array(items) => {
            let mut changed = false;
            for item in items.iter_mut() {
                changed |= redact_json_value_with_profile(item, profile);
            }
            changed
        }
        serde_json::Value::Object(map) => {
            let mut changed = false;
            for (key, item) in map.iter_mut() {
                // Key-aware: a string value under a secret-named key (e.g.
                // "client_secret", "password") is redacted whole, even when the
                // value alone carries no `=`/`:`/token-prefix the text passes
                // would catch. This closes a leak on the candidate-payload path.
                if key_looks_secret(key) {
                    changed |= redact_secret_json_value(item);
                } else {
                    changed |= redact_json_value_with_profile(item, profile);
                }
            }
            changed
        }
        _ => false,
    }
}

/// Redact every scalar below a secret-named key while preserving container
/// shape. This covers structured credentials such as arrays of tokens or nested
/// `{ "credentials": { "value": ... } }` objects.
fn redact_secret_json_value(value: &mut serde_json::Value) -> bool {
    match value {
        serde_json::Value::Object(map) => map.values_mut().fold(false, |changed, item| {
            redact_secret_json_value(item) | changed
        }),
        serde_json::Value::Array(items) => items.iter_mut().fold(false, |changed, item| {
            redact_secret_json_value(item) | changed
        }),
        serde_json::Value::Null => false,
        _ => {
            let replacement = serde_json::Value::String("[REDACTED_SECRET]".to_owned());
            if *value == replacement {
                false
            } else {
                *value = replacement;
                true
            }
        }
    }
}

fn redact_private_key_blocks(input: &str) -> String {
    let mut output = String::new();
    let mut in_private_key = false;
    for line in input.lines() {
        if line.contains("-----BEGIN ") && line.contains(" PRIVATE KEY-----") {
            in_private_key = true;
            output.push_str("[REDACTED_PRIVATE_KEY]");
            output.push('\n');
            continue;
        }
        if in_private_key {
            if line.contains("-----END ") && line.contains(" PRIVATE KEY-----") {
                in_private_key = false;
            }
            continue;
        }
        output.push_str(line);
        output.push('\n');
    }
    if input.ends_with('\n') {
        output
    } else {
        output.trim_end_matches('\n').to_owned()
    }
}

fn redact_assignment_like_secrets(input: &str) -> String {
    let mut output = Vec::new();
    for line in input.lines() {
        // For each `=`/`:` separator, the KEY is the identifier token IMMEDIATELY before it. Redact
        // from the earliest separator whose key is secret-named to end of line. Keying on the
        // adjacent identifier (not "anywhere in the line", and not "only before the first
        // separator") is both precise — prose like "entities: …, Refresh Token Cache" or
        // "Keywords: …, token" has no secret-named key adjacent to a separator — and leak-safe for
        // multi-field lines like "host: db, password=… ; token=…", where the secret key follows a
        // benign one.
        let mut cut: Option<(char, usize)> = None;
        for (i, ch) in line.char_indices() {
            if ch != '=' && ch != ':' {
                continue;
            }
            let before = line[..i].trim_end();
            let key_start = before
                .rfind(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '-'))
                .map(|p| p + 1)
                .unwrap_or(0);
            let key = &before[key_start..];
            if !key.is_empty() && key_looks_secret(key) {
                cut = Some((ch, i));
                break;
            }
        }
        let redacted = cut.map(|(ch, i)| {
            let prefix = line[..i].trim_end();
            if ch == '=' {
                format!("{prefix}=[REDACTED_SECRET]")
            } else {
                format!("{prefix}: [REDACTED_SECRET]")
            }
        });
        output.push(redacted.unwrap_or_else(|| line.to_owned()));
    }
    let joined = output.join("\n");
    if input.ends_with('\n') {
        format!("{joined}\n")
    } else {
        joined
    }
}

/// Redact known secret token formats while preserving the original whitespace
/// structure byte-for-byte. Only actual secret substitutions change the text,
/// so secret-free input round-trips unchanged (and is not flagged sensitive).
fn redact_token_prefixes(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut token = String::new();
    for ch in input.chars() {
        if ch.is_whitespace() {
            if !token.is_empty() {
                output.push_str(&redact_single_token(&token));
                token.clear();
            }
            output.push(ch);
        } else {
            token.push(ch);
        }
    }
    if !token.is_empty() {
        output.push_str(&redact_single_token(&token));
    }
    output
}

fn redact_single_token(token: &str) -> String {
    let trimmed =
        token.trim_matches(|ch: char| !ch.is_ascii_alphanumeric() && ch != '-' && ch != '_');
    if trimmed.is_empty() {
        return token.to_owned();
    }
    let replacement = if trimmed.starts_with("sk-ant-") {
        Some("[REDACTED_ANTHROPIC_KEY]")
    } else if trimmed.starts_with("sk-") && trimmed.len() >= 20 {
        Some("[REDACTED_OPENAI_KEY]")
    } else if trimmed.starts_with("sk_live_")
        || trimmed.starts_with("sk_test_")
        || trimmed.starts_with("pk_live_")
        || trimmed.starts_with("rk_live_")
    {
        Some("[REDACTED_STRIPE_KEY]")
    } else if trimmed.starts_with("ghp_")
        || trimmed.starts_with("gho_")
        || trimmed.starts_with("ghu_")
        || trimmed.starts_with("ghs_")
        || trimmed.starts_with("ghr_")
        || trimmed.starts_with("github_pat_")
    {
        Some("[REDACTED_GITHUB_TOKEN]")
    } else if trimmed.starts_with("glpat-") {
        Some("[REDACTED_GITLAB_TOKEN]")
    } else if trimmed.starts_with("xoxb-")
        || trimmed.starts_with("xoxp-")
        || trimmed.starts_with("xoxa-")
        || trimmed.starts_with("xapp-")
    {
        Some("[REDACTED_SLACK_TOKEN]")
    } else if trimmed.starts_with("AKIA") && trimmed.len() >= 16 {
        Some("[REDACTED_AWS_KEY]")
    } else if (trimmed.starts_with("AIza") && trimmed.len() >= 20) || trimmed.starts_with("ya29.") {
        Some("[REDACTED_GOOGLE_KEY]")
    // M-E6: additional gitleaks-style provider prefixes (high precision).
    } else if trimmed.starts_with("SG.") && trimmed.len() >= 20 {
        Some("[REDACTED_SENDGRID_KEY]")
    } else if trimmed.starts_with("npm_") && trimmed.len() >= 20 {
        Some("[REDACTED_NPM_TOKEN]")
    } else if trimmed.starts_with("dop_v1_")
        || trimmed.starts_with("doo_v1_")
        || trimmed.starts_with("dor_v1_")
    {
        Some("[REDACTED_DIGITALOCEAN_TOKEN]")
    } else if trimmed.starts_with("hf_") && trimmed.len() >= 20 {
        Some("[REDACTED_HUGGINGFACE_TOKEN]")
    } else if trimmed.starts_with("shpat_")
        || trimmed.starts_with("shpss_")
        || trimmed.starts_with("shpca_")
        || trimmed.starts_with("shppa_")
    {
        Some("[REDACTED_SHOPIFY_TOKEN]")
    } else if trimmed.starts_with("PMAK-") {
        Some("[REDACTED_POSTMAN_KEY]")
    } else if trimmed.starts_with("dckr_pat_") {
        Some("[REDACTED_DOCKER_TOKEN]")
    } else if trimmed.starts_with("lin_api_") {
        Some("[REDACTED_LINEAR_KEY]")
    } else if trimmed.starts_with("whsec_") && trimmed.len() >= 20 {
        Some("[REDACTED_STRIPE_WEBHOOK_SECRET]")
    } else if looks_like_jwt(trimmed) {
        Some("[REDACTED_JWT]")
    } else if looks_high_entropy_secret(trimmed) {
        // No known prefix, but a long, mixed-charset, high-entropy token — almost certainly a
        // credential. Conservatively gated (see `looks_high_entropy_secret`) to protect precision.
        Some("[REDACTED_HIGH_ENTROPY_SECRET]")
    } else {
        None
    };
    match replacement {
        Some(label) => token.replace(trimmed, label),
        None => token.to_owned(),
    }
}

fn looks_like_jwt(token: &str) -> bool {
    let mut parts = token.split('.');
    let Some(first) = parts.next() else {
        return false;
    };
    let Some(second) = parts.next() else {
        return false;
    };
    let Some(third) = parts.next() else {
        return false;
    };
    parts.next().is_none()
        && first.len() > 8
        && second.len() > 8
        && third.len() > 8
        && [first, second, third].iter().all(|part| {
            part.chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
        })
}

/// Shannon entropy (bits per character) of a token's character distribution. Random
/// credential strings score high (~4–6 bits); natural-language words and repetitive
/// strings score low. Pure.
fn shannon_entropy(token: &str) -> f64 {
    let chars: Vec<char> = token.chars().collect();
    let n = chars.len();
    if n == 0 {
        return 0.0;
    }
    let mut counts: std::collections::BTreeMap<char, usize> = std::collections::BTreeMap::new();
    for &ch in &chars {
        *counts.entry(ch).or_insert(0) += 1;
    }
    let n = n as f64;
    -counts
        .values()
        .map(|&c| {
            let p = c as f64 / n;
            p * p.log2()
        })
        .sum::<f64>()
}

/// M-E6: a conservatively-gated heuristic for a credential that carries no recognizable
/// provider prefix. To protect precision (over-redaction corrupts memory), ALL must hold:
/// the token is `≥ 28` chars of `[A-Za-z0-9_-]`, mixes lower + upper + digit (this excludes
/// lowercase-hex git SHAs/md5/uuids, ALL-CAPS constants, and natural words), and has Shannon
/// entropy `≥ 3.5` bits/char (random-looking). Tuned against the redaction eval's benign cases.
fn looks_high_entropy_secret(token: &str) -> bool {
    // 32 chars is the floor for most real API keys/tokens; shorter mixed strings (e.g. a 28-char
    // base64 data fragment) are more likely benign, so requiring ≥32 trims false positives. A
    // genuinely random ≥32-char mixed-case+digit blob is still inherently ambiguous with a
    // credential — when in doubt the seam errs toward redaction (a leak is worse than a corrupted
    // blob), accepting rare base64-data over-redaction.
    const MIN_LEN: usize = 32;
    const MIN_ENTROPY: f64 = 3.5;
    if token.chars().count() < MIN_LEN {
        return false;
    }
    let mut has_lower = false;
    let mut has_upper = false;
    let mut has_digit = false;
    for ch in token.chars() {
        match ch {
            'a'..='z' => has_lower = true,
            'A'..='Z' => has_upper = true,
            '0'..='9' => has_digit = true,
            '-' | '_' => {}
            _ => return false, // only key-shaped charsets qualify
        }
    }
    has_lower && has_upper && has_digit && shannon_entropy(token) >= MIN_ENTROPY
}

fn compact_capture_payload(payload: &serde_json::Value) -> String {
    compact_capture_text(&payload.to_string())
}

fn session_log_item_from_row(row: &Row<'_>) -> rusqlite::Result<SessionLogItem> {
    let accomplishments: Option<String> = row.get(6)?;
    let remaining: Option<String> = row.get(7)?;
    let files_changed: Option<String> = row.get(8)?;
    let decisions_made: Option<String> = row.get(9)?;
    let entities_touched: Option<String> = row.get(10)?;

    Ok(SessionLogItem {
        id: row.get(0)?,
        session_type: row.get(1)?,
        status: row.get(2)?,
        scope: row.get(3)?,
        goal: row.get(4)?,
        summary: row.get(5)?,
        accomplishments: parse_json_list(accomplishments.as_deref()),
        remaining: parse_json_list(remaining.as_deref()),
        files_changed: parse_json_list(files_changed.as_deref()),
        decisions_made: parse_json_list(decisions_made.as_deref()),
        entities_touched: parse_json_list(entities_touched.as_deref()),
        handoff_context: row.get(11)?,
        parent_session: row.get(12)?,
        child_session: row.get(13)?,
        started_at: row.get(14)?,
        ended_at: row.get(15)?,
    })
}

fn load_session_log_item(
    connection: &Connection,
    session_id: &str,
    project: &str,
) -> Result<SessionLogItem> {
    connection
        .query_row(
            "
            SELECT id, session_type, status, scope, goal, summary, accomplishments, remaining,
                   files_changed, decisions_made, entities_touched, handoff_context,
                   parent_session, child_session, started_at, ended_at
            FROM sessions
            WHERE id = ?1 AND project = ?2
            ",
            params![session_id, project],
            session_log_item_from_row,
        )
        .optional()?
        .ok_or_else(|| GrafikiError::SessionNotFound(session_id.to_owned()))
}

fn enqueue_embedding_job(
    tx: &rusqlite::Transaction<'_>,
    record_type: &str,
    record_id: &str,
    scope: &str,
    content: &str,
) -> Result<()> {
    let content = content.trim();
    if content.is_empty() {
        return Ok(());
    }

    tx.execute(
        "
        INSERT OR IGNORE INTO embedding_jobs (id, record_type, record_id, scope, content_hash)
        VALUES (?1, ?2, ?3, ?4, ?5)
        ",
        params![new_ulid(), record_type, record_id, scope, checksum(content)],
    )?;

    Ok(())
}

fn delete_embedding_records(
    tx: &rusqlite::Transaction<'_>,
    record_type: &str,
    record_id: &str,
) -> Result<()> {
    tx.execute(
        "DELETE FROM embedding_jobs WHERE record_type = ?1 AND record_id = ?2",
        params![record_type, record_id],
    )?;
    tx.execute(
        "DELETE FROM embedding_vectors WHERE record_type = ?1 AND record_id = ?2",
        params![record_type, record_id],
    )?;
    tx.execute(
        "DELETE FROM embedding_metadata WHERE record_type = ?1 AND record_id = ?2",
        params![record_type, record_id],
    )?;
    delete_sqlite_vec_records(tx, record_type, record_id)?;
    Ok(())
}

#[cfg(feature = "sqlite-vec")]
fn sqlite_vec_tables(connection: &Connection) -> Result<Vec<String>> {
    let mut statement = connection.prepare(
        "SELECT name FROM sqlite_schema
         WHERE type = 'table'
           AND name LIKE 'embedding_vec_%'
           AND sql LIKE 'CREATE VIRTUAL TABLE%'",
    )?;
    let rows = statement.query_map([], |row| row.get(0))?;
    collect_rows(rows)
}

#[cfg(feature = "sqlite-vec")]
fn delete_sqlite_vec_records(
    connection: &Connection,
    record_type: &str,
    record_id: &str,
) -> Result<()> {
    for table in sqlite_vec_tables(connection)? {
        // Names come only from sqlite_schema and are generated internally as
        // `embedding_vec_<hex>`, never from caller input.
        connection.execute(
            &format!("DELETE FROM {table} WHERE record_type = ?1 AND record_id = ?2"),
            params![record_type, record_id],
        )?;
    }
    Ok(())
}

#[cfg(not(feature = "sqlite-vec"))]
fn delete_sqlite_vec_records(
    _connection: &Connection,
    _record_type: &str,
    _record_id: &str,
) -> Result<()> {
    Ok(())
}

/// Remove index rows whose trusted source no longer participates in active
/// retrieval. Rebuild calls this before enqueueing live records, covering stale
/// rows left by legacy builds as well as current explicit retire/delete paths.
fn sweep_orphan_embedding_records(
    connection: &mut Connection,
    scope_chain: &[String],
) -> Result<usize> {
    let mut candidates: HashSet<(String, String)> = HashSet::new();
    for table in ["embedding_jobs", "embedding_vectors", "embedding_metadata"] {
        let sql = if table == "embedding_metadata" {
            format!("SELECT DISTINCT record_type, record_id FROM {table}")
        } else {
            let scoped = scoped_query(
                &format!(
                    "SELECT DISTINCT record_type, record_id FROM {table} WHERE scope IN ({{scopes}})"
                ),
                scope_chain.len(),
            );
            let mut statement = connection.prepare(&scoped)?;
            let rows = statement.query_map(params_from_iter(scope_chain.iter()), |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?;
            candidates.extend(collect_rows(rows)?);
            continue;
        };
        let mut statement = connection.prepare(&sql)?;
        let rows = statement.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        candidates.extend(collect_rows(rows)?);
    }

    let mut stale = Vec::new();
    for (record_type, record_id) in candidates {
        if load_embeddable_record(connection, &record_type, &record_id)?.is_none() {
            stale.push((record_type, record_id));
        }
    }
    let tx = connection.transaction()?;
    for (record_type, record_id) in &stale {
        delete_embedding_records(&tx, record_type, record_id)?;
    }
    tx.commit()?;
    sweep_sqlite_vec_orphans(connection, scope_chain)?;
    Ok(stale.len())
}

#[cfg(feature = "sqlite-vec")]
fn sweep_sqlite_vec_orphans(connection: &Connection, scope_chain: &[String]) -> Result<()> {
    for table in sqlite_vec_tables(connection)? {
        let rows: Vec<(String, String, String)> = {
            let mut statement = connection.prepare(&format!(
                "SELECT record_type, record_id, scope FROM {table}"
            ))?;
            let rows = collect_rows(
                statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?,
            )?;
            rows
        };
        for (record_type, record_id, scope) in rows {
            if scope_chain.iter().any(|allowed| allowed == &scope)
                && load_embeddable_record(connection, &record_type, &record_id)?.is_none()
            {
                connection.execute(
                    &format!("DELETE FROM {table} WHERE record_type = ?1 AND record_id = ?2"),
                    params![record_type, record_id],
                )?;
            }
        }
    }
    Ok(())
}

#[cfg(not(feature = "sqlite-vec"))]
fn sweep_sqlite_vec_orphans(_connection: &Connection, _scope_chain: &[String]) -> Result<()> {
    Ok(())
}

#[derive(Debug, Clone)]
struct PendingEmbeddingJob {
    id: String,
    record_type: String,
    record_id: String,
    content_hash: String,
}

enum EmbeddingJobOutcome {
    Processed,
    Skipped,
}

fn enqueue_embeddable_records(
    connection: &mut Connection,
    scope_chain: &[String],
) -> Result<usize> {
    let records = load_embeddable_records(connection, scope_chain)?;
    let tx = connection.transaction()?;
    let mut enqueued = 0;
    for record in records {
        let before = tx.changes();
        enqueue_embedding_job(
            &tx,
            &record.record_type,
            &record.record_id,
            &record.scope,
            &record.content,
        )?;
        if tx.changes() > before {
            enqueued += 1;
        }
    }
    tx.commit()?;
    Ok(enqueued)
}

/// Revive failed embedding jobs in scope back to pending so a `rebuild` retries
/// them instead of leaving them as permanent dead-letters.
fn requeue_failed_embedding_jobs(connection: &Connection, scope_chain: &[String]) -> Result<usize> {
    if scope_chain.is_empty() {
        return Ok(0);
    }
    let sql = scoped_query(
        "
        UPDATE embedding_jobs
        SET status = 'pending', error = NULL
        WHERE status = 'failed' AND scope IN ({scopes})
        ",
        scope_chain.len(),
    );
    let params: Vec<&dyn rusqlite::ToSql> = scope_chain
        .iter()
        .map(|scope| scope as &dyn rusqlite::ToSql)
        .collect();
    let count = connection.execute(&sql, params.as_slice())?;
    Ok(count)
}

fn pending_embedding_jobs(
    connection: &Connection,
    scope_chain: &[String],
    limit: usize,
) -> Result<Vec<PendingEmbeddingJob>> {
    let sql = scoped_query(
        "
        SELECT id, record_type, record_id, content_hash
        FROM embedding_jobs
        WHERE status = 'pending' AND scope IN ({scopes})
        ORDER BY created_at ASC, id ASC
        LIMIT ?
        ",
        scope_chain.len(),
    );
    let mut params: Vec<&dyn rusqlite::ToSql> = scope_chain
        .iter()
        .map(|scope| scope as &dyn rusqlite::ToSql)
        .collect();
    let limit = limit as i64;
    params.push(&limit);
    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map(params.as_slice(), |row| {
        Ok(PendingEmbeddingJob {
            id: row.get(0)?,
            record_type: row.get(1)?,
            record_id: row.get(2)?,
            content_hash: row.get(3)?,
        })
    })?;
    collect_rows(rows)
}

fn pending_embedding_scopes(connection: &Connection) -> Result<Vec<String>> {
    let mut statement = connection.prepare(
        "
        SELECT DISTINCT scope
        FROM embedding_jobs
        WHERE status = 'pending'
        ORDER BY scope ASC
        ",
    )?;
    let rows = statement.query_map([], |row| row.get(0))?;
    collect_rows(rows)
}

fn process_embedding_job(
    connection: &mut Connection,
    provider: &impl EmbeddingProvider,
    job: &PendingEmbeddingJob,
) -> Result<EmbeddingJobOutcome> {
    let Some(record) = load_embeddable_record(connection, &job.record_type, &job.record_id)? else {
        mark_embedding_job_skipped(connection, &job.id, "record no longer exists")?;
        return Ok(EmbeddingJobOutcome::Skipped);
    };
    let content_hash = checksum(record.content.trim());
    if content_hash != job.content_hash {
        mark_embedding_job_skipped(connection, &job.id, "record content changed")?;
        return Ok(EmbeddingJobOutcome::Skipped);
    }

    let embedding = provider.embed(&record.content)?;
    if embedding.len() != provider.dimension() {
        return Err(GrafikiError::Embedding(format!(
            "embedding dimension mismatch: expected {}, got {}",
            provider.dimension(),
            embedding.len()
        )));
    }

    let tx = connection.transaction()?;
    tx.execute(
        "
        INSERT INTO embedding_vectors (
            record_type, record_id, scope, provider, model, dimension, content_hash, embedding
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
        ON CONFLICT(record_type, record_id, provider, model) DO UPDATE SET
            scope = excluded.scope,
            dimension = excluded.dimension,
            content_hash = excluded.content_hash,
            embedding = excluded.embedding,
            updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
        ",
        params![
            record.record_type,
            record.record_id,
            record.scope,
            provider.provider_name(),
            provider.model_name(),
            provider.dimension() as i64,
            content_hash,
            serde_json::to_string(&embedding)?
        ],
    )?;
    tx.execute(
        "
        INSERT INTO embedding_metadata (
            record_type, record_id, provider, model, dimension, content_hash
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6)
        ON CONFLICT(record_type, record_id, provider, model) DO UPDATE SET
            dimension = excluded.dimension,
            content_hash = excluded.content_hash,
            embedded_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
        ",
        params![
            record.record_type,
            record.record_id,
            provider.provider_name(),
            provider.model_name(),
            provider.dimension() as i64,
            content_hash
        ],
    )?;
    tx.commit()?;

    upsert_vector_index(connection, provider, &record, &embedding)?;

    connection.execute(
        "
        UPDATE embedding_jobs
        SET status = 'embedded',
            attempts = attempts + 1,
            error = NULL,
            updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
        WHERE id = ?1
        ",
        [&job.id],
    )?;

    Ok(EmbeddingJobOutcome::Processed)
}

#[cfg(feature = "sqlite-vec")]
fn upsert_vector_index(
    connection: &Connection,
    provider: &impl EmbeddingProvider,
    record: &EmbeddableRecord,
    embedding: &[f32],
) -> Result<()> {
    let table_name = sqlite_vec_table_name(provider);
    let mut backend = SqliteVecBackend::new(connection, &table_name, provider.dimension())?;
    backend.upsert(VectorRecord {
        record_type: record.record_type.clone(),
        record_id: record.record_id.clone(),
        scope: record.scope.clone(),
        embedding: embedding.to_vec(),
    })
}

#[cfg(not(feature = "sqlite-vec"))]
fn upsert_vector_index(
    _connection: &Connection,
    _provider: &impl EmbeddingProvider,
    _record: &EmbeddableRecord,
    _embedding: &[f32],
) -> Result<()> {
    Ok(())
}

fn mark_embedding_job_skipped(
    connection: &mut Connection,
    job_id: &str,
    reason: &str,
) -> Result<()> {
    connection.execute(
        "
        UPDATE embedding_jobs
        SET status = 'skipped',
            attempts = attempts + 1,
            error = ?2,
            updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
        WHERE id = ?1
        ",
        params![job_id, reason],
    )?;
    Ok(())
}

fn mark_embedding_job_failed(
    connection: &mut Connection,
    job_id: &str,
    reason: &str,
) -> Result<()> {
    connection.execute(
        "
        UPDATE embedding_jobs
        SET status = 'failed',
            attempts = attempts + 1,
            error = ?2,
            updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
        WHERE id = ?1
        ",
        params![job_id, reason],
    )?;
    Ok(())
}

fn count_pending_embedding_jobs(connection: &Connection, scope_chain: &[String]) -> Result<i64> {
    query_scoped_count(
        connection,
        "SELECT COUNT(*) FROM embedding_jobs WHERE status = 'pending' AND scope IN ({scopes})",
        scope_chain,
    )
}

#[derive(Debug, Clone)]
struct EmbeddableRecord {
    record_type: String,
    record_id: String,
    scope: String,
    content: String,
}

fn load_embeddable_record(
    connection: &Connection,
    record_type: &str,
    record_id: &str,
) -> Result<Option<EmbeddableRecord>> {
    match record_type {
        "entity" => connection
            .query_row(
                "
                SELECT id, scope, name || ' ' || entity_type
                FROM entities
                WHERE id = ?1 AND retired_at IS NULL
                ",
                [record_id],
                |row| {
                    Ok(EmbeddableRecord {
                        record_type: "entity".to_owned(),
                        record_id: row.get(0)?,
                        scope: row.get(1)?,
                        content: row.get(2)?,
                    })
                },
            )
            .optional()
            .map_err(Into::into),
        "observation" => connection
            .query_row(
                "
                SELECT o.id, e.scope, o.content
                FROM observations o
                JOIN entities e ON e.id = o.entity_id
                WHERE o.id = ?1 AND o.valid_to IS NULL AND e.retired_at IS NULL
                ",
                [record_id],
                |row| {
                    Ok(EmbeddableRecord {
                        record_type: "observation".to_owned(),
                        record_id: row.get(0)?,
                        scope: row.get(1)?,
                        content: row.get(2)?,
                    })
                },
            )
            .optional()
            .map_err(Into::into),
        "decision" => connection
            .query_row(
                "
                SELECT id, scope, title || ' ' || coalesce(reasoning, '')
                FROM decisions
                WHERE id = ?1 AND status != 'revoked'
                ",
                [record_id],
                |row| {
                    Ok(EmbeddableRecord {
                        record_type: "decision".to_owned(),
                        record_id: row.get(0)?,
                        scope: row.get(1)?,
                        content: row.get(2)?,
                    })
                },
            )
            .optional()
            .map_err(Into::into),
        "context" => connection
            .query_row(
                "
                SELECT key, scope, title || ' ' || content
                FROM context
                WHERE key = ?1 AND retired_at IS NULL
                ",
                [record_id],
                |row| {
                    Ok(EmbeddableRecord {
                        record_type: "context".to_owned(),
                        record_id: row.get(0)?,
                        scope: row.get(1)?,
                        content: row.get(2)?,
                    })
                },
            )
            .optional()
            .map_err(Into::into),
        _ => Ok(None),
    }
}

fn load_embeddable_records(
    connection: &Connection,
    scope_chain: &[String],
) -> Result<Vec<EmbeddableRecord>> {
    let mut records = Vec::new();

    records.extend(query_scoped_rows(
        connection,
        "
        SELECT id, scope, name || ' ' || entity_type
        FROM entities
        WHERE retired_at IS NULL AND scope IN ({scopes})
        ORDER BY updated_at ASC, id ASC
        ",
        scope_chain,
        |row| {
            Ok(EmbeddableRecord {
                record_type: "entity".to_owned(),
                record_id: row.get(0)?,
                scope: row.get(1)?,
                content: row.get(2)?,
            })
        },
    )?);
    records.extend(query_scoped_rows(
        connection,
        "
        SELECT o.id, e.scope, o.content
        FROM observations o
        JOIN entities e ON e.id = o.entity_id
        WHERE o.valid_to IS NULL AND e.retired_at IS NULL AND e.scope IN ({scopes})
        ORDER BY o.created_at ASC, o.id ASC
        ",
        scope_chain,
        |row| {
            Ok(EmbeddableRecord {
                record_type: "observation".to_owned(),
                record_id: row.get(0)?,
                scope: row.get(1)?,
                content: row.get(2)?,
            })
        },
    )?);
    records.extend(query_scoped_rows(
        connection,
        "
        SELECT id, scope, title || ' ' || coalesce(reasoning, '')
        FROM decisions
        WHERE status NOT IN ('revoked', 'superseded') AND scope IN ({scopes})
        ORDER BY created_at ASC, id ASC
        ",
        scope_chain,
        |row| {
            Ok(EmbeddableRecord {
                record_type: "decision".to_owned(),
                record_id: row.get(0)?,
                scope: row.get(1)?,
                content: row.get(2)?,
            })
        },
    )?);
    records.extend(query_scoped_rows(
        connection,
        "
        SELECT key, scope, title || ' ' || content
        FROM context
        WHERE retired_at IS NULL AND scope IN ({scopes})
        ORDER BY updated_at ASC, key ASC
        ",
        scope_chain,
        |row| {
            Ok(EmbeddableRecord {
                record_type: "context".to_owned(),
                record_id: row.get(0)?,
                scope: row.get(1)?,
                content: row.get(2)?,
            })
        },
    )?);

    Ok(records)
}

fn checksum(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Hard per-field ceilings at the write boundary. The HTTP layer caps the whole
/// request body, but a single multi-megabyte field sails under that cap and then
/// pollutes every downstream artifact verbatim — the briefing used to render a
/// 10MB decision title in full. Agents feed this system; oversized fields are a
/// matter of course, not an edge case. Truncation is marked so a clipped record
/// stays honest about being clipped.
pub(crate) const MAX_TITLE_CHARS: usize = 500;
pub(crate) const MAX_BODY_CHARS: usize = 20_000;
const TRUNCATION_MARK: &str = " …[truncated]";

pub(crate) fn clamp_field(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_owned();
    }
    let clipped: String = value.chars().take(max_chars).collect();
    format!("{clipped}{TRUNCATION_MARK}")
}

fn parse_relation_spec(raw: &str) -> Result<(String, String)> {
    let Some((target_id, relation_type)) = raw.split_once(':') else {
        return Err(GrafikiError::InvalidRelationType(raw.to_owned()));
    };
    Ok((
        target_id.trim().to_owned(),
        validate_relation_type(relation_type.trim())?,
    ))
}

fn json_array(items: &[String]) -> Result<String> {
    Ok(serde_json::to_string(items)?)
}

fn parse_json_list(value: Option<&str>) -> Vec<String> {
    value
        .and_then(|value| serde_json::from_str::<Vec<String>>(value).ok())
        .unwrap_or_default()
}

pub(crate) fn slugify(name: &str) -> String {
    let mut slug = String::new();
    let mut previous_dash = false;

    for ch in name.trim().chars().flat_map(char::to_lowercase) {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch);
            previous_dash = false;
        } else if !previous_dash {
            slug.push('-');
            previous_dash = true;
        }
    }

    slug.trim_matches('-').to_owned()
}

/// Resolve a stable readable entity id without overwriting an unrelated entity
/// whose name happens to have the same ASCII slug. Unicode-only names receive a
/// deterministic hash-backed id instead of the unsafe empty string.
fn entity_id_for_name(connection: &Connection, name: &str) -> Result<String> {
    let slug = slugify(name);
    let needs_hash = slug.is_empty();
    let base = if needs_hash {
        "entity".to_owned()
    } else {
        slug
    };
    let existing_name: Option<String> = connection
        .query_row("SELECT name FROM entities WHERE id = ?1", [&base], |row| {
            row.get(0)
        })
        .optional()?;
    if !needs_hash
        && existing_name
            .as_deref()
            .is_none_or(|existing| existing == name)
    {
        return Ok(base);
    }

    let digest = checksum(name);
    let candidate = format!("{base}-{}", &digest[..10]);
    let collision: Option<String> = connection
        .query_row(
            "SELECT name FROM entities WHERE id = ?1",
            [&candidate],
            |row| row.get(0),
        )
        .optional()?;
    if collision
        .as_deref()
        .is_some_and(|existing| existing != name)
    {
        // A SHA-256 prefix collision is extraordinarily unlikely, but never
        // allow it to become a destructive upsert.
        return Ok(format!("{base}-{digest}"));
    }
    Ok(candidate)
}

fn scoped_query(template: &str, scope_count: usize) -> String {
    if scope_count == 0 {
        // Browse semantics: an empty scope chain means "no scope filter".
        // `<col> LIKE '%'` is true for every (NOT NULL) scope value and keeps
        // the template's surrounding WHERE/AND structure intact.
        return template.replace("IN ({scopes})", "LIKE '%'");
    }
    template.replace("{scopes}", &placeholders(scope_count))
}

/// Scope chain for BROWSE surfaces (lists, detail views, extraction reads): an
/// empty scope means "everything", not "root only" — a record parked in a
/// sub-scope must never be invisible by default (2026-07-04 audit; same family
/// as the review-queue fix). A non-empty scope still narrows to its chain.
/// Retrieval/search/status keep ordinary chain semantics — there scope is a
/// relevance mechanism, not a browse filter.
fn browse_scope_chain(scope: &Scope) -> Vec<String> {
    if scope.as_str().is_empty() {
        Vec::new()
    } else {
        scope.chain().into_vec()
    }
}

fn placeholders(count: usize) -> String {
    std::iter::repeat_n("?", count)
        .collect::<Vec<_>>()
        .join(", ")
}

fn query_scoped_rows<T, F>(
    connection: &Connection,
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

fn search_keyword_memory(
    connection: &Connection,
    query: &str,
    record_type: &str,
    scope_chain: &[String],
    limit: usize,
) -> Result<Vec<SearchResult>> {
    let mut ranked_types: Vec<VecDeque<SearchResult>> = Vec::new();
    let fts_query = fts5_terms_query(query);
    if let Some(fts_query) = fts_query.as_deref() {
        if matches!(record_type, "all" | "entities") {
            ranked_types.push(search_entities(connection, fts_query, scope_chain, limit)?.into());
        }
        if matches!(record_type, "all" | "observations") {
            ranked_types
                .push(search_observations(connection, fts_query, scope_chain, limit)?.into());
        }
        if matches!(record_type, "all" | "decisions") {
            ranked_types.push(search_decisions(connection, fts_query, scope_chain, limit)?.into());
        }
        if matches!(record_type, "all" | "context") {
            ranked_types.push(search_context(connection, fts_query, scope_chain, limit)?.into());
        }
    }

    // A global append-then-truncate lets the first type consume the entire
    // result window (six entity hits could hide an exact observation). Preserve
    // every type-local relevance order, then fairly interleave the ranked arms.
    // The arm order is stable, so identical databases produce identical output.
    let mut results = Vec::with_capacity(limit);
    while results.len() < limit {
        let mut advanced = false;
        for arm in &mut ranked_types {
            if let Some(result) = arm.pop_front() {
                results.push(result);
                advanced = true;
                if results.len() == limit {
                    break;
                }
            }
        }
        if !advanced {
            break;
        }
    }
    Ok(results)
}

fn fts5_terms_query(raw: &str) -> Option<String> {
    let terms: Vec<String> = raw
        .split(|character: char| {
            !character.is_alphanumeric() && character != '_' && character != '-'
        })
        .map(str::trim)
        .filter(|term| term.len() > 1)
        .map(|term| format!("\"{}\"", term.replace('"', "\"\"")))
        .collect();
    (!terms.is_empty()).then(|| terms.join(" OR "))
}

fn search_semantic_memory(
    connection: &Connection,
    query: &str,
    record_type: &str,
    scope_chain: &[String],
    limit: usize,
) -> Result<Vec<SearchResult>> {
    let provider = configured_embedding_provider()?;
    let query_embedding = provider.embed(query)?;

    #[cfg(feature = "sqlite-vec")]
    {
        let sqlite_vec_results = search_sqlite_vec_memory(
            connection,
            provider.provider_name(),
            provider.model_name(),
            provider.dimension(),
            &query_embedding,
            record_type,
            scope_chain,
            limit,
        )?;
        if !sqlite_vec_results.is_empty() {
            return Ok(sqlite_vec_results);
        }
    }

    search_json_vector_memory(
        connection,
        provider.provider_name(),
        provider.model_name(),
        provider.dimension(),
        &query_embedding,
        record_type,
        scope_chain,
        limit,
    )
}

fn search_json_vector_memory(
    connection: &Connection,
    provider_name: &str,
    model_name: &str,
    dimension: usize,
    query_embedding: &[f32],
    record_type: &str,
    scope_chain: &[String],
    limit: usize,
) -> Result<Vec<SearchResult>> {
    let sql = scoped_query(
        "
        SELECT record_type, record_id, embedding
        FROM embedding_vectors
        WHERE provider = ?1
          AND model = ?2
          AND dimension = ?3
          AND scope IN ({scopes})
        ",
        scope_chain.len(),
    );
    let dimension = dimension as i64;
    let mut params: Vec<&dyn rusqlite::ToSql> = vec![&provider_name, &model_name, &dimension];
    params.extend(
        scope_chain
            .iter()
            .map(|scope| scope as &dyn rusqlite::ToSql),
    );
    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map(params.as_slice(), |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    })?;

    let mut scored = Vec::new();
    for row in rows {
        let (candidate_type, candidate_id, embedding_json) = row?;
        if !record_type_allows(record_type, &candidate_type) {
            continue;
        }
        let embedding: Vec<f32> = serde_json::from_str(&embedding_json)?;
        if embedding.len() != query_embedding.len() {
            continue;
        }
        let score = cosine_similarity(query_embedding, &embedding);
        if score <= 0.0 {
            continue;
        }
        scored.push((candidate_type, candidate_id, score));
    }
    scored.sort_by(|left, right| {
        right
            .2
            .partial_cmp(&left.2)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut results = Vec::new();
    for (candidate_type, candidate_id, score) in scored {
        if let Some(mut result) = load_search_result(connection, &candidate_type, &candidate_id)? {
            result.score = Some(round_search_score(score as f64));
            results.push(result);
        }
        if results.len() >= limit {
            break;
        }
    }
    Ok(results)
}

#[cfg(feature = "sqlite-vec")]
fn search_sqlite_vec_memory(
    connection: &Connection,
    provider_name: &str,
    model_name: &str,
    dimension: usize,
    query_embedding: &[f32],
    record_type: &str,
    scope_chain: &[String],
    limit: usize,
) -> Result<Vec<SearchResult>> {
    let table_name = sqlite_vec_table_name_parts(provider_name, model_name, dimension);
    if !sqlite_vec_table_exists(connection, &table_name)? {
        return Ok(Vec::new());
    }

    let backend = SqliteVecBackend::new(connection, &table_name, dimension)?;
    let candidate_limit = limit.saturating_mul(8).max(limit).min(500);
    let candidates = backend.search(query_embedding, candidate_limit)?;
    let mut seen = HashSet::new();
    let mut results = Vec::new();
    for candidate in candidates {
        if !record_type_allows(record_type, &candidate.record_type)
            || !scope_chain.iter().any(|scope| scope == &candidate.scope)
        {
            continue;
        }
        if !seen.insert((candidate.record_type.clone(), candidate.record_id.clone())) {
            continue;
        }
        if let Some(mut result) =
            load_search_result(connection, &candidate.record_type, &candidate.record_id)?
        {
            result.score = Some(round_search_score(candidate.score as f64));
            results.push(result);
        }
        if results.len() >= limit {
            break;
        }
    }
    Ok(results)
}

#[cfg(feature = "sqlite-vec")]
fn sqlite_vec_table_exists(connection: &Connection, table_name: &str) -> Result<bool> {
    let count: i64 = connection.query_row(
        "
        SELECT COUNT(*)
        FROM sqlite_schema
        WHERE type = 'table' AND name = ?1
        ",
        [table_name],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

#[cfg(feature = "sqlite-vec")]
fn sqlite_vec_table_name(provider: &impl EmbeddingProvider) -> String {
    sqlite_vec_table_name_parts(
        provider.provider_name(),
        provider.model_name(),
        provider.dimension(),
    )
}

#[cfg(feature = "sqlite-vec")]
fn sqlite_vec_table_name_parts(provider_name: &str, model_name: &str, dimension: usize) -> String {
    let key = format!("{provider_name}:{model_name}:{dimension}");
    let hash = checksum(&key);
    format!("embedding_vec_{}", &hash[..16])
}

/// H3 graph-aware arm: Personalized PageRank over the in-scope `relations` graph,
/// seeded from the entities surfaced by the lexical/dense arms, mapped back to
/// retrievable records (each ranked entity contributes itself + its live
/// observations). Returns an empty list when there are no seeds or no relations,
/// so it is a safe no-op on corpora without a graph.
/// Load the currently-valid relations subgraph whose endpoints are both in `scopes`,
/// into an in-memory [`crate::graph::Graph`]. Shared by H3 graph-retrieval (passes the
/// full scope chain) and H5 reflection (passes a single scope). The `ORDER BY` makes
/// the row stream — and therefore the per-node adjacency lists — canonical, which is
/// required for deterministic community detection (REFLECTION_DESIGN §0/C2).
fn load_scope_subgraph(connection: &Connection, scopes: &[String]) -> Result<crate::graph::Graph> {
    let placeholders = vec!["?"; scopes.len()].join(",");
    let sql = format!(
        "
        SELECT r.from_entity, r.to_entity, r.weight
        FROM relations r
        JOIN entities ef ON ef.id = r.from_entity
        JOIN entities et ON et.id = r.to_entity
        WHERE r.valid_to IS NULL
          AND ef.retired_at IS NULL
          AND et.retired_at IS NULL
          AND ef.scope IN ({placeholders}) AND et.scope IN ({placeholders})
        ORDER BY r.from_entity, r.to_entity, r.relation
        "
    );
    let mut params: Vec<&dyn rusqlite::ToSql> = Vec::with_capacity(scopes.len() * 2);
    for scope in scopes {
        params.push(scope);
    }
    for scope in scopes {
        params.push(scope);
    }
    let mut graph = crate::graph::Graph::new();
    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map(params.as_slice(), |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, f64>(2)?,
        ))
    })?;
    for row in rows {
        let (from, to, weight) = row?;
        graph.add_edge(&from, &to, weight);
    }
    Ok(graph)
}

/// H5 — detect entity communities in the in-scope relations graph, build a
/// deterministic extractive summary of each, and propose each as a **pending**
/// `context` candidate with provenance (`evidence_links`) and redaction. Never
/// auto-approves. Idempotent: a membership/source-fact dedup key skips re-proposing an
/// unchanged community. v1 is single-scope. See `docs/REFLECTION_DESIGN.md`.
pub fn run_reflection(
    options: crate::reflection::RunReflectionOptions,
) -> Result<crate::reflection::ReflectionReport> {
    use crate::reflection::{
        build_summary, community_dedup_key, CommunityDetail, ReflectionReport, SourceObservation,
    };

    let min_size = options.min_community_size.max(1);
    let max_size = options.max_community_size.max(min_size);
    let max_obs = options.max_obs_per_summary.max(1);

    let scope = Scope::new(&options.scope)?;
    let (project, connection) = resolve_and_open(
        options.project_name.clone(),
        options.start_dir.clone(),
        options.grafiki_home.clone(),
    )?;

    // v1 is single-scope (REFLECTION_DESIGN §0/C7): detect over exactly the run scope,
    // so every member shares one scope and the candidate's scope is unambiguous.
    let scopes = [scope.as_str().to_string()];
    let graph = load_scope_subgraph(&connection, &scopes)?;
    let communities = crate::graph::detect_communities(&graph);
    let communities_detected = communities.len();

    let scope_slug = {
        let s = slugify(scope.as_str());
        if s.is_empty() {
            "root".to_string()
        } else {
            s
        }
    };

    let mut details: Vec<CommunityDetail> = Vec::new();
    let mut communities_summarized = 0usize;
    let mut candidates_created = 0usize;
    let mut skipped_existing = 0usize;
    let mut skipped_too_large = 0usize;

    let mut name_stmt = connection
        .prepare("SELECT name FROM entities WHERE id = ?1 AND scope = ?2 AND retired_at IS NULL")?;
    let mut obs_stmt = connection.prepare(
        "SELECT id, content, category, confidence FROM observations \
         WHERE entity_id = ?1 AND valid_to IS NULL ORDER BY id",
    )?;

    for community in &communities {
        let size = community.members.len();
        if size < min_size {
            continue; // singleton / sub-theme: counted in `detected`, not summarizable
        }
        let member_ids = community.members.clone();
        // Community cohesion Q_c (modularity contribution) — discounts loose communities
        // in the proposed confidence (§4.4) and is reported per community.
        let modularity = crate::graph::community_modularity(&graph, &member_ids);

        // Too-large guard (C6): skip rather than emit a meaningless mega-summary.
        if size > max_size {
            details.push(CommunityDetail {
                community_id: community.id,
                member_entity_ids: member_ids.clone(),
                member_entity_names: Vec::new(),
                observation_count: 0,
                modularity_contribution: modularity,
                dedup_key: community_dedup_key(scope.as_str(), &member_ids, &[]),
                candidate_id: None,
                status: "skipped_too_large".to_string(),
            });
            skipped_too_large += 1;
            continue;
        }

        // Within-community weighted degree per member = stable, local salience (C5):
        // unaffected by edges/PPR elsewhere in the scope.
        let member_set: std::collections::BTreeSet<&str> =
            member_ids.iter().map(String::as_str).collect();
        let salience = |member: &str| -> f64 {
            graph
                .neighbors(member)
                .iter()
                .filter(|(nbr, _)| member_set.contains(nbr.as_str()))
                .map(|(_, w)| *w)
                .sum()
        };

        // Load member names + their currently-valid observations, REDACTING each
        // observation at the source (C1) so neither the summary nor the evidence
        // excerpt can carry a secret that ingest failed to scrub.
        let mut member_names: Vec<String> = Vec::with_capacity(size);
        let mut observations: Vec<SourceObservation> = Vec::new();
        for member in &member_ids {
            let name: String = name_stmt
                .query_row(params![member, scope.as_str()], |row| row.get(0))
                .optional()?
                .unwrap_or_else(|| member.clone());
            member_names.push(name.clone());
            let member_salience = salience(member);
            let rows = obs_stmt.query_map([member], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, f64>(3)?,
                ))
            })?;
            for row in rows {
                let (obs_id, content, category, confidence) = row?;
                let (redacted, _) = redact_text(&content);
                observations.push(SourceObservation {
                    observation_id: obs_id,
                    entity_id: member.clone(),
                    entity_name: name.clone(),
                    content: redacted,
                    category,
                    confidence,
                    salience: member_salience,
                });
            }
        }

        if observations.is_empty() {
            details.push(CommunityDetail {
                community_id: community.id,
                member_entity_ids: member_ids.clone(),
                member_entity_names: member_names,
                observation_count: 0,
                modularity_contribution: modularity,
                dedup_key: community_dedup_key(scope.as_str(), &member_ids, &[]),
                candidate_id: None,
                status: "skipped_no_observations".to_string(),
            });
            continue;
        }

        let draft = build_summary(
            scope.as_str(),
            &member_ids,
            &member_names,
            &observations,
            max_obs,
        );
        let context_key = format!("reflection-{scope_slug}-{}", draft.dedup_key);

        // Dedup. An approved `context` row with this key is an UNCONDITIONAL skip — even
        // under `--force` — so reflection never silently overwrites an existing
        // consolidation. (`add_context` now UPSERTS on a key collision rather than
        // crashing, so this is a correctness/idempotency choice, no longer a
        // crash-avoidance one — §4.6.) The extraction_candidates check (any status incl.
        // `rejected`, so a human rejection is durable) is the part `--force` bypasses to
        // re-propose a fresh review.
        let context_hit: Option<i64> = connection
            .query_row(
                "SELECT 1 FROM context WHERE key = ?1 AND retired_at IS NULL LIMIT 1",
                [&context_key],
                |row| row.get(0),
            )
            .optional()?;
        let candidate_hit: Option<i64> = if options.force {
            None
        } else {
            connection
                .query_row(
                    "SELECT 1 FROM extraction_candidates \
                     WHERE scope = ?1 AND json_extract(payload, '$.dedup_key') = ?2 \
                       AND status IN ('pending','approved','rejected') LIMIT 1",
                    params![scope.as_str(), draft.dedup_key],
                    |row| row.get(0),
                )
                .optional()?
        };
        if context_hit.is_some() || candidate_hit.is_some() {
            details.push(CommunityDetail {
                community_id: community.id,
                member_entity_ids: member_ids.clone(),
                member_entity_names: member_names,
                observation_count: draft.kept.len(),
                modularity_contribution: modularity,
                dedup_key: draft.dedup_key,
                candidate_id: None,
                status: "skipped_existing".to_string(),
            });
            skipped_existing += 1;
            continue;
        }

        // Confidence (§4.4): mean of ALL member-observation confidences (not just the
        // kept representatives) discounted by community cohesion. `cohesion` maps the
        // modularity contribution Q_c into [0.5, 1.0] (Q_ref = 0.3), so a loose community
        // is discounted while a tight one is not. Deterministic (all store-derived).
        let mean_conf =
            observations.iter().map(|o| o.confidence).sum::<f64>() / observations.len() as f64;
        let cohesion = 0.5 + 0.5 * (modularity / 0.3).clamp(0.0, 1.0);
        let confidence = (mean_conf * cohesion).clamp(0.0, 1.0);

        let payload = serde_json::json!({
            "key": context_key,
            "title": draft.title,
            "category": "architecture",
            "content": draft.content,
            "members": member_ids.clone(),
            "reflection_version": 1,
            "dedup_key": draft.dedup_key,
        });
        // One evidence link per kept observation. The excerpt is the already-redacted
        // content (C1) — `insert_candidate_evidence` only compacts, it does not redact.
        let evidence: Vec<EvidenceInput> = draft
            .kept
            .iter()
            .map(|o| EvidenceInput {
                source_event_id: None,
                source_type: "reflection".to_string(),
                source: Some("reflection:louvain:v1".to_string()),
                title: Some(o.entity_name.clone()),
                excerpt: o.content.clone(),
                uri: Some(format!("grafiki://observation/{}", o.observation_id)),
                byte_start: None,
                byte_end: None,
                line_start: None,
                line_end: None,
                captured_at: None,
            })
            .collect();
        let rationale = format!(
            "Community reflection over {} entities (modularity contribution {modularity:.4}); \
             {} source observations.",
            member_ids.len(),
            draft.kept.len()
        );

        let proposed = propose_candidate(ProposeCandidateOptions {
            project_name: options.project_name.clone(),
            start_dir: options.start_dir.clone(),
            grafiki_home: options.grafiki_home.clone(),
            source_type: "reflection".to_string(),
            source: Some("reflection:louvain:v1".to_string()),
            record_type: "context".to_string(),
            payload,
            scope: scope.as_str().to_string(),
            confidence,
            rationale: Some(rationale),
            evidence,
        })?;

        communities_summarized += 1;
        candidates_created += 1;
        details.push(CommunityDetail {
            community_id: community.id,
            member_entity_ids: member_ids,
            member_entity_names: member_names,
            observation_count: draft.kept.len(),
            modularity_contribution: modularity,
            dedup_key: draft.dedup_key,
            candidate_id: Some(proposed.candidate.id),
            status: "created".to_string(),
        });
    }

    Ok(ReflectionReport {
        project: project.project,
        scope: scope.as_str().to_string(),
        communities_detected,
        communities_summarized,
        candidates_created,
        skipped_existing,
        skipped_too_large,
        details,
    })
}

fn graph_search_results(
    connection: &Connection,
    scope_chain: &[String],
    keyword_results: &[SearchResult],
    semantic_results: &[SearchResult],
    limit: usize,
) -> Result<Vec<SearchResult>> {
    use crate::graph::{
        personalized_pagerank, DEFAULT_DAMPING, DEFAULT_MAX_ITERS, DEFAULT_TOLERANCE,
    };

    // 1. Seeds: entity ids from the prior arms, weighted by rank (earlier ⇒ stronger).
    let mut seeds: std::collections::BTreeMap<String, f64> = std::collections::BTreeMap::new();
    for results in [keyword_results, semantic_results] {
        for (rank, result) in results.iter().enumerate() {
            let entity_id = match result.record_type.as_str() {
                "entity" => Some(result.id.clone()),
                // search_observations returns the owning entity_id in `title`.
                "observation" => Some(result.title.clone()),
                _ => None,
            };
            if let Some(id) = entity_id {
                *seeds.entry(id).or_insert(0.0) += 1.0 / (rank as f64 + 1.0);
            }
        }
    }
    if seeds.is_empty() {
        return Ok(Vec::new());
    }

    // 2. Load the in-scope, currently-valid relations subgraph (both endpoints in scope).
    let graph = load_scope_subgraph(connection, scope_chain)?;
    // No graph, or no seed actually lands on a node → nothing to add.
    if graph.is_empty() || !seeds.keys().any(|s| graph.contains(s)) {
        return Ok(Vec::new());
    }

    // 3. Personalized PageRank.
    let scores = personalized_pagerank(
        &graph,
        &seeds,
        DEFAULT_DAMPING,
        DEFAULT_MAX_ITERS,
        DEFAULT_TOLERANCE,
    );

    // 4. Rank entities by PPR (desc, id tie-break) and map back to records.
    let mut ranked: Vec<(String, f64)> = scores.into_iter().filter(|(_, s)| *s > 0.0).collect();
    ranked.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.0.cmp(&b.0))
    });

    let mut out = Vec::new();
    // Prepared once and reused across ranked entities (avoids re-compiling the SQL
    // per iteration).
    let mut obs_statement = connection.prepare(
        "SELECT id, content FROM observations WHERE entity_id = ?1 AND valid_to IS NULL",
    )?;
    for (entity_id, _score) in ranked {
        let entity: Option<(String, String, String)> = connection
            .query_row(
                "SELECT name, entity_type, scope FROM entities WHERE id = ?1 AND retired_at IS NULL",
                [&entity_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;
        let Some((name, entity_type, scope)) = entity else {
            continue;
        };
        if !scope_chain.iter().any(|s| s == &scope) {
            continue;
        }
        out.push(SearchResult {
            record_type: "entity".to_owned(),
            id: entity_id.clone(),
            title: name,
            snippet: entity_type,
            scope: scope.clone(),
            score: None,
            evidence: Vec::new(),
        });
        let obs = obs_statement.query_map([&entity_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        for row in obs {
            let (obs_id, content) = row?;
            out.push(SearchResult {
                record_type: "observation".to_owned(),
                id: obs_id,
                title: entity_id.clone(),
                snippet: content,
                scope: scope.clone(),
                score: None,
                evidence: Vec::new(),
            });
        }
        if out.len() >= limit {
            break;
        }
    }
    Ok(out)
}

/// H4: reorder a fused candidate list with the local cross-encoder reranker (when
/// the model is available), truncating to `limit`. Returns the (possibly
/// reordered) results plus an optional fallback note when reranking did not run.
fn rerank_results(
    query: &str,
    candidates: Vec<SearchResult>,
    limit: usize,
) -> (Vec<SearchResult>, Option<String>) {
    #[cfg(feature = "fastembed")]
    {
        if candidates.is_empty() {
            return (candidates, None);
        }
        let docs: Vec<String> = candidates
            .iter()
            .map(|r| format!("{} {}", r.title, r.snippet))
            .collect();
        match crate::embeddings::rerank_documents(query, &docs) {
            Ok(scored) => {
                let mut out = Vec::with_capacity(limit);
                for (index, score) in scored.into_iter().take(limit) {
                    if let Some(result) = candidates.get(index) {
                        let mut result = result.clone();
                        result.score = Some(round_search_score(score as f64));
                        out.push(result);
                    }
                }
                (out, None)
            }
            Err(error) => {
                let mut fused = candidates;
                fused.truncate(limit);
                (
                    fused,
                    Some(format!(
                        "Reranking unavailable ({error}); returned fused results."
                    )),
                )
            }
        }
    }
    #[cfg(not(feature = "fastembed"))]
    {
        let _ = query;
        let mut fused = candidates;
        fused.truncate(limit);
        (
            fused,
            Some(
                "Reranking requires building with the `fastembed` feature; returned fused results."
                    .to_owned(),
            ),
        )
    }
}

/// RRF rank-decay constant (shared by the fusion and the temporal-boost scaling).
const RRF_K: f64 = 45.0;
/// One rank-0 RRF score unit — the scale a `temporal_weight` of 1.0 is worth.
pub(crate) const RRF_UNIT: f64 = 1.0 / (RRF_K + 1.0);

/// Compute the additive temporal boost per candidate (M-E1/M-E2). Empty when
/// `temporal_weight <= 0`. Boost = `temporal_weight · RRF_UNIT · temporal_term(recency, salience)`:
/// recency = Weibull freshness of the record's timestamp by category; salience = reuse from the
/// `agent_queries` audit log. The decay math is pure (`crate::decay`); this fn does only the I/O.
fn temporal_boosts(
    connection: &Connection,
    scope_chain: &[String],
    candidates: &[(String, String)],
    temporal_weight: f64,
) -> Result<HashMap<(String, String), f64>> {
    let mut boosts: HashMap<(String, String), f64> = HashMap::new();
    if temporal_weight <= 0.0 || candidates.is_empty() {
        return Ok(boosts);
    }
    let placeholders = |n: usize| vec!["?"; n].join(",");

    // 1. Recency: (age_hours, category) per candidate, by record type. Types without a
    //    meaningful timestamp (entity/state) are recency-neutral and simply omitted.
    let mut recency: HashMap<(String, String), (f64, String)> = HashMap::new();
    let mut by_type: std::collections::BTreeMap<&str, Vec<&str>> =
        std::collections::BTreeMap::new();
    for (ty, id) in candidates {
        by_type.entry(ty.as_str()).or_default().push(id.as_str());
    }
    for (ty, ids) in &by_type {
        let sql = match *ty {
            "observation" => format!(
                "SELECT id, category, (strftime('%s','now') - strftime('%s', valid_from))/3600.0 \
                 FROM observations WHERE valid_to IS NULL AND id IN ({})",
                placeholders(ids.len())
            ),
            "decision" => format!(
                "SELECT id, 'decision', (strftime('%s','now') - strftime('%s', created_at))/3600.0 \
                 FROM decisions WHERE id IN ({})",
                placeholders(ids.len())
            ),
            "context" => format!(
                "SELECT key, 'general', (strftime('%s','now') - strftime('%s', updated_at))/3600.0 \
                 FROM context WHERE key IN ({})",
                placeholders(ids.len())
            ),
            _ => continue,
        };
        let mut stmt = connection.prepare(&sql)?;
        let params: Vec<&dyn rusqlite::ToSql> =
            ids.iter().map(|s| s as &dyn rusqlite::ToSql).collect();
        let rows = stmt.query_map(params.as_slice(), |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<f64>>(2)?.unwrap_or(0.0),
            ))
        })?;
        for row in rows {
            let (id, category, age_hours) = row?;
            recency.insert(((*ty).to_string(), id), (age_hours, category));
        }
    }

    // 2. Salience: access count + last-access age per "type:id" from the audit log.
    let mut salience: HashMap<String, (u64, f64)> = HashMap::new();
    {
        let sql = format!(
            "SELECT je.value, COUNT(*), \
                (strftime('%s','now') - strftime('%s', MAX(aq.created_at)))/3600.0 \
             FROM agent_queries aq, json_each(aq.returned_ids) je \
             WHERE aq.scope IN ({}) GROUP BY je.value",
            placeholders(scope_chain.len())
        );
        let mut stmt = connection.prepare(&sql)?;
        let params: Vec<&dyn rusqlite::ToSql> = scope_chain
            .iter()
            .map(|s| s as &dyn rusqlite::ToSql)
            .collect();
        let rows = stmt.query_map(params.as_slice(), |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?.max(0) as u64,
                row.get::<_, Option<f64>>(2)?.unwrap_or(0.0),
            ))
        })?;
        for row in rows {
            let (key, count, last_age) = row?;
            salience.insert(key, (count, last_age));
        }
    }

    // 3. Combine into the additive boost.
    for (ty, id) in candidates {
        let key = (ty.clone(), id.clone());
        let rec = recency
            .get(&key)
            .map(|(age, cat)| crate::decay::category_freshness(cat, *age))
            .unwrap_or(0.0);
        let sal = salience
            .get(&format!("{ty}:{id}"))
            .map(|(count, last_age)| crate::decay::reuse_salience(*count, *last_age))
            .unwrap_or(0.0);
        let term = crate::decay::temporal_term(rec, sal);
        if term > 0.0 {
            boosts.insert(key, temporal_weight * RRF_UNIT * term);
        }
    }
    Ok(boosts)
}

fn hybrid_search_results(
    query: &str,
    keyword_results: Vec<SearchResult>,
    semantic_results: Vec<SearchResult>,
    graph_results: Vec<SearchResult>,
    limit: usize,
    temporal_boost: &HashMap<(String, String), f64>,
) -> Vec<SearchResult> {
    let mut scored: HashMap<(String, String), HybridScore> = HashMap::new();
    add_hybrid_scores(&mut scored, query, keyword_results, SearchSource::Keyword);
    add_hybrid_scores(&mut scored, query, semantic_results, SearchSource::Semantic);
    add_hybrid_scores(&mut scored, query, graph_results, SearchSource::Graph);
    // M-E1/M-E2: add the precomputed temporal boost (recency + reuse salience) before ranking.
    // Empty map (the default, temporal_weight=0) leaves every score untouched.
    if !temporal_boost.is_empty() {
        for (key, hybrid) in scored.iter_mut() {
            if let Some(bonus) = temporal_boost.get(key) {
                hybrid.score += *bonus;
            }
        }
    }
    let mut results: Vec<_> = scored.into_values().collect();
    results.sort_by(|left, right| {
        right
            .score
            .partial_cmp(&left.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.best_rank.cmp(&right.best_rank))
            .then_with(|| left.result.record_type.cmp(&right.result.record_type))
            .then_with(|| left.result.id.cmp(&right.result.id))
    });
    results
        .into_iter()
        .map(|mut result| {
            result.result.score = Some(round_search_score(result.score));
            result.result
        })
        .take(limit)
        .collect()
}

#[derive(Debug, Clone)]
struct HybridScore {
    result: SearchResult,
    score: f64,
    sources: u8,
    best_rank: usize,
}

#[derive(Debug, Clone, Copy)]
enum SearchSource {
    Keyword,
    Semantic,
    Graph,
}

impl SearchSource {
    fn weight(self) -> f64 {
        match self {
            Self::Keyword => 1.10,
            Self::Semantic => 1.00,
            // Slightly below the direct-match arms: graph reachability is a
            // weaker (structural) signal than a direct lexical/dense hit.
            Self::Graph => 0.90,
        }
    }

    fn bit(self) -> u8 {
        match self {
            Self::Keyword => 0b001,
            Self::Semantic => 0b010,
            Self::Graph => 0b100,
        }
    }
}

fn add_hybrid_scores(
    scored: &mut HashMap<(String, String), HybridScore>,
    query: &str,
    results: Vec<SearchResult>,
    source: SearchSource,
) {
    const CROSS_SOURCE_BONUS: f64 = 0.018;
    for (rank, result) in results.into_iter().enumerate() {
        let key = (result.record_type.clone(), result.id.clone());
        let source_bit = source.bit();
        let rank_score = source.weight() / (RRF_K + rank as f64 + 1.0);
        let score = rank_score + text_match_boost(query, &result);
        scored
            .entry(key)
            .and_modify(|existing| {
                if existing.sources & source_bit == 0 {
                    existing.score += CROSS_SOURCE_BONUS;
                }
                existing.score += score;
                existing.sources |= source_bit;
                existing.best_rank = existing.best_rank.min(rank);
            })
            .or_insert(HybridScore {
                result,
                score,
                sources: source_bit,
                best_rank: rank,
            });
    }
}

fn text_match_boost(query: &str, result: &SearchResult) -> f64 {
    let normalized_query = query.trim().to_ascii_lowercase();
    if normalized_query.is_empty() {
        return 0.0;
    }

    let title = result.title.to_ascii_lowercase();
    let snippet = result.snippet.to_ascii_lowercase();
    let mut boost = 0.0;

    if title == normalized_query {
        boost += 0.020;
    } else if title.contains(&normalized_query) {
        boost += 0.012;
    }
    if snippet.contains(&normalized_query) {
        boost += 0.008;
    }

    let terms = search_terms(query);
    if !terms.is_empty() {
        let title_hits = terms.iter().filter(|term| title.contains(*term)).count();
        let snippet_hits = terms.iter().filter(|term| snippet.contains(*term)).count();
        let term_count = terms.len() as f64;
        boost += 0.006 * title_hits as f64 / term_count;
        boost += 0.004 * snippet_hits as f64 / term_count;
    }

    boost
}

fn search_terms(query: &str) -> Vec<String> {
    query
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .filter(|term| !term.is_empty())
        .map(|term| term.to_ascii_lowercase())
        .collect()
}

fn round_search_score(score: f64) -> f64 {
    if !score.is_finite() {
        return 0.0;
    }
    (score * 10_000.0).round() / 10_000.0
}

fn record_type_allows(filter: &str, record_type: &str) -> bool {
    matches!(
        (filter, record_type),
        ("all", _)
            | ("entities", "entity")
            | ("observations", "observation")
            | ("decisions", "decision")
            | ("context", "context")
    )
}

fn normalize_memory_record_type(record_type: &str) -> Result<String> {
    let normalized = match record_type.trim().to_ascii_lowercase().as_str() {
        "entity" | "entities" => "entity",
        "observation" | "observations" => "observation",
        "decision" | "decisions" => "decision",
        "context" | "contexts" => "context",
        "state" | "state_item" | "states" => "state",
        "relation" | "relations" => "relation",
        "session" | "sessions" => "session",
        other => return Err(GrafikiError::InvalidRecordType(other.to_owned())),
    };
    Ok(normalized.to_owned())
}

fn load_search_result(
    connection: &Connection,
    record_type: &str,
    record_id: &str,
) -> Result<Option<SearchResult>> {
    match record_type {
        "entity" => connection
            .query_row(
                "
                SELECT id, name, entity_type, scope
                FROM entities
                WHERE id = ?1 AND retired_at IS NULL
                ",
                [record_id],
                |row| {
                    Ok(SearchResult {
                        record_type: "entity".to_owned(),
                        id: row.get(0)?,
                        title: row.get(1)?,
                        snippet: row.get(2)?,
                        scope: row.get(3)?,
                        score: None,
                        evidence: Vec::new(),
                    })
                },
            )
            .optional()
            .map_err(Into::into),
        "observation" => connection
            .query_row(
                "
                SELECT o.id, e.id, o.content, e.scope
                FROM observations o
                JOIN entities e ON e.id = o.entity_id
                WHERE o.id = ?1 AND o.valid_to IS NULL AND e.retired_at IS NULL
                ",
                [record_id],
                |row| {
                    Ok(SearchResult {
                        record_type: "observation".to_owned(),
                        id: row.get(0)?,
                        title: row.get(1)?,
                        snippet: row.get(2)?,
                        scope: row.get(3)?,
                        score: None,
                        evidence: Vec::new(),
                    })
                },
            )
            .optional()
            .map_err(Into::into),
        "decision" => connection
            .query_row(
                "
                SELECT id, title, coalesce(reasoning, ''), scope
                FROM decisions
                WHERE id = ?1 AND status NOT IN ('revoked', 'superseded')
                ",
                [record_id],
                |row| {
                    Ok(SearchResult {
                        record_type: "decision".to_owned(),
                        id: row.get(0)?,
                        title: row.get(1)?,
                        snippet: row.get(2)?,
                        scope: row.get(3)?,
                        score: None,
                        evidence: Vec::new(),
                    })
                },
            )
            .optional()
            .map_err(Into::into),
        "context" => connection
            .query_row(
                "
                SELECT key, title, content, scope
                FROM context
                WHERE key = ?1 AND retired_at IS NULL
                ",
                [record_id],
                |row| {
                    Ok(SearchResult {
                        record_type: "context".to_owned(),
                        id: row.get(0)?,
                        title: row.get(1)?,
                        snippet: row.get(2)?,
                        scope: row.get(3)?,
                        score: None,
                        evidence: Vec::new(),
                    })
                },
            )
            .optional()
            .map_err(Into::into),
        _ => Ok(None),
    }
}

fn search_entities(
    connection: &Connection,
    query: &str,
    scope_chain: &[String],
    limit: usize,
) -> Result<Vec<SearchResult>> {
    // FTS5 over name + entity_type, mirroring the other record types. `query` is
    // the already-sanitized FTS terms query (see `fts5_terms_query`), so a
    // natural-language query matches by token instead of a useless full-string
    // LIKE.
    let sql = scoped_query(
        "
        SELECT e.id, e.name, e.entity_type, e.scope
        FROM entities_fts f
        JOIN entities e ON e.rowid = f.rowid
        WHERE entities_fts MATCH ? AND e.retired_at IS NULL AND e.scope IN ({scopes})
        ORDER BY rank
        LIMIT ?
        ",
        scope_chain.len(),
    );
    query_search_rows(connection, &sql, query, scope_chain, limit, |row| {
        let id: String = row.get(0)?;
        let name: String = row.get(1)?;
        let entity_type: String = row.get(2)?;
        let scope: String = row.get(3)?;
        Ok(SearchResult {
            record_type: "entity".to_owned(),
            id,
            title: name,
            snippet: entity_type,
            scope,
            score: None,
            evidence: Vec::new(),
        })
    })
}

fn search_observations(
    connection: &Connection,
    query: &str,
    scope_chain: &[String],
    limit: usize,
) -> Result<Vec<SearchResult>> {
    let sql = scoped_query(
        "
        SELECT o.id, e.id, o.content, e.scope
        FROM observations_fts f
        JOIN observations o ON o.rowid = f.rowid
        JOIN entities e ON e.id = o.entity_id
        WHERE observations_fts MATCH ? AND o.valid_to IS NULL
          AND e.retired_at IS NULL AND e.scope IN ({scopes})
        ORDER BY rank
        LIMIT ?
        ",
        scope_chain.len(),
    );
    query_search_rows(connection, &sql, query, scope_chain, limit, |row| {
        let id: String = row.get(0)?;
        let entity_id: String = row.get(1)?;
        let content: String = row.get(2)?;
        let scope: String = row.get(3)?;
        Ok(SearchResult {
            record_type: "observation".to_owned(),
            id,
            title: entity_id,
            snippet: content,
            scope,
            score: None,
            evidence: Vec::new(),
        })
    })
}

fn search_decisions(
    connection: &Connection,
    query: &str,
    scope_chain: &[String],
    limit: usize,
) -> Result<Vec<SearchResult>> {
    let sql = scoped_query(
        "
        SELECT d.id, d.title, coalesce(d.reasoning, ''), d.scope
        FROM decisions_fts f
        JOIN decisions d ON d.rowid = f.rowid
        WHERE decisions_fts MATCH ?
          AND d.status NOT IN ('revoked', 'superseded') AND d.scope IN ({scopes})
        ORDER BY rank
        LIMIT ?
        ",
        scope_chain.len(),
    );
    query_search_rows(connection, &sql, query, scope_chain, limit, |row| {
        let id: String = row.get(0)?;
        let title: String = row.get(1)?;
        let reasoning: String = row.get(2)?;
        let scope: String = row.get(3)?;
        Ok(SearchResult {
            record_type: "decision".to_owned(),
            id,
            title,
            snippet: reasoning,
            scope,
            score: None,
            evidence: Vec::new(),
        })
    })
}

fn search_context(
    connection: &Connection,
    query: &str,
    scope_chain: &[String],
    limit: usize,
) -> Result<Vec<SearchResult>> {
    let sql = scoped_query(
        "
        SELECT c.key, c.title, c.content, c.scope
        FROM context_fts f
        JOIN context c ON c.rowid = f.rowid
        WHERE context_fts MATCH ? AND c.retired_at IS NULL AND c.scope IN ({scopes})
        ORDER BY rank
        LIMIT ?
        ",
        scope_chain.len(),
    );
    query_search_rows(connection, &sql, query, scope_chain, limit, |row| {
        let id: String = row.get(0)?;
        let title: String = row.get(1)?;
        let content: String = row.get(2)?;
        let scope: String = row.get(3)?;
        Ok(SearchResult {
            record_type: "context".to_owned(),
            id,
            title,
            snippet: content,
            scope,
            score: None,
            evidence: Vec::new(),
        })
    })
}

fn relations_for_entity(connection: &Connection, entity_id: &str) -> Result<Vec<GraphRelation>> {
    let mut statement = connection.prepare(
        "
        SELECT id, from_entity, to_entity, relation, weight, confidence, source_type, source
        FROM relations
        WHERE valid_to IS NULL AND (from_entity = ?1 OR to_entity = ?1)
        ORDER BY created_at ASC, id ASC
        ",
    )?;
    let rows = statement.query_map([entity_id], graph_relation_from_row)?;
    collect_rows(rows)
}

fn load_graph_relation(connection: &Connection, relation_id: &str) -> Result<GraphRelation> {
    connection
        .query_row(
            "
            SELECT id, from_entity, to_entity, relation, weight, confidence, source_type, source
            FROM relations
            WHERE id = ?1 AND valid_to IS NULL
            ",
            [relation_id],
            graph_relation_from_row,
        )
        .optional()?
        .ok_or_else(|| GrafikiError::RelationNotFound(relation_id.to_owned()))
}

fn load_graph_entities(
    connection: &Connection,
    entity_ids: &HashSet<String>,
) -> Result<Vec<GraphEntity>> {
    let mut entities = Vec::new();
    for entity_id in entity_ids {
        entities.push(load_graph_entity(connection, entity_id)?);
    }
    entities.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(entities)
}

fn load_graph_entity(connection: &Connection, entity_id: &str) -> Result<GraphEntity> {
    connection
        .query_row(
            "
            SELECT id, name, entity_type, scope
            FROM entities
            WHERE id = ?1 AND retired_at IS NULL
            ",
            [entity_id],
            |row| {
                Ok(GraphEntity {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    entity_type: row.get(2)?,
                    scope: row.get(3)?,
                })
            },
        )
        .optional()?
        .ok_or_else(|| GrafikiError::EntityNotFound(entity_id.to_owned()))
}

fn graph_relation_from_row(row: &Row<'_>) -> rusqlite::Result<GraphRelation> {
    Ok(GraphRelation {
        id: row.get(0)?,
        from_entity: row.get(1)?,
        to_entity: row.get(2)?,
        relation: row.get(3)?,
        weight: row.get(4)?,
        confidence: row.get(5)?,
        source_type: row.get(6)?,
        source: row.get(7)?,
    })
}

fn count_scoped_entities(connection: &Connection, scope_chain: &[String]) -> Result<i64> {
    query_scoped_count(
        connection,
        "SELECT COUNT(*) FROM entities WHERE retired_at IS NULL AND scope IN ({scopes})",
        scope_chain,
    )
}

fn count_scoped_relations(connection: &Connection, scope_chain: &[String]) -> Result<i64> {
    let placeholders = placeholders(scope_chain.len());
    let sql = format!(
        "
        SELECT COUNT(*)
        FROM relations r
        JOIN entities f ON f.id = r.from_entity
        JOIN entities t ON t.id = r.to_entity
        WHERE r.valid_to IS NULL
          AND f.retired_at IS NULL
          AND t.retired_at IS NULL
          AND f.scope IN ({placeholders})
          AND t.scope IN ({placeholders})
        "
    );
    let repeated = repeat_scope_chain(scope_chain, 2);
    connection
        .query_row(&sql, params_from_iter(repeated.iter()), |row| row.get(0))
        .map_err(Into::into)
}

fn count_scoped_observations(connection: &Connection, scope_chain: &[String]) -> Result<i64> {
    query_scoped_count(
        connection,
        "
        SELECT COUNT(*)
        FROM observations o
        JOIN entities e ON e.id = o.entity_id
        WHERE o.valid_to IS NULL AND e.scope IN ({scopes})
        ",
        scope_chain,
    )
}

fn count_scoped_decisions(connection: &Connection, scope_chain: &[String]) -> Result<i64> {
    query_scoped_count(
        connection,
        "SELECT COUNT(*) FROM decisions
         WHERE status NOT IN ('revoked', 'superseded') AND scope IN ({scopes})",
        scope_chain,
    )
}

fn count_scoped_active_sessions(connection: &Connection, scope_chain: &[String]) -> Result<i64> {
    query_scoped_count(
        connection,
        "SELECT COUNT(*) FROM sessions WHERE status = 'active' AND scope IN ({scopes})",
        scope_chain,
    )
}

fn query_scoped_count(
    connection: &Connection,
    template: &str,
    scope_chain: &[String],
) -> Result<i64> {
    let sql = scoped_query(template, scope_chain.len());
    connection
        .query_row(&sql, params_from_iter(scope_chain.iter()), |row| row.get(0))
        .map_err(Into::into)
}

fn query_god_nodes(
    connection: &Connection,
    scope_chain: &[String],
    limit: usize,
) -> Result<Vec<NodeDegree>> {
    query_degree_nodes(
        connection,
        "
        SELECT e.id, e.name, e.entity_type, e.scope, COUNT(r.id) AS degree
        FROM entities e
        LEFT JOIN relations r ON r.valid_to IS NULL AND (r.from_entity = e.id OR r.to_entity = e.id)
        WHERE e.retired_at IS NULL AND e.scope IN ({scopes})
        GROUP BY e.id
        HAVING degree > 0
        ORDER BY degree DESC, e.id ASC
        LIMIT ?
        ",
        scope_chain,
        limit,
    )
}

fn query_orphan_entities(
    connection: &Connection,
    scope_chain: &[String],
    limit: usize,
) -> Result<Vec<NodeDegree>> {
    query_degree_nodes(
        connection,
        "
        SELECT e.id, e.name, e.entity_type, e.scope, COUNT(r.id) AS degree
        FROM entities e
        LEFT JOIN relations r ON r.valid_to IS NULL AND (r.from_entity = e.id OR r.to_entity = e.id)
        WHERE e.retired_at IS NULL AND e.scope IN ({scopes})
        GROUP BY e.id
        HAVING degree <= 1
        ORDER BY degree ASC, e.id ASC
        LIMIT ?
        ",
        scope_chain,
        limit,
    )
}

fn suggest_report_queries(
    entity_count: i64,
    relation_count: i64,
    decision_count: i64,
    active_session_count: i64,
    god_nodes: &[NodeDegree],
    orphan_entities: &[NodeDegree],
) -> Vec<String> {
    let mut queries = Vec::new();

    if let Some(node) = god_nodes.first() {
        queries.push(format!(
            "What decisions and observations depend on {}?",
            node.name
        ));
    }
    if let Some(node) = orphan_entities.first() {
        queries.push(format!(
            "Should {} be connected to another entity in this project?",
            node.name
        ));
    }
    if entity_count > 1 && relation_count == 0 {
        queries.push("Which entities should be related before the next handoff?".to_owned());
    }
    if decision_count > 0 {
        queries.push("Which active decisions are most likely to need revisiting?".to_owned());
    }
    if active_session_count > 0 {
        queries.push("What active work needs a clean handoff or status update?".to_owned());
    }
    if queries.is_empty() {
        queries.push("What project memory should be captured before the next session?".to_owned());
    }

    queries.truncate(5);
    queries
}

fn query_degree_nodes(
    connection: &Connection,
    template: &str,
    scope_chain: &[String],
    limit: usize,
) -> Result<Vec<NodeDegree>> {
    let sql = scoped_query(template, scope_chain.len());
    let limit = limit as i64;
    let mut params: Vec<&dyn rusqlite::ToSql> = scope_chain
        .iter()
        .map(|scope| scope as &dyn rusqlite::ToSql)
        .collect();
    params.push(&limit);
    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map(params.as_slice(), |row| {
        Ok(NodeDegree {
            id: row.get(0)?,
            name: row.get(1)?,
            entity_type: row.get(2)?,
            scope: row.get(3)?,
            degree: row.get(4)?,
        })
    })?;
    collect_rows(rows)
}

fn repeat_scope_chain(scope_chain: &[String], times: usize) -> Vec<String> {
    let mut repeated = Vec::with_capacity(scope_chain.len() * times);
    for _ in 0..times {
        repeated.extend(scope_chain.iter().cloned());
    }
    repeated
}

fn validate_import_scopes(bundle: &ExportBundle) -> Result<()> {
    Scope::new(&bundle.scope)?;
    for entity in &bundle.entities {
        Scope::new(&entity.scope)?;
    }
    for observation in &bundle.observations {
        Scope::new(&observation.scope)?;
    }
    for decision in &bundle.decisions {
        Scope::new(&decision.scope)?;
    }
    for item in &bundle.state {
        Scope::new(&item.scope)?;
    }
    for item in &bundle.context {
        Scope::new(&item.scope)?;
    }
    for session in &bundle.sessions {
        Scope::new(&session.scope)?;
    }
    Ok(())
}

fn entity_exists_in_tx(tx: &rusqlite::Transaction<'_>, entity_id: &str) -> Result<bool> {
    let exists: i64 = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM entities WHERE id = ?1 AND retired_at IS NULL)",
        [entity_id],
        |row| row.get(0),
    )?;
    Ok(exists == 1)
}

fn export_entities(connection: &Connection, scope_chain: &[String]) -> Result<Vec<GraphEntity>> {
    query_scoped_rows(
        connection,
        "
        SELECT id, name, entity_type, scope
        FROM entities
        WHERE retired_at IS NULL AND scope IN ({scopes})
        ORDER BY id ASC
        ",
        scope_chain,
        |row| {
            Ok(GraphEntity {
                id: row.get(0)?,
                name: row.get(1)?,
                entity_type: row.get(2)?,
                scope: row.get(3)?,
            })
        },
    )
}

fn export_relations(connection: &Connection, scope_chain: &[String]) -> Result<Vec<GraphRelation>> {
    let placeholders = placeholders(scope_chain.len());
    let sql = format!(
        "
        SELECT r.id, r.from_entity, r.to_entity, r.relation, r.weight, r.confidence, r.source_type, r.source
        FROM relations r
        JOIN entities f ON f.id = r.from_entity
        JOIN entities t ON t.id = r.to_entity
        WHERE r.valid_to IS NULL
          AND f.retired_at IS NULL
          AND t.retired_at IS NULL
          AND f.scope IN ({placeholders})
          AND t.scope IN ({placeholders})
        ORDER BY r.created_at ASC, r.id ASC
        "
    );
    let repeated = repeat_scope_chain(scope_chain, 2);
    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map(params_from_iter(repeated.iter()), graph_relation_from_row)?;
    collect_rows(rows)
}

fn export_observations(
    connection: &Connection,
    scope_chain: &[String],
) -> Result<Vec<ExportObservation>> {
    query_scoped_rows(
        connection,
        "
        SELECT o.id, o.entity_id, o.content, o.category, o.confidence, e.scope
        FROM observations o
        JOIN entities e ON e.id = o.entity_id
        WHERE o.valid_to IS NULL AND e.retired_at IS NULL AND e.scope IN ({scopes})
        ORDER BY o.created_at ASC, o.id ASC
        ",
        scope_chain,
        |row| {
            Ok(ExportObservation {
                id: row.get(0)?,
                entity_id: row.get(1)?,
                content: row.get(2)?,
                category: row.get(3)?,
                confidence: row.get(4)?,
                scope: row.get(5)?,
            })
        },
    )
}

fn export_decisions(
    connection: &Connection,
    scope_chain: &[String],
) -> Result<Vec<ExportDecision>> {
    query_scoped_rows(
        connection,
        "
        SELECT id, title, status, scope, reasoning, superseded_by
        FROM decisions
        WHERE status != 'revoked' AND scope IN ({scopes})
        ORDER BY created_at ASC, id ASC
        ",
        scope_chain,
        |row| {
            Ok(ExportDecision {
                id: row.get(0)?,
                title: row.get(1)?,
                status: row.get(2)?,
                scope: row.get(3)?,
                reasoning: row.get(4)?,
                superseded_by: row.get(5)?,
            })
        },
    )
}

fn export_state(connection: &Connection, scope_chain: &[String]) -> Result<Vec<StateItem>> {
    query_scoped_rows(
        connection,
        "
        SELECT key, title, status, priority, owner, scope, details, blockers, depends_on
        FROM state
        WHERE status != 'abandoned' AND scope IN ({scopes})
        ORDER BY updated_at ASC, key ASC
        ",
        scope_chain,
        state_item_from_row,
    )
}

fn export_context(connection: &Connection, scope_chain: &[String]) -> Result<Vec<ExportContext>> {
    query_scoped_rows(
        connection,
        "
        SELECT key, title, category, scope, version, content
        FROM context
        WHERE retired_at IS NULL AND scope IN ({scopes})
        ORDER BY updated_at ASC, key ASC
        ",
        scope_chain,
        |row| {
            Ok(ExportContext {
                key: row.get(0)?,
                title: row.get(1)?,
                category: row.get(2)?,
                scope: row.get(3)?,
                version: row.get(4)?,
                content: row.get(5)?,
            })
        },
    )
}

fn export_sessions(
    connection: &Connection,
    project: &str,
    scope_chain: &[String],
) -> Result<Vec<SessionLogItem>> {
    let sql = scoped_query(
        "
        SELECT id, session_type, status, scope, goal, summary, accomplishments, remaining,
               files_changed, decisions_made, entities_touched, handoff_context,
               parent_session, child_session, started_at, ended_at
        FROM sessions
        WHERE project = ? AND scope IN ({scopes})
        ORDER BY started_at ASC, id ASC
        ",
        scope_chain.len(),
    );
    let mut params: Vec<&dyn rusqlite::ToSql> = vec![&project];
    params.extend(
        scope_chain
            .iter()
            .map(|scope| scope as &dyn rusqlite::ToSql),
    );
    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map(params.as_slice(), session_log_item_from_row)?;
    collect_rows(rows)
}

fn query_search_rows<T, F>(
    connection: &Connection,
    sql: &str,
    query: &str,
    scope_chain: &[String],
    limit: usize,
    mapper: F,
) -> Result<Vec<T>>
where
    F: FnMut(&Row<'_>) -> rusqlite::Result<T>,
{
    let mut params: Vec<&dyn rusqlite::ToSql> = vec![&query];
    params.extend(
        scope_chain
            .iter()
            .map(|scope| scope as &dyn rusqlite::ToSql),
    );
    let limit = limit as i64;
    params.push(&limit);

    let mut statement = connection.prepare(sql)?;
    let rows = statement.query_map(params.as_slice(), mapper)?;
    collect_rows(rows)
}

fn status_active_sessions(connection: &Connection, scope_chain: &[String]) -> Result<Vec<String>> {
    query_scoped_rows(
        connection,
        "
        SELECT id, session_type, goal, scope
        FROM sessions
        WHERE status = 'active' AND scope IN ({scopes})
        ORDER BY started_at DESC
        LIMIT 10
        ",
        scope_chain,
        |row| {
            let id: String = row.get(0)?;
            let session_type: String = row.get(1)?;
            let goal: Option<String> = row.get(2)?;
            let scope: String = row.get(3)?;
            Ok(format!(
                "[session:{id}] {session_type}: {} [{}]",
                goal.unwrap_or_else(|| "No goal".to_owned()),
                display_scope(&scope)
            ))
        },
    )
}

fn status_active_state(connection: &Connection, scope_chain: &[String]) -> Result<Vec<String>> {
    query_scoped_rows(
        connection,
        "
        SELECT key, title, status, priority, scope
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
            let scope: String = row.get(4)?;
            Ok(format!(
                "[state:{key}] {title} ({status}, {priority}) [{}]",
                display_scope(&scope)
            ))
        },
    )
}

fn status_recent_decisions(connection: &Connection, scope_chain: &[String]) -> Result<Vec<String>> {
    query_scoped_rows(
        connection,
        "
        SELECT id, title, status, scope
        FROM decisions
        WHERE status NOT IN ('revoked', 'superseded') AND scope IN ({scopes})
        ORDER BY created_at DESC
        LIMIT 10
        ",
        scope_chain,
        |row| {
            let id: String = row.get(0)?;
            let title: String = row.get(1)?;
            let status: String = row.get(2)?;
            let scope: String = row.get(3)?;
            Ok(format!(
                "[decision:{id}] {title} ({status}) [{}]",
                display_scope(&scope)
            ))
        },
    )
}

fn status_recent_events(connection: &Connection, scope_chain: &[String]) -> Result<Vec<String>> {
    query_scoped_rows(
        connection,
        "
        SELECT id, summary, scope
        FROM events
        WHERE scope IN ({scopes})
        ORDER BY created_at DESC
        LIMIT 10
        ",
        scope_chain,
        |row| {
            let id: String = row.get(0)?;
            let summary: String = row.get(1)?;
            let scope: String = row.get(2)?;
            Ok(format!(
                "[event:{id}] {summary} [{}]",
                display_scope(&scope)
            ))
        },
    )
}

fn display_scope(scope: &str) -> &str {
    if scope.is_empty() {
        "global"
    } else {
        scope
    }
}

fn detail_metadata(label: &str, value: impl Into<String>) -> DetailMetadata {
    DetailMetadata {
        label: label.to_owned(),
        value: value.into(),
    }
}

fn detail_events(
    project_name: Option<String>,
    start_dir: PathBuf,
    grafiki_home: Option<PathBuf>,
    scope: &str,
    record_type: &str,
    id: &str,
) -> Vec<DetailEvent> {
    list_events(EventListOptions {
        project_name,
        start_dir,
        grafiki_home,
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
                    || normalize_memory_record_type(&event.target_type)
                        .map(|target_type| target_type == record_type && event.summary.contains(id))
                        .unwrap_or(false)
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

fn related_for_entity(bundle: &ExportBundle, entity_id: &str) -> Vec<RelatedMemoryRecord> {
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

fn entity_title(bundle: &ExportBundle, entity_id: &str) -> String {
    bundle
        .entities
        .iter()
        .find(|entity| entity.id == entity_id)
        .map(|entity| entity.name.clone())
        .unwrap_or_else(|| entity_id.to_owned())
}

#[cfg(test)]
mod tests {
    use crate::project::{init_project, InitOptions};
    use crate::session::{start_session, StartSessionOptions};

    use super::{
        add_context, approve_candidate, ask_memory, bulk_review_candidates, capture_ledger, chat,
        chat_with_provider, delete_context, delete_decision, delete_entity, delete_observation,
        delete_relation, delete_state, edit_candidate, end_session, export_memory,
        extract_capture_memory, generate_report, get_context, get_embedding_status, get_graph,
        get_status, handoff_session, hybrid_search_results, import_memory, ingest_capture_event,
        list_candidates, list_capture_events, list_context, list_decisions, list_entities,
        list_events, list_observations, list_relations, list_sessions, list_state, log_decision,
        pending_embedding_count, process_embedding_jobs, propose_candidate, redact_sensitive_text,
        reject_candidate, reopen_candidate, resolve_and_open, revert_candidate_approval,
        run_capture_watch, save_entity, search_memory, start_capture_session, update_context,
        update_decision, update_entity, update_observation, update_relation, update_session,
        upsert_state, AddContextOptions, ApproveCandidateOptions, AskMemoryOptions,
        BulkCandidateReviewOptions, CandidateOrder, CaptureLedgerOptions, ChatOptions,
        ContextListOptions, DecisionListOptions, DeleteContextOptions, DeleteDecisionOptions,
        DeleteEntityOptions, DeleteObservationOptions, DeleteRelationOptions, DeleteStateOptions,
        EditCandidateOptions, EmbeddingStatusOptions, EndSessionOptions, EventListOptions,
        EvidenceInput, ExportOptions, ExtractCaptureOptions, ExtractionCandidate,
        GetContextOptions, GraphOptions, HandoffOptions, ImportOptions, IngestCaptureEventOptions,
        ListCandidatesOptions, ListCaptureEventsOptions, LogDecisionOptions,
        ObservationListOptions, ProcessEmbeddingsOptions, ProjectReportOptions,
        ProposeCandidateOptions, RejectCandidateOptions, RelationListOptions,
        ReopenCandidateOptions, RevertApprovalOptions, RunCaptureWatchOptions, SaveEntityOptions,
        SearchMemoryOptions, SearchMode, SearchReport, SearchResult, SessionLogOptions,
        StartCaptureOptions, StateListOptions, StatusOptions, UpdateContextOptions,
        UpdateDecisionOptions, UpdateEntityOptions, UpdateObservationOptions,
        UpdateRelationOptions, UpdateSessionOptions, UpsertStateOptions, MAX_BODY_CHARS,
        MAX_TITLE_CHARS, TRUNCATION_MARK,
    };

    fn setup_project() -> (tempfile::TempDir, std::path::PathBuf, std::path::PathBuf) {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().join("home");
        let project_dir = temp.path().join("example-project");
        init_project(InitOptions {
            project_name: None,
            project_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
        })
        .unwrap();
        (temp, home, project_dir)
    }

    #[test]
    fn reflection_cohesion_confidence_and_force_respects_context_backstop() {
        use super::run_reflection;
        use crate::reflection::RunReflectionOptions;

        let (_t, home, dir) = setup_project();
        let scope = "core";
        let save = |name: &str, ty: &str, text: &str, relate: Option<&str>| {
            save_entity(SaveEntityOptions {
                project_name: None,
                start_dir: dir.clone(),
                grafiki_home: Some(home.clone()),
                name: name.to_string(),
                entity_type: ty.to_string(),
                observe: Some(text.to_string()),
                category: "architecture".to_string(),
                scope: scope.to_string(),
                relate: relate.map(str::to_string),
            })
            .unwrap();
        };
        // A triangle (the whole graph) → one community whose modularity contribution is 0.
        save("Auth Service", "service", "Auth throttles requests.", None);
        save(
            "JWT Library",
            "library",
            "Tokens rotate often.",
            Some("auth-service:uses"),
        );
        save(
            "Refresh Token Cache",
            "module",
            "Token reuse is rejected.",
            Some("auth-service:uses"),
        );
        save(
            "JWT Library",
            "library",
            "Tokens are stored hashed.",
            Some("refresh-token-cache:uses"),
        );

        let opts = |force: bool| {
            let mut o = RunReflectionOptions::new(scope, dir.clone());
            o.grafiki_home = Some(home.clone());
            o.force = force;
            o
        };

        let first = run_reflection(opts(false)).unwrap();
        assert_eq!(first.candidates_created, 1, "one community ⇒ one candidate");
        let created = first
            .details
            .iter()
            .find(|d| d.status == "created")
            .unwrap();
        let candidate_id = created.candidate_id.clone().unwrap();

        // Cohesion discount: a whole-graph community has Q_c ≈ 0, so confidence is
        // mean(obs confidence = 1.0) × cohesion(0.5) = 0.5 — NOT the un-discounted 1.0
        // the old floor-only formula produced.
        let pending = list_candidates(ListCandidatesOptions {
            project_name: None,
            start_dir: dir.clone(),
            grafiki_home: Some(home.clone()),
            status: Some("pending".to_string()),
            scope: scope.to_string(),
            limit: 50,
            order: CandidateOrder::Recent,
        })
        .unwrap();
        let candidate = pending.iter().find(|c| c.id == candidate_id).unwrap();
        assert!(
            candidate.confidence <= 0.6,
            "cohesion must discount a loosely-connected community: {}",
            candidate.confidence
        );

        // Approve → a trusted `context` row with this key now exists.
        approve_candidate(ApproveCandidateOptions {
            project_name: None,
            start_dir: dir.clone(),
            grafiki_home: Some(home.clone()),
            id: candidate_id,
        })
        .unwrap();

        // --force bypasses the candidate-table dedup but MUST still honor the
        // context.key backstop, so it proposes nothing that would collide at approval.
        let forced = run_reflection(opts(true)).unwrap();
        assert_eq!(
            forced.candidates_created, 0,
            "--force must not bypass the context.key existence backstop"
        );
        assert_eq!(forced.skipped_existing, 1);
    }

    // --- M-E1/M-E2 temporal boost (decay + salience) -----------------------

    fn save_obs(home: &std::path::Path, dir: &std::path::Path, name: &str, text: &str) -> String {
        save_entity(SaveEntityOptions {
            project_name: None,
            start_dir: dir.to_path_buf(),
            grafiki_home: Some(home.to_path_buf()),
            name: name.to_string(),
            entity_type: "concept".to_string(),
            observe: Some(text.to_string()),
            category: "general".to_string(),
            scope: "core".to_string(),
            relate: None,
        })
        .unwrap()
        .observation_id
        .unwrap()
    }

    fn graph_search_ids(
        home: &std::path::Path,
        dir: &std::path::Path,
        query: &str,
        temporal_weight: f64,
    ) -> Vec<String> {
        // Graph mode is the model-free fused path: it always routes through
        // `hybrid_search_results` (where the temporal boost applies), even with no
        // semantic vectors or relations.
        search_memory(SearchMemoryOptions {
            project_name: None,
            start_dir: dir.to_path_buf(),
            grafiki_home: Some(home.to_path_buf()),
            query: query.to_string(),
            record_type: "all".to_string(),
            mode: SearchMode::Graph,
            scope: "core".to_string(),
            limit: 10,
            temporal_weight,
        })
        .unwrap()
        .results
        .into_iter()
        .map(|r| r.id)
        .collect()
    }

    #[test]
    fn temporal_weight_promotes_recent_over_stale() {
        let (_t, home, dir) = setup_project();
        // Lexically IDENTICAL observations on distinct entities ⇒ equal lexical score, so the
        // baseline order is the deterministic insertion tiebreak (old first) and recency is the
        // ONLY differentiator once the temporal weight is applied.
        let old_id = save_obs(
            &home,
            &dir,
            "Alpha Cache",
            "Cache layer uses in-memory storage for sessions.",
        );
        let new_id = save_obs(
            &home,
            &dir,
            "Bravo Cache",
            "Cache layer uses in-memory storage for sessions.",
        );
        {
            let (_p, conn) = resolve_and_open(None, dir.clone(), Some(home.clone())).unwrap();
            conn.execute(
                "UPDATE observations SET valid_from = '2025-01-01T00:00:00Z' WHERE id = ?1",
                [&old_id],
            )
            .unwrap();
        }

        let baseline = graph_search_ids(&home, &dir, "cache layer storage sessions", 0.0);
        let boosted = graph_search_ids(&home, &dir, "cache layer storage sessions", 5.0);
        let pos = |v: &[String], id: &str| v.iter().position(|x| x == id);

        assert!(
            pos(&baseline, &old_id) < pos(&baseline, &new_id),
            "baseline (no temporal weight) ranks by insertion tiebreak — older first: {baseline:?}"
        );
        assert!(
            pos(&boosted, &new_id) < pos(&boosted, &old_id),
            "temporal weight must flip the order, promoting the recent record: {boosted:?}"
        );
        // Deterministic.
        assert_eq!(
            boosted,
            graph_search_ids(&home, &dir, "cache layer storage sessions", 5.0)
        );
    }

    #[test]
    fn temporal_weight_promotes_reused_record() {
        let (_t, home, dir) = setup_project();
        // Equally-fresh, equally-matching observations. `cold` is saved FIRST so it owns the
        // deterministic baseline tiebreak (smaller ULID); only `reused` (saved second) gets
        // audit-log reuse. Recency is equal, so a flip can only come from salience.
        let cold_id = save_obs(
            &home,
            &dir,
            "Beta Deploy",
            "Deployment pipeline runs on staging.",
        );
        let reused_id = save_obs(
            &home,
            &dir,
            "Alpha Deploy",
            "Deployment pipeline runs on staging.",
        );
        {
            let (_p, conn) = resolve_and_open(None, dir.clone(), Some(home.clone())).unwrap();
            let returned =
                serde_json::to_string(&vec![format!("observation:{reused_id}")]).unwrap();
            for _ in 0..5 {
                conn.execute(
                    "INSERT INTO agent_queries (id, agent, question, scope, returned_ids, retrieval_mode) \
                     VALUES (?1, 'eval', 'deployment', 'core', ?2, 'graph')",
                    rusqlite::params![super::new_ulid(), returned],
                )
                .unwrap();
            }
        }

        let pos = |v: &[String], id: &str| v.iter().position(|x| x == id);
        // Baseline (no weight): the first-saved `cold` wins the tiebreak — reuse must overcome it.
        let baseline = graph_search_ids(&home, &dir, "deployment pipeline staging", 0.0);
        assert!(
            pos(&baseline, &cold_id) < pos(&baseline, &reused_id),
            "baseline tiebreak should rank the first-saved (cold) record ahead: {baseline:?}"
        );
        // With weight: audit-log reuse must flip the order, promoting the reused record.
        let boosted = graph_search_ids(&home, &dir, "deployment pipeline staging", 5.0);
        assert!(
            pos(&boosted, &reused_id) < pos(&boosted, &cold_id),
            "audit-log reuse should promote the reused record over the cold one: {boosted:?}"
        );
    }

    // --- M-E3 calibrated confidence + active-learning review order -----------

    #[test]
    fn active_learning_order_and_calibrated_confidence() {
        let (_t, home, dir) = setup_project();
        let scope = "core";
        let propose = |source_type: &str, content: &str, evidence: Vec<EvidenceInput>| -> String {
            propose_candidate(ProposeCandidateOptions {
                project_name: None,
                start_dir: dir.clone(),
                grafiki_home: Some(home.clone()),
                source_type: source_type.to_string(),
                source: None,
                record_type: "observation".to_string(),
                payload: serde_json::json!({
                    "entity_name": "Subject",
                    "content": content,
                    "category": "general",
                }),
                scope: scope.to_string(),
                confidence: 0.5,
                rationale: None,
                evidence,
            })
            .unwrap()
            .candidate
            .id
        };
        let ev = || EvidenceInput {
            source_event_id: None,
            source_type: "test".to_string(),
            source: None,
            title: None,
            excerpt: "corroborating excerpt".to_string(),
            uri: None,
            byte_start: None,
            byte_end: None,
            line_start: None,
            line_end: None,
            captured_at: None,
        };

        // Proposed in DESCENDING priority order (evidenced inferred fact, then tool, then the
        // confident user fact LAST) — so recency (newest-first) is the REVERSE of the
        // active-learning order, making any coincidence between the two impossible.
        let _evd_id = propose("inferred", "inferred from context", vec![ev(), ev()]);
        let tool_id = propose("tool", "tool reported value", Vec::new());
        let user_id = propose("user", "user stated preference", Vec::new());

        let list = |order: CandidateOrder| -> Vec<ExtractionCandidate> {
            list_candidates(ListCandidatesOptions {
                project_name: None,
                start_dir: dir.clone(),
                grafiki_home: Some(home.clone()),
                status: Some("pending".to_string()),
                scope: scope.to_string(),
                limit: 50,
                order,
            })
            .unwrap()
        };

        let recent = list(CandidateOrder::Recent);
        let active = list(CandidateOrder::ActiveLearning);
        let find =
            |v: &[ExtractionCandidate], id: &str| v.iter().find(|c| c.id == id).cloned().unwrap();

        // Calibration: trusted source ⇒ higher calibrated confidence than a tool fact.
        let user = find(&recent, &user_id);
        let tool = find(&recent, &tool_id);
        assert!(
            user.calibrated_confidence > tool.calibrated_confidence,
            "user {} should outrank tool {}",
            user.calibrated_confidence,
            tool.calibrated_confidence
        );
        assert!(
            (tool.calibrated_confidence - 0.5).abs() < 1e-9,
            "tool prior 0.5 with no evidence"
        );

        // Active learning: the confident, uncorroborated user fact is the LEAST informative ⇒ last.
        let pos = |v: &[ExtractionCandidate], id: &str| v.iter().position(|c| c.id == id).unwrap();
        assert_eq!(
            pos(&active, &user_id),
            active.len() - 1,
            "the confident user fact should be reviewed last under active learning"
        );
        // And the ordering must actually differ from plain recency.
        assert_ne!(
            recent.iter().map(|c| c.id.clone()).collect::<Vec<_>>(),
            active.iter().map(|c| c.id.clone()).collect::<Vec<_>>(),
            "active-learning order should differ from recency"
        );
        // Deterministic.
        assert_eq!(
            active.iter().map(|c| c.id.clone()).collect::<Vec<_>>(),
            list(CandidateOrder::ActiveLearning)
                .iter()
                .map(|c| c.id.clone())
                .collect::<Vec<_>>()
        );
    }

    fn start_codex(home: std::path::PathBuf, project_dir: std::path::PathBuf) -> String {
        start_session(StartSessionOptions {
            project_name: None,
            start_dir: project_dir,
            grafiki_home: Some(home),
            session_type: "codex".to_owned(),
            goal: "Work on memory loop".to_owned(),
            scope: "example-project/core".to_owned(),
        })
        .unwrap()
        .session_id
    }

    #[test]
    fn hybrid_search_rewards_cross_source_overlap() {
        let keyword_results = vec![
            test_search_result(
                "decision",
                "wal",
                "Use SQLite WAL",
                "Local writes stay responsive",
            ),
            test_search_result(
                "context",
                "auth-guide",
                "Auth Guide",
                "Refresh token rotation prevents session replay",
            ),
        ];
        let semantic_results = vec![
            test_search_result(
                "context",
                "auth-guide",
                "Auth Guide",
                "Refresh token rotation prevents session replay",
            ),
            test_search_result(
                "observation",
                "auth-obs",
                "auth-service",
                "Rotating tokens are used during refresh",
            ),
        ];

        let results = hybrid_search_results(
            "token rotation",
            keyword_results,
            semantic_results,
            Vec::new(),
            3,
            &std::collections::HashMap::new(),
        );

        assert_eq!(results[0].id, "auth-guide");
        assert!(results[0].score.is_some());
        assert_eq!(results.len(), 3);
    }

    fn test_search_result(record_type: &str, id: &str, title: &str, snippet: &str) -> SearchResult {
        SearchResult {
            record_type: record_type.to_owned(),
            id: id.to_owned(),
            title: title.to_owned(),
            snippet: snippet.to_owned(),
            scope: "example-project/core".to_owned(),
            score: None,
            evidence: Vec::new(),
        }
    }

    #[test]
    fn retrieval_quality_fixture_keeps_topics_separated() {
        let (_temp, home, project_dir) = setup_project();
        start_codex(home.clone(), project_dir.clone());

        let auth = save_entity(SaveEntityOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            name: "Auth Service".to_owned(),
            entity_type: "service".to_owned(),
            observe: Some(
                "JWT refresh token rotation prevents replay during session renewal".to_owned(),
            ),
            category: "architecture".to_owned(),
            scope: "example-project/core".to_owned(),
            relate: None,
        })
        .unwrap();
        let storage = save_entity(SaveEntityOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            name: "Storage Layer".to_owned(),
            entity_type: "service".to_owned(),
            observe: Some(
                "SQLite WAL keeps local writes responsive while readers continue".to_owned(),
            ),
            category: "architecture".to_owned(),
            scope: "example-project/core".to_owned(),
            relate: None,
        })
        .unwrap();
        let billing = save_entity(SaveEntityOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            name: "Billing Worker".to_owned(),
            entity_type: "service".to_owned(),
            observe: Some(
                "Stripe webhook retries use idempotency keys for invoice processing".to_owned(),
            ),
            category: "architecture".to_owned(),
            scope: "example-project/core".to_owned(),
            relate: None,
        })
        .unwrap();
        let search = save_entity(SaveEntityOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            name: "Search Index".to_owned(),
            entity_type: "tool".to_owned(),
            observe: Some(
                "Embedding rebuild stores vectors for semantic and hybrid retrieval".to_owned(),
            ),
            category: "architecture".to_owned(),
            scope: "example-project/core".to_owned(),
            relate: None,
        })
        .unwrap();

        add_context(AddContextOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            key: "auth-runbook".to_owned(),
            title: "Auth Runbook".to_owned(),
            category: "runbook".to_owned(),
            scope: "example-project/core".to_owned(),
            content: "Session replay protection depends on refresh token rotation.".to_owned(),
        })
        .unwrap();
        log_decision(LogDecisionOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            title: "Use SQLite WAL for local storage".to_owned(),
            reasoning: Some("Readers must continue while local writes happen".to_owned()),
            alternatives: vec!["rollback journal".to_owned()],
            tags: vec!["storage".to_owned()],
            scope: "example-project/core".to_owned(),
            supersedes: None,
        })
        .unwrap();

        let processed = process_embedding_jobs(ProcessEmbeddingsOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            scope: "example-project/core".to_owned(),
            limit: 100,
            rebuild: true,
        })
        .unwrap();
        assert!(processed.processed >= 10);

        let status = get_embedding_status(EmbeddingStatusOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            scope: "example-project/core".to_owned(),
        })
        .unwrap();
        assert_eq!(status.runtime.missing_or_stale_records, 0);
        assert_eq!(
            status.runtime.fresh_records,
            status.runtime.embeddable_records
        );

        assert_top_results_contain(
            search_memory(SearchMemoryOptions {
                project_name: None,
                start_dir: project_dir.clone(),
                grafiki_home: Some(home.clone()),
                query: "refresh token replay".to_owned(),
                record_type: "all".to_owned(),
                mode: SearchMode::Hybrid,
                scope: "example-project/core".to_owned(),
                limit: 4,
                temporal_weight: 0.0,
            })
            .unwrap(),
            &[auth.observation_id.as_deref().unwrap(), "auth-runbook"],
        );

        assert_top_results_contain(
            search_memory(SearchMemoryOptions {
                project_name: None,
                start_dir: project_dir.clone(),
                grafiki_home: Some(home.clone()),
                query: "sqlite writes readers".to_owned(),
                record_type: "all".to_owned(),
                mode: SearchMode::Hybrid,
                scope: "example-project/core".to_owned(),
                limit: 4,
                temporal_weight: 0.0,
            })
            .unwrap(),
            &[storage.observation_id.as_deref().unwrap()],
        );

        assert_top_results_contain(
            search_memory(SearchMemoryOptions {
                project_name: None,
                start_dir: project_dir.clone(),
                grafiki_home: Some(home.clone()),
                query: "webhook idempotency invoice".to_owned(),
                record_type: "all".to_owned(),
                mode: SearchMode::Hybrid,
                scope: "example-project/core".to_owned(),
                limit: 4,
                temporal_weight: 0.0,
            })
            .unwrap(),
            &[billing.observation_id.as_deref().unwrap()],
        );

        assert_top_results_contain(
            search_memory(SearchMemoryOptions {
                project_name: None,
                start_dir: project_dir,
                grafiki_home: Some(home),
                query: "semantic vectors retrieval".to_owned(),
                record_type: "all".to_owned(),
                mode: SearchMode::Hybrid,
                scope: "example-project/core".to_owned(),
                limit: 4,
                temporal_weight: 0.0,
            })
            .unwrap(),
            &[search.observation_id.as_deref().unwrap()],
        );
    }

    fn assert_top_results_contain(report: SearchReport, expected_ids: &[&str]) {
        assert!(
            report.semantic_available,
            "semantic search should be available for {}",
            report.query
        );
        assert_eq!(report.fallback, None);
        assert!(
            report.results.iter().all(|result| result.score.is_some()),
            "ranked results should expose scores for {}",
            report.query
        );
        let result_ids: Vec<_> = report
            .results
            .iter()
            .map(|result| result.id.as_str())
            .collect();
        assert!(
            expected_ids
                .iter()
                .any(|expected_id| result_ids.contains(expected_id)),
            "expected one of {expected_ids:?} in top results for {}; got {result_ids:?}",
            report.query
        );
    }

    #[test]
    fn coding_memory_retrieval_answers_agent_questions() {
        let (_temp, home, project_dir) = setup_project();
        let session_id = start_codex(home.clone(), project_dir.clone());
        let scope = "example-project/core".to_owned();

        let storage_decision = log_decision(LogDecisionOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            title: "Use SQLite WAL for local alpha storage".to_owned(),
            reasoning: Some(
                "Keep SQLite WAL instead of a Postgres sidecar because Grafiki must stay local-first, zero setup, and responsive for readers during writes. The Postgres sidecar approach was rejected because it adds operational overhead for a solo developer install."
                    .to_owned(),
            ),
            alternatives: vec![
                "Postgres sidecar".to_owned(),
                "Hosted memory service".to_owned(),
            ],
            tags: vec!["storage".to_owned(), "local-first".to_owned()],
            scope: scope.clone(),
            supersedes: None,
        })
        .unwrap();

        let rejected_context = add_context(AddContextOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            key: "rejected-architecture-options".to_owned(),
            title: "Rejected Architecture Options".to_owned(),
            category: "architecture".to_owned(),
            scope: scope.clone(),
            content: "Do not use a hosted memory service for the launch path; cloud sync is later. Do not require a Postgres sidecar for local memory capture."
                .to_owned(),
        })
        .unwrap();

        let review_context = add_context(AddContextOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            key: "review-queue-launch".to_owned(),
            title: "Review Queue Launch Shape".to_owned(),
            category: "guide".to_owned(),
            scope: scope.clone(),
            content: "Before editing desktop review, keep the inspector hidden until the user selects evidence. Group candidates by capture source and day, then let the user approve, reject, edit, and open evidence quickly."
                .to_owned(),
        })
        .unwrap();

        let flaky_context = add_context(AddContextOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            key: "desktop-sidecar-gotcha".to_owned(),
            title: "Desktop Sidecar Smoke Gotcha".to_owned(),
            category: "runbook".to_owned(),
            scope: scope.clone(),
            content: "The installed macOS app does not show Rust or frontend changes until the debug desktop build copies the updated sidecar into /Applications/Grafiki.app."
                .to_owned(),
        })
        .unwrap();

        let handoff_context = add_context(AddContextOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            key: "last-session-handoff".to_owned(),
            title: "Last Session Handoff".to_owned(),
            category: "runbook".to_owned(),
            scope: scope.clone(),
            content: "Last session wired terminal, file, and git capture adapters plus capture consent config. Remaining work is review queue grouping and retrieval evaluation fixtures."
                .to_owned(),
        })
        .unwrap();

        upsert_state(UpsertStateOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            key: "desktop-review-focus".to_owned(),
            title: "Review Queue is launch-critical".to_owned(),
            status: "in-progress".to_owned(),
            owner: Some("grafiki".to_owned()),
            details: Some(
                "Before editing desktop, keep inspector hidden until selected and group candidates by capture source/day."
                    .to_owned(),
            ),
            blockers: Vec::new(),
            depends_on: Vec::new(),
            scope: scope.clone(),
            priority: "high".to_owned(),
        })
        .unwrap();

        end_session(EndSessionOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            session_id: Some(session_id),
            status: "completed".to_owned(),
            summary: Some("Finished capture adapters and consent config".to_owned()),
            accomplishments: vec![
                "Terminal command metadata capture".to_owned(),
                "File watcher and git summarizer".to_owned(),
            ],
            remaining: vec![
                "Review queue grouping".to_owned(),
                "Retrieval quality fixtures".to_owned(),
            ],
            files_changed: vec![
                "crates/grafiki-core/src/memory.rs".to_owned(),
                "apps/grafiki-desktop/src/App.tsx".to_owned(),
            ],
        })
        .unwrap();

        let processed = process_embedding_jobs(ProcessEmbeddingsOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            scope: scope.clone(),
            limit: 100,
            rebuild: true,
        })
        .unwrap();
        assert!(processed.processed >= 5);

        assert_top_results_contain(
            search_memory(SearchMemoryOptions {
                project_name: None,
                start_dir: project_dir.clone(),
                grafiki_home: Some(home.clone()),
                query: "why did we reject Postgres sidecar".to_owned(),
                record_type: "all".to_owned(),
                mode: SearchMode::Hybrid,
                scope: scope.clone(),
                limit: 5,
                temporal_weight: 0.0,
            })
            .unwrap(),
            &[
                storage_decision.decision_id.as_str(),
                rejected_context.key.as_str(),
            ],
        );

        assert_top_results_contain(
            search_memory(SearchMemoryOptions {
                project_name: None,
                start_dir: project_dir.clone(),
                grafiki_home: Some(home.clone()),
                query: "what should I know before editing desktop review queue".to_owned(),
                record_type: "all".to_owned(),
                mode: SearchMode::Hybrid,
                scope: scope.clone(),
                limit: 5,
                temporal_weight: 0.0,
            })
            .unwrap(),
            &[review_context.key.as_str()],
        );

        assert_top_results_contain(
            search_memory(SearchMemoryOptions {
                project_name: None,
                start_dir: project_dir.clone(),
                grafiki_home: Some(home.clone()),
                query: "what happened last session capture consent adapters".to_owned(),
                record_type: "all".to_owned(),
                mode: SearchMode::Hybrid,
                scope: scope.clone(),
                limit: 5,
                temporal_weight: 0.0,
            })
            .unwrap(),
            &[handoff_context.key.as_str()],
        );

        let exact_keyword = search_memory(SearchMemoryOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            query: "sidecar smoke gotcha".to_owned(),
            record_type: "context".to_owned(),
            mode: SearchMode::Keyword,
            scope: scope.clone(),
            limit: 5,
            temporal_weight: 0.0,
        })
        .unwrap();
        assert!(
            exact_keyword
                .results
                .iter()
                .any(|result| result.id == flaky_context.key),
            "expected exact keyword fallback to find {}; got {:?}",
            flaky_context.key,
            exact_keyword
                .results
                .iter()
                .map(|result| result.id.as_str())
                .collect::<Vec<_>>()
        );

        let briefing = ask_memory(AskMemoryOptions {
            project_name: None,
            start_dir: project_dir,
            grafiki_home: Some(home),
            question: "What approach was rejected for local storage, and what should I know before editing desktop review?"
                .to_owned(),
            scope,
            limit: 6,
            agent: Some("codex".to_owned()),
            temporal_weight: 0.0,
        })
        .unwrap();

        assert_eq!(briefing.agent, "codex");
        assert!(briefing.semantic_available);
        assert!(
            briefing
                .active_state
                .iter()
                .any(|item| item.contains("Review Queue is launch-critical")),
            "expected active review state in briefing: {:?}",
            briefing.active_state
        );
        assert!(
            briefing.relevant_memory.iter().any(|result| {
                result.id == storage_decision.decision_id
                    || result.id == rejected_context.key
                    || result.id == review_context.key
            }),
            "expected coding memory in briefing; got {:?}",
            briefing
                .relevant_memory
                .iter()
                .map(|result| (&result.record_type, &result.id))
                .collect::<Vec<_>>()
        );
        assert!(
            briefing.answer.contains("Most relevant trusted memory"),
            "expected cited memory answer; got {}",
            briefing.answer
        );
    }

    #[test]
    fn decision_save_search_status_and_end_round_trip() {
        let (_temp, home, project_dir) = setup_project();
        let session_id = start_codex(home.clone(), project_dir.clone());

        let decision = log_decision(LogDecisionOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            title: "Use SQLite WAL".to_owned(),
            reasoning: Some("Local readers should continue while writes happen".to_owned()),
            alternatives: vec!["plain rollback journal".to_owned()],
            tags: vec!["storage".to_owned()],
            scope: "example-project/core".to_owned(),
            supersedes: None,
        })
        .unwrap();

        let saved = save_entity(SaveEntityOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            name: "Auth Service".to_owned(),
            entity_type: "service".to_owned(),
            observe: Some("JWT refresh uses rotating tokens".to_owned()),
            category: "architecture".to_owned(),
            scope: "example-project/core".to_owned(),
            relate: None,
        })
        .unwrap();

        let search = search_memory(SearchMemoryOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            query: "rotating".to_owned(),
            record_type: "all".to_owned(),
            mode: SearchMode::Keyword,
            scope: "example-project/core".to_owned(),
            limit: 10,
            temporal_weight: 0.0,
        })
        .unwrap();

        let semantic_search = search_memory(SearchMemoryOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            query: "rotating".to_owned(),
            record_type: "all".to_owned(),
            mode: SearchMode::Semantic,
            scope: "example-project/core".to_owned(),
            limit: 10,
            temporal_weight: 0.0,
        })
        .unwrap();

        let (_project, connection) =
            resolve_and_open(None, project_dir.clone(), Some(home.clone())).unwrap();
        let pending_embedding_jobs: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM embedding_jobs WHERE status = 'pending'",
                [],
                |row| row.get(0),
            )
            .unwrap();

        let processed_embeddings = process_embedding_jobs(ProcessEmbeddingsOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            scope: "example-project/core".to_owned(),
            limit: 10,
            rebuild: false,
        })
        .unwrap();

        let semantic_after_processing = search_memory(SearchMemoryOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            query: "rotating".to_owned(),
            record_type: "all".to_owned(),
            mode: SearchMode::Semantic,
            scope: "example-project/core".to_owned(),
            limit: 10,
            temporal_weight: 0.0,
        })
        .unwrap();

        #[cfg(feature = "sqlite-vec")]
        {
            let (_project, connection) =
                resolve_and_open(None, project_dir.clone(), Some(home.clone())).unwrap();
            let vector_indexes: i64 = connection
                .query_row(
                    "
                    SELECT COUNT(*)
                    FROM sqlite_schema
                    WHERE type = 'table'
                      AND name LIKE 'embedding_vec_%'
                      AND sql LIKE '%USING vec0%'
                    ",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(vector_indexes, 1);
        }

        let status = get_status(StatusOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            scope: "example-project/core".to_owned(),
        })
        .unwrap();

        let ended = end_session(EndSessionOptions {
            project_name: None,
            start_dir: project_dir,
            grafiki_home: Some(home),
            session_id: Some(session_id.clone()),
            status: "completed".to_owned(),
            summary: Some("Finished the memory loop".to_owned()),
            accomplishments: vec!["Added commands".to_owned()],
            remaining: vec!["Handoff".to_owned()],
            files_changed: vec!["src/main.rs".to_owned()],
        })
        .unwrap();

        assert_eq!(ended.session_id, session_id);
        assert_eq!(decision.title, "Use SQLite WAL");
        assert_eq!(saved.entity_id, "auth-service");
        assert_eq!(search.results.len(), 1);
        assert_eq!(search.mode, SearchMode::Keyword);
        assert!(!search.semantic_available);
        assert_eq!(search.fallback, None);
        assert_eq!(semantic_search.results.len(), 1);
        assert_eq!(
            semantic_search.fallback.as_deref(),
            Some("Semantic search has no indexed vectors yet; returned keyword results. Run `grafiki embeddings rebuild`.")
        );
        assert_eq!(pending_embedding_jobs, 3);
        assert_eq!(processed_embeddings.processed, 3);
        assert_eq!(processed_embeddings.pending_remaining, 0);
        assert!(semantic_after_processing.semantic_available);
        assert_eq!(semantic_after_processing.fallback, None);
        assert!(!semantic_after_processing.results.is_empty());
        assert!(semantic_after_processing.results[0].score.is_some());
        assert!(status
            .active_sessions
            .iter()
            .any(|item| item.contains(&session_id)));
    }

    #[test]
    fn editable_records_round_trip() {
        let (_temp, home, project_dir) = setup_project();
        start_codex(home.clone(), project_dir.clone());

        let decision = log_decision(LogDecisionOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            title: "Use the desktop detail editor".to_owned(),
            reasoning: Some("Memory needs correction, not only capture.".to_owned()),
            alternatives: Vec::new(),
            tags: vec!["desktop".to_owned()],
            scope: "example-project/desktop".to_owned(),
            supersedes: None,
        })
        .unwrap();
        let saved = save_entity(SaveEntityOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            name: "Memory Browser".to_owned(),
            entity_type: "module".to_owned(),
            observe: Some("Shows editable memory records".to_owned()),
            category: "progress".to_owned(),
            scope: "example-project/desktop".to_owned(),
            relate: None,
        })
        .unwrap();

        let decisions = list_decisions(DecisionListOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            scope: "example-project/desktop".to_owned(),
            status: None,
        })
        .unwrap();
        assert_eq!(decisions.len(), 1);

        let updated_decision = update_decision(UpdateDecisionOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            id: decision.decision_id.clone(),
            title: Some("Use the maintained detail editor".to_owned()),
            reasoning: Some("Memory needs correction and deletion.".to_owned()),
            scope: None,
            status: Some("revisit".to_owned()),
        })
        .unwrap();
        assert_eq!(updated_decision.status, "revisit");

        let updated_entity = update_entity(UpdateEntityOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            id: saved.entity_id.clone(),
            name: Some("Memory Console".to_owned()),
            entity_type: Some("module".to_owned()),
            scope: Some("example-project/desktop".to_owned()),
        })
        .unwrap();
        assert_eq!(updated_entity.name, "Memory Console");

        let observation_id = saved.observation_id.clone().unwrap();
        let updated_observation = update_observation(UpdateObservationOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            id: observation_id.clone(),
            content: Some("Shows editable and removable memory records".to_owned()),
            category: Some("learned".to_owned()),
        })
        .unwrap();
        assert_eq!(updated_observation.category, "learned");

        let observations = list_observations(ObservationListOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            scope: "example-project/desktop".to_owned(),
            category: Some("learned".to_owned()),
        })
        .unwrap();
        assert_eq!(observations.len(), 1);

        let deleted_observation = delete_observation(DeleteObservationOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            id: observation_id,
        })
        .unwrap();
        assert_eq!(deleted_observation.entity_id, saved.entity_id);
        assert!(list_observations(ObservationListOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            scope: "example-project/desktop".to_owned(),
            category: None,
        })
        .unwrap()
        .is_empty());

        let deleted_decision = delete_decision(DeleteDecisionOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            id: decision.decision_id,
        })
        .unwrap();
        assert_eq!(deleted_decision.title, "Use the maintained detail editor");

        let deleted_entity = delete_entity(DeleteEntityOptions {
            project_name: None,
            start_dir: project_dir,
            grafiki_home: Some(home),
            id: saved.entity_id,
        })
        .unwrap();
        assert_eq!(deleted_entity.name, "Memory Console");
    }

    #[test]
    fn handoff_creates_child_session_and_context() {
        let (_temp, home, project_dir) = setup_project();
        let session_id = start_codex(home.clone(), project_dir.clone());

        let handoff = handoff_session(HandoffOptions {
            project_name: None,
            start_dir: project_dir,
            grafiki_home: Some(home),
            session_id: Some(session_id.clone()),
        })
        .unwrap();

        assert_eq!(handoff.parent_session_id, session_id);
        assert_ne!(handoff.child_session_id, handoff.parent_session_id);
        assert!(handoff.handoff_context.contains("Grafiki Handoff"));
        assert!(handoff.handoff_context.contains("Work on memory loop"));
    }

    #[test]
    fn context_crud_round_trip() {
        let (_temp, home, project_dir) = setup_project();
        start_codex(home.clone(), project_dir.clone());

        let added = add_context(AddContextOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            key: "phase1-prd".to_owned(),
            title: "Phase 1 PRD".to_owned(),
            category: "spec".to_owned(),
            scope: "example-project/core".to_owned(),
            content: "Phase 1 ships the memory loop.".to_owned(),
        })
        .unwrap();

        let listed = list_context(ContextListOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            category: Some("spec".to_owned()),
            scope: "example-project/core".to_owned(),
        })
        .unwrap();

        let shown = get_context(GetContextOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            key: "phase1-prd".to_owned(),
        })
        .unwrap();

        let updated = update_context(UpdateContextOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            key: "phase1-prd".to_owned(),
            title: None,
            category: None,
            scope: None,
            content: Some("Phase 1 ships init, start, memory, handoff, and context.".to_owned()),
        })
        .unwrap();

        let search = search_memory(SearchMemoryOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            query: "handoff".to_owned(),
            record_type: "context".to_owned(),
            mode: SearchMode::Keyword,
            scope: "example-project/core".to_owned(),
            limit: 10,
            temporal_weight: 0.0,
        })
        .unwrap();

        let deleted = delete_context(DeleteContextOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            key: "phase1-prd".to_owned(),
        })
        .unwrap();

        assert_eq!(added.version, 1);
        assert_eq!(listed.len(), 1);
        assert_eq!(shown.title, "Phase 1 PRD");
        assert_eq!(updated.version, 2);
        assert_eq!(search.results.len(), 1);
        assert_eq!(deleted.key, "phase1-prd");
        assert!(get_context(GetContextOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            key: "phase1-prd".to_owned(),
        })
        .is_err());
        let (_project, connection) = resolve_and_open(None, project_dir, Some(home)).unwrap();
        let retired: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM context WHERE key = 'phase1-prd' AND retired_at IS NOT NULL",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(retired, 1, "delete must preserve the retired history row");
    }

    #[test]
    fn add_context_upserts_existing_key() {
        let (_temp, home, project_dir) = setup_project();
        start_codex(home.clone(), project_dir.clone());

        let add = |title: &str, content: &str| {
            add_context(AddContextOptions {
                project_name: None,
                start_dir: project_dir.clone(),
                grafiki_home: Some(home.clone()),
                key: "deploy-runbook".to_owned(),
                title: title.to_owned(),
                category: "runbook".to_owned(),
                scope: "example-project/core".to_owned(),
                content: content.to_owned(),
            })
        };

        // First add → version 1.
        let first = add("Deploy", "Deploy via CI.").unwrap();
        assert_eq!(first.version, 1);

        // Re-adding the SAME key must UPSERT (bump version), not fail on UNIQUE.
        let second = add("Deploy v2", "Deploy via CI, then verify health.").unwrap();
        assert_eq!(
            second.version, 2,
            "re-adding an existing key must bump version"
        );

        // Exactly one row remains, carrying the updated content.
        let listed = list_context(ContextListOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            category: Some("runbook".to_owned()),
            scope: "example-project/core".to_owned(),
        })
        .unwrap();
        assert_eq!(listed.len(), 1, "upsert must not create a duplicate row");

        let shown = get_context(GetContextOptions {
            project_name: None,
            start_dir: project_dir,
            grafiki_home: Some(home),
            key: "deploy-runbook".to_owned(),
        })
        .unwrap();
        assert_eq!(shown.title, "Deploy v2");
        assert!(shown.content.contains("verify health"));
    }

    #[test]
    fn chat_is_grounded_and_cited_or_abstains() {
        let (_temp, home, project_dir) = setup_project();
        let scope = "example-project/core";

        save_entity(SaveEntityOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            name: "Deploy Target".to_owned(),
            entity_type: "service".to_owned(),
            observe: Some("We deploy to GCP europe-west1".to_owned()),
            category: "architecture".to_owned(),
            scope: scope.to_owned(),
            relate: None,
        })
        .unwrap();

        // A question the memory can answer: grounded answer + a citation.
        let reply = chat(ChatOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            question: "where do we deploy".to_owned(),
            scope: scope.to_owned(),
            limit: 8,
            temporal_weight: 0.0,
        })
        .unwrap();
        assert!(reply.used_memory, "should answer from memory");
        assert!(
            reply.answer.contains("europe-west1"),
            "answer must be grounded in the retrieved memory: {}",
            reply.answer
        );
        assert!(
            reply
                .citations
                .iter()
                .any(|c| c.snippet.contains("europe-west1")),
            "the answer must cite the memory it used"
        );

        // A question the memory cannot answer: ABSTAIN, never fabricate.
        let empty = chat(ChatOptions {
            project_name: None,
            start_dir: project_dir,
            grafiki_home: Some(home),
            question: "what is the airspeed velocity of an unladen swallow".to_owned(),
            scope: scope.to_owned(),
            limit: 8,
            temporal_weight: 0.0,
        })
        .unwrap();
        assert!(!empty.used_memory);
        assert!(empty.citations.is_empty());
        assert_eq!(empty.answer, crate::chat::NO_MEMORY_ANSWER);
    }

    #[test]
    fn chat_rejects_fabricated_citations_and_attaches_only_cited() {
        use crate::chat::{ChatProvider, GroundedMemory};

        let (_temp, home, project_dir) = setup_project();
        let scope = "example-project/core";
        for (name, obs) in [
            ("Deploy Target", "We deploy to GCP europe-west1"),
            ("CI Runner", "CI runs on self-hosted linux workers"),
        ] {
            save_entity(SaveEntityOptions {
                project_name: None,
                start_dir: project_dir.clone(),
                grafiki_home: Some(home.clone()),
                name: name.to_owned(),
                entity_type: "service".to_owned(),
                observe: Some(obs.to_owned()),
                category: "architecture".to_owned(),
                scope: scope.to_owned(),
                relate: None,
            })
            .unwrap();
        }

        // A model that cites a source it was never given ([99]) = fabrication.
        struct Fabricator;
        impl ChatProvider for Fabricator {
            fn generate(&self, _q: &str, _m: &[GroundedMemory]) -> crate::Result<String> {
                Ok("We deploy to the moon [99].".to_owned())
            }
            fn judges_relevance(&self) -> bool {
                true
            }
        }
        let fabricated = chat_with_provider(
            ChatOptions {
                project_name: None,
                start_dir: project_dir.clone(),
                grafiki_home: Some(home.clone()),
                question: "where do we deploy".to_owned(),
                scope: scope.to_owned(),
                limit: 8,
                temporal_weight: 0.0,
            },
            &Fabricator,
        )
        .unwrap();
        assert!(
            !fabricated.used_memory && fabricated.citations.is_empty(),
            "a fabricated [99] citation must be rejected, not presented as grounded"
        );
        assert_eq!(fabricated.answer, crate::chat::NO_MEMORY_ANSWER);

        // A model that cites exactly [1] must get ONLY that one citation back,
        // not every retrieved memory.
        struct CitesFirst;
        impl ChatProvider for CitesFirst {
            fn generate(&self, _q: &str, _m: &[GroundedMemory]) -> crate::Result<String> {
                Ok("We deploy to GCP europe-west1 [1].".to_owned())
            }
            fn judges_relevance(&self) -> bool {
                true
            }
        }
        let cited = chat_with_provider(
            ChatOptions {
                project_name: None,
                start_dir: project_dir,
                grafiki_home: Some(home),
                question: "where do we deploy".to_owned(),
                scope: scope.to_owned(),
                limit: 8,
                temporal_weight: 0.0,
            },
            &CitesFirst,
        )
        .unwrap();
        assert!(cited.used_memory);
        assert_eq!(
            cited.citations.len(),
            1,
            "only the cited memory should be attached, not all retrieved ones"
        );
        assert_eq!(cited.citations[0].index, 1);
    }

    #[test]
    fn superseded_decisions_are_excluded_from_retrieval() {
        let (_temp, home, project_dir) = setup_project();
        let scope = "example-project/core";
        let first = log_decision(LogDecisionOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            title: "Use REST for the public API".to_owned(),
            reasoning: Some("Simplest for early clients".to_owned()),
            alternatives: vec![],
            tags: vec![],
            scope: scope.to_owned(),
            supersedes: None,
        })
        .unwrap();
        log_decision(LogDecisionOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            title: "Use gRPC for the public API".to_owned(),
            reasoning: Some("Stronger typing across services".to_owned()),
            alternatives: vec![],
            tags: vec![],
            scope: scope.to_owned(),
            supersedes: Some(first.decision_id.clone()),
        })
        .unwrap();

        // Keyword search for the shared subject must return the CURRENT decision
        // only — not the obsolete superseded one alongside it.
        let results = search_memory(SearchMemoryOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            query: "public API".to_owned(),
            record_type: "decisions".to_owned(),
            mode: SearchMode::Keyword,
            scope: scope.to_owned(),
            limit: 10,
            temporal_weight: 0.0,
        })
        .unwrap();
        assert!(
            results.results.iter().all(|r| r.id != first.decision_id),
            "the superseded decision must not appear in search results"
        );
        assert!(
            results.results.iter().any(|r| r.title.contains("gRPC")),
            "the current decision should still be found"
        );

        // The default decision list (no explicit status filter) also excludes it.
        let listed = list_decisions(DecisionListOptions {
            project_name: None,
            start_dir: project_dir,
            grafiki_home: Some(home),
            status: None,
            scope: scope.to_owned(),
        })
        .unwrap();
        assert!(listed.iter().all(|d| d.id != first.decision_id));
    }

    #[test]
    fn extract_capture_memory_proposes_reviewable_candidates() {
        use std::io::{Read, Write};
        use std::net::TcpListener;

        let (_temp, home, project_dir) = setup_project();
        let scope = "example-project/core";

        // Capture a tiny "coding session".
        let capture = start_capture_session(StartCaptureOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            scope: scope.to_owned(),
            source_app: Some("test".to_owned()),
            consent_profile: None,
            redaction_profile: None,
        })
        .unwrap();
        ingest_capture_event(IngestCaptureEventOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            capture_id: Some(capture.capture.id.clone()),
            source_type: "terminal".to_owned(),
            source: None,
            title: Some("session".to_owned()),
            text: Some("We chose SQLite over Postgres for V1 because it's embedded.".to_owned()),
            payload: None,
            metadata: None,
            scope: scope.to_owned(),
            privacy_level: None,
            captured_at: None,
            redacted: false,
        })
        .unwrap();

        // Stand-in Ollama: return two extracted items as the model's content.
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let mut buf = [0u8; 16384];
            let _ = socket.read(&mut buf).unwrap();
            let content = "[{\"kind\":\"decision\",\"title\":\"Use SQLite\",\"content\":\"Chosen \
                 over Postgres for V1 because it is embedded.\"},{\"kind\":\"context\",\"title\":\
                 \"DB constraint\",\"content\":\"V1 excludes a server database.\"}]";
            let envelope =
                serde_json::json!({"message":{"role":"assistant","content":content}}).to_string();
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\
                 Content-Length: {}\r\nConnection: close\r\n\r\n{envelope}",
                envelope.len()
            );
            socket.write_all(response.as_bytes()).unwrap();
        });

        let report = extract_capture_memory(ExtractCaptureOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            capture_id: Some(capture.capture.id.clone()),
            scope: scope.to_owned(),
            limit: 100,
            model: None,
            ollama_url: Some(format!("http://127.0.0.1:{port}")),
            unextracted_only: false,
        })
        .unwrap();
        server.join().unwrap();

        assert_eq!(
            report.proposed, 2,
            "both extracted items should be proposed"
        );

        // They land in the REVIEW queue (propose-for-review), not trusted memory —
        // the guardrail against a wrong extraction becoming a trusted fact.
        let candidates = list_candidates(ListCandidatesOptions {
            project_name: None,
            start_dir: project_dir,
            grafiki_home: Some(home),
            status: Some("pending".to_owned()),
            scope: scope.to_owned(),
            limit: 100,
            order: CandidateOrder::Recent,
        })
        .unwrap();
        let extracted: Vec<_> = candidates
            .iter()
            .filter(|candidate| candidate.source_type == "capture:llm")
            .collect();
        assert_eq!(extracted.len(), 2);
        assert!(extracted.iter().any(|c| c.record_type == "decision"));
        assert!(extracted.iter().any(|c| c.record_type == "context"));
    }

    #[test]
    fn bare_high_entropy_secret_near_keyword_is_redacted() {
        // No prefix, no `=` assignment — the shape that previously leaked
        // while the JWT beside it was caught.
        let mut text = String::from(
            "The webhook signing key is a3f9c2e81b7d4056e9c1f80a2b6d4e37 for staging.",
        );
        assert!(redact_sensitive_text(&mut text));
        assert!(!text.contains("a3f9c2e81b7d4056e9c1f80a2b6d4e37"), "{text}");
        assert!(text.contains("[REDACTED_SECRET]"));

        // The same token WITHOUT a secret-naming word on the line survives —
        // build ids / artifact hashes are not secrets by default.
        let mut benign =
            String::from("The deployment build is a3f9c2e81b7d4056e9c1f80a2b6d4e37 for staging.");
        redact_sensitive_text(&mut benign);
        assert!(
            benign.contains("a3f9c2e81b7d4056e9c1f80a2b6d4e37"),
            "{benign}"
        );

        // Long pure-alphabetic words on a key-naming line survive (no digits).
        let mut prose = String::from(
            "The key architectural consideration is internationalizationframeworks here.",
        );
        redact_sensitive_text(&mut prose);
        assert!(prose.contains("internationalizationframeworks"), "{prose}");
    }

    #[test]
    fn oversized_fields_are_clamped_at_the_write_boundary() {
        let (_temp, home, project_dir) = setup_project();
        // Well under the 16MiB HTTP body cap, and previously stored verbatim —
        // it then rendered UNCLAMPED inside the briefing.
        let huge = "A".repeat(1_000_000);
        let report = log_decision(LogDecisionOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            title: huge.clone(),
            reasoning: Some(huge),
            alternatives: vec![],
            tags: vec![],
            scope: String::new(),
            supersedes: None,
        })
        .unwrap();

        let (_project, connection) = resolve_and_open(None, project_dir, Some(home)).unwrap();
        let (title, reasoning): (String, String) = connection
            .query_row(
                "SELECT title, reasoning FROM decisions WHERE id = ?1",
                rusqlite::params![report.decision_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert!(
            title.chars().count() <= MAX_TITLE_CHARS + TRUNCATION_MARK.chars().count(),
            "title must be clamped, got {} chars",
            title.chars().count()
        );
        assert!(title.ends_with(TRUNCATION_MARK), "clamping must be marked");
        assert!(
            reasoning.chars().count() <= MAX_BODY_CHARS + TRUNCATION_MARK.chars().count(),
            "reasoning must be clamped, got {} chars",
            reasoning.chars().count()
        );
    }

    #[test]
    fn default_scope_cursor_extraction_sees_events() {
        use std::io::{Read, Write};
        use std::net::TcpListener;

        let (_temp, home, project_dir) = setup_project();

        // Events at the default (empty/root) scope — what the desktop terminal
        // ingest and a bare `grafiki capture extract` actually produce.
        let capture = start_capture_session(StartCaptureOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            scope: String::new(),
            source_app: Some("test".to_owned()),
            consent_profile: None,
            redaction_profile: None,
        })
        .unwrap();
        ingest_capture_event(IngestCaptureEventOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            capture_id: Some(capture.capture.id.clone()),
            source_type: "terminal".to_owned(),
            source: None,
            title: Some("session".to_owned()),
            text: Some("We chose SQLite over Postgres for V1 because it's embedded.".to_owned()),
            payload: None,
            metadata: None,
            scope: String::new(),
            privacy_level: None,
            captured_at: None,
            redacted: false,
        })
        .unwrap();

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let mut buf = [0u8; 16384];
            let _ = socket.read(&mut buf).unwrap();
            let content = "[{\"kind\":\"decision\",\"title\":\"Use SQLite\",\"content\":\
                 \"Chosen over Postgres for V1 because it is embedded.\"}]";
            let envelope =
                serde_json::json!({"message":{"role":"assistant","content":content}}).to_string();
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\
                 Content-Length: {}\r\nConnection: close\r\n\r\n{envelope}",
                envelope.len()
            );
            socket.write_all(response.as_bytes()).unwrap();
        });

        // The exact path the desktop "Extract now" button, the ⌘K action, and a
        // bare `grafiki capture extract` hit: no capture id, default scope,
        // cursor-based unextracted-only read. The root scope resolves to an
        // empty chain, and `scope IN ()` is always false in SQLite — this used
        // to return zero events and report "no new session activity" forever.
        let report = extract_capture_memory(ExtractCaptureOptions {
            project_name: None,
            start_dir: project_dir,
            grafiki_home: Some(home),
            capture_id: None,
            scope: String::new(),
            limit: 100,
            model: None,
            ollama_url: Some(format!("http://127.0.0.1:{port}")),
            unextracted_only: true,
        })
        .unwrap();
        server.join().unwrap();

        assert!(
            report.events_read >= 1,
            "default-scope events must be visible to cursor-based extraction, got: {}",
            report.message
        );
        assert_eq!(report.proposed, 1);
    }

    #[test]
    fn run_capture_watch_imports_transcript_then_proposes() {
        use std::io::{Read, Write};
        use std::net::TcpListener;

        let (_temp, home, project_dir) = setup_project();
        let scope = "example-project/core";

        // A fake Claude Code transcript on disk (what the watcher tails).
        let transcript = project_dir.join("session.jsonl");
        std::fs::write(
            &transcript,
            "{\"type\":\"user\",\"timestamp\":\"2026-07-01T00:00:00Z\",\"message\":{\"role\":\
             \"user\",\"content\":\"Let's use SQLite for V1.\"}}\n{\"type\":\"assistant\",\
             \"timestamp\":\"2026-07-01T00:00:01Z\",\"message\":{\"role\":\"assistant\",\"content\":\
             [{\"type\":\"text\",\"text\":\"Agreed — SQLite, embedded, no server.\"}]}}\n",
        )
        .unwrap();

        // Stand-in Ollama returning one extracted decision.
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let mut buf = [0u8; 16384];
            let _ = socket.read(&mut buf).unwrap();
            let content = "[{\"kind\":\"decision\",\"title\":\"Use SQLite\",\"content\":\"SQLite \
                 for V1 — embedded, no server.\"}]";
            let envelope =
                serde_json::json!({"message":{"role":"assistant","content":content}}).to_string();
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{envelope}",
                envelope.len()
            );
            socket.write_all(response.as_bytes()).unwrap();
        });

        // One watcher pass with an explicit transcript path (the daemon uses None
        // to auto-discover this project's ~/.claude/projects dir).
        let report = run_capture_watch(RunCaptureWatchOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            agent: "claude-code".to_owned(),
            input: Some(transcript.clone()),
            scope: scope.to_owned(),
            limit: 100,
            model: None,
            ollama_url: Some(format!("http://127.0.0.1:{port}")),
        })
        .unwrap();
        server.join().unwrap();

        assert!(report.events_imported >= 2, "transcript turns imported");
        assert_eq!(report.proposed, 1, "the extracted decision is proposed");

        // Landed in the review queue (propose-for-review), from the LLM extractor.
        let candidates = list_candidates(ListCandidatesOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            status: Some("pending".to_owned()),
            scope: scope.to_owned(),
            limit: 100,
            order: CandidateOrder::Recent,
        })
        .unwrap();
        assert!(candidates
            .iter()
            .any(|c| c.source_type == "capture:llm" && c.record_type == "decision"));

        // Second pass with the SAME transcript: dedup ⇒ nothing new ⇒ no model call,
        // no duplicate proposal (incremental by construction).
        let again = run_capture_watch(RunCaptureWatchOptions {
            project_name: None,
            start_dir: project_dir,
            grafiki_home: Some(home),
            agent: "claude-code".to_owned(),
            input: Some(transcript),
            scope: scope.to_owned(),
            limit: 100,
            model: None,
            ollama_url: Some("http://127.0.0.1:1".to_owned()), // unreachable; must not be hit
        })
        .unwrap();
        assert_eq!(again.events_imported, 0, "re-run imports nothing new");
        assert_eq!(again.proposed, 0);
    }

    #[test]
    fn run_capture_watch_rechecks_consent_and_fails_closed() {
        let (_temp, home, project_dir) = setup_project();
        crate::project::update_capture_config(crate::project::UpdateCaptureConfigOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            sources: crate::project::CaptureSourceUpdates {
                transcripts: Some(false),
                ..Default::default()
            },
            ..Default::default()
        })
        .unwrap();
        let disabled = run_capture_watch(RunCaptureWatchOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            agent: "claude-code".to_owned(),
            input: None,
            scope: "example-project".to_owned(),
            limit: 10,
            model: None,
            ollama_url: None,
        })
        .unwrap_err();
        assert!(disabled.to_string().contains("disabled"));

        std::fs::remove_file(project_dir.join(".grafiki.capture.json")).unwrap();
        let missing = run_capture_watch(RunCaptureWatchOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home),
            agent: "claude-code".to_owned(),
            input: None,
            scope: "example-project".to_owned(),
            limit: 10,
            model: None,
            ollama_url: None,
        })
        .unwrap_err();
        assert!(missing.to_string().contains("existing consent file"));
        assert!(
            !project_dir.join(".grafiki.capture.json").exists(),
            "passive capture must not recreate a deleted consent file"
        );
    }

    #[test]
    fn failed_extraction_does_not_consume_the_session() {
        use std::io::{Read, Write};
        use std::net::TcpListener;

        let (_temp, home, project_dir) = setup_project();
        let scope = "example-project/core";
        let transcript = project_dir.join("session.jsonl");
        std::fs::write(
            &transcript,
            "{\"type\":\"user\",\"timestamp\":\"2026-07-01T00:00:00Z\",\"message\":{\"role\":\
             \"user\",\"content\":\"Pin the CI timezone to UTC.\"}}\n",
        )
        .unwrap();

        // Pass 1: the transcript imports, but the model is unreachable — the
        // extraction fails AFTER the events were ingested (the exact real-world
        // failure that used to permanently consume the session).
        let failed = run_capture_watch(RunCaptureWatchOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            agent: "claude-code".to_owned(),
            input: Some(transcript.clone()),
            scope: scope.to_owned(),
            limit: 100,
            model: None,
            ollama_url: Some("http://127.0.0.1:1".to_owned()), // nothing listens here
        });
        assert!(failed.is_err(), "unreachable model must surface an error");

        // Pass 2: the model is back. Import dedups to 0 new events, but the
        // cursor never advanced — the SAME events must now be extracted.
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let mut buf = [0u8; 16384];
            let _ = socket.read(&mut buf).unwrap();
            let content =
                "[{\"kind\":\"decision\",\"title\":\"Pin CI to UTC\",\"content\":\"CI timezone \
                 pinned to UTC.\"}]";
            let envelope =
                serde_json::json!({"message":{"role":"assistant","content":content}}).to_string();
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{envelope}",
                envelope.len()
            );
            socket.write_all(response.as_bytes()).unwrap();
        });
        let recovered = run_capture_watch(RunCaptureWatchOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            agent: "claude-code".to_owned(),
            input: Some(transcript),
            scope: scope.to_owned(),
            limit: 100,
            model: None,
            ollama_url: Some(format!("http://127.0.0.1:{port}")),
        })
        .unwrap();
        server.join().unwrap();

        assert_eq!(recovered.events_imported, 0, "dedup: nothing re-imported");
        assert_eq!(
            recovered.proposed, 1,
            "the previously-failed session must still be extracted"
        );
        let candidates = list_candidates(ListCandidatesOptions {
            project_name: None,
            start_dir: project_dir,
            grafiki_home: Some(home),
            status: Some("pending".to_owned()),
            scope: scope.to_owned(),
            limit: 100,
            order: CandidateOrder::Recent,
        })
        .unwrap();
        assert!(candidates
            .iter()
            .any(|c| c.source_type == "capture:llm" && c.record_type == "decision"));
    }

    #[test]
    fn revert_candidate_approval_refuses_when_entity_is_shared() {
        // 2026-07-04 adversarial review finding: entities are upserted by
        // (name, scope) across approvals — a DIFFERENT candidate's already-
        // approved observation can point at the SAME entity row. Undoing the
        // first candidate's entity approval must NOT cascade-delete the
        // second candidate's unrelated, already-trusted observation.
        let (_temp, home, project_dir) = setup_project();

        let entity_candidate = propose_candidate(ProposeCandidateOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            source_type: "capture:llm".to_owned(),
            source: None,
            record_type: "entity".to_owned(),
            payload: serde_json::json!({ "name": "SharedThing", "entity_type": "concept" }),
            scope: String::new(),
            confidence: 0.6,
            rationale: None,
            evidence: Vec::new(),
        })
        .unwrap();
        let entity_id = approve_candidate(ApproveCandidateOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            id: entity_candidate.candidate.id.clone(),
        })
        .unwrap()
        .candidate
        .trusted_record_id
        .unwrap();

        let observation_candidate = propose_candidate(ProposeCandidateOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            source_type: "capture:llm".to_owned(),
            source: None,
            record_type: "observation".to_owned(),
            payload: serde_json::json!({
                "entity_name": "SharedThing",
                "content": "an unrelated fact from a different session"
            }),
            scope: String::new(),
            confidence: 0.6,
            rationale: None,
            evidence: Vec::new(),
        })
        .unwrap();
        approve_candidate(ApproveCandidateOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            id: observation_candidate.candidate.id.clone(),
        })
        .unwrap();

        let result = revert_candidate_approval(RevertApprovalOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            id: entity_candidate.candidate.id.clone(),
        });
        assert!(
            result.is_err(),
            "revert must refuse when the entity has other approved memory attached"
        );

        // Nothing was touched: the entity candidate is still approved, the
        // observation candidate is still approved, and the observation itself
        // (and its entity) still exist untouched.
        let entity_candidate_after = list_candidates(ListCandidatesOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            status: Some("approved".to_owned()),
            scope: String::new(),
            limit: 50,
            order: CandidateOrder::Recent,
        })
        .unwrap()
        .into_iter()
        .find(|c| c.id == entity_candidate.candidate.id)
        .unwrap();
        assert_eq!(entity_candidate_after.status, "approved");
        let observations = list_observations(ObservationListOptions {
            project_name: None,
            start_dir: project_dir,
            grafiki_home: Some(home),
            scope: String::new(),
            category: None,
        })
        .unwrap();
        assert!(observations
            .iter()
            .any(|o| o.entity_id == entity_id && o.entity_name == "SharedThing"));
    }

    #[test]
    fn undo_restores_a_context_record_that_approval_overwrote() {
        let (_temp, home, project_dir) = setup_project();
        // A trusted context record already exists (the user wrote it directly).
        add_context(AddContextOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            key: "deploy-runbook".to_owned(),
            title: "Deploy Runbook".to_owned(),
            content: "ORIGINAL: run ./deploy.sh from main".to_owned(),
            category: "runbook".to_owned(),
            scope: String::new(),
        })
        .unwrap();

        // A candidate approval collides on the same key and overwrites it.
        let proposed = propose_candidate(ProposeCandidateOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            source_type: "capture:llm".to_owned(),
            source: None,
            record_type: "context".to_owned(),
            payload: serde_json::json!({
                "key": "deploy-runbook",
                "title": "Deploy Runbook",
                "category": "runbook",
                "content": "OVERWRITTEN: run make deploy",
            }),
            scope: String::new(),
            confidence: 0.6,
            rationale: None,
            evidence: Vec::new(),
        })
        .unwrap();
        approve_candidate(ApproveCandidateOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            id: proposed.candidate.id.clone(),
        })
        .unwrap();

        // Undo must bring back the ORIGINAL contents, not delete the record.
        revert_candidate_approval(RevertApprovalOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            id: proposed.candidate.id,
        })
        .unwrap();
        let restored = get_context(GetContextOptions {
            project_name: None,
            start_dir: project_dir,
            grafiki_home: Some(home),
            key: "deploy-runbook".to_owned(),
        })
        .unwrap();
        assert!(
            restored.content.contains("ORIGINAL"),
            "undo must restore the overwritten record's prior contents, got: {}",
            restored.content
        );
    }

    #[test]
    fn candidate_evidence_free_text_is_redacted() {
        let (_temp, home, project_dir) = setup_project();
        let proposed = propose_candidate(ProposeCandidateOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            source_type: "capture:llm".to_owned(),
            source: None,
            record_type: "decision".to_owned(),
            payload: serde_json::json!({ "title": "Wire up webhook", "reasoning": "ship it" }),
            scope: String::new(),
            confidence: 0.6,
            rationale: None,
            evidence: vec![EvidenceInput {
                source_event_id: None,
                source_type: "terminal".to_owned(),
                source: Some("export API_TOKEN=supersecretvalue123".to_owned()),
                title: Some("API_SECRET=anothersecretvalue456".to_owned()),
                excerpt: "$ export SESSION_PASSWORD=thirdsecretvalue789 && curl ...".to_owned(),
                uri: None,
                byte_start: None,
                byte_end: None,
                line_start: None,
                line_end: None,
                captured_at: None,
            }],
        })
        .unwrap();
        let candidate = list_candidates(ListCandidatesOptions {
            project_name: None,
            start_dir: project_dir,
            grafiki_home: Some(home),
            status: Some("pending".to_owned()),
            scope: String::new(),
            limit: 10,
            order: CandidateOrder::Recent,
        })
        .unwrap()
        .into_iter()
        .find(|c| c.id == proposed.candidate.id)
        .unwrap();
        let evidence_blob = format!("{:?}", candidate.evidence);
        assert!(
            !evidence_blob.contains("supersecretvalue123")
                && !evidence_blob.contains("anothersecretvalue456")
                && !evidence_blob.contains("thirdsecretvalue789"),
            "evidence source/title/excerpt must all be redacted, got: {evidence_blob}"
        );
    }

    #[test]
    fn revert_candidate_approval_recovers_when_trusted_record_already_deleted() {
        // 2026-07-04 adversarial review finding: if the trusted record was
        // already deleted independently (e.g. via Browse), revert must still
        // succeed and return the candidate to pending — not wedge it forever
        // behind a "record not found" error.
        let (_temp, home, project_dir) = setup_project();
        let proposed = propose_candidate(ProposeCandidateOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            source_type: "capture:llm".to_owned(),
            source: None,
            record_type: "decision".to_owned(),
            payload: serde_json::json!({ "title": "Doomed", "reasoning": "will be deleted early" }),
            scope: String::new(),
            confidence: 0.6,
            rationale: None,
            evidence: Vec::new(),
        })
        .unwrap();
        let approved = approve_candidate(ApproveCandidateOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            id: proposed.candidate.id.clone(),
        })
        .unwrap();
        let decision_id = approved.candidate.trusted_record_id.unwrap();

        // Independently delete the trusted record, as if via Browse.
        delete_decision(DeleteDecisionOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            id: decision_id,
        })
        .unwrap();

        let reverted = revert_candidate_approval(RevertApprovalOptions {
            project_name: None,
            start_dir: project_dir,
            grafiki_home: Some(home),
            id: proposed.candidate.id,
        })
        .unwrap();
        assert_eq!(reverted.candidate.status, "pending");
    }

    #[test]
    fn revert_approval_undoes_a_wrong_approve() {
        // don-norman-design-critic finding: approve had no way back even though
        // it's the highest-blast-radius action (creates trusted memory injected
        // into future sessions). revert_candidate_approval is that way back.
        let (_temp, home, project_dir) = setup_project();
        let proposed = propose_candidate(ProposeCandidateOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            source_type: "capture:llm".to_owned(),
            source: None,
            record_type: "decision".to_owned(),
            payload: serde_json::json!({ "title": "Wrong call", "reasoning": "oops" }),
            scope: String::new(),
            confidence: 0.6,
            rationale: None,
            evidence: vec![EvidenceInput {
                source_event_id: None,
                source_type: "transcript".to_owned(),
                source: None,
                title: None,
                excerpt: "oops".to_owned(),
                uri: None,
                byte_start: None,
                byte_end: None,
                line_start: None,
                line_end: None,
                captured_at: None,
            }],
        })
        .unwrap();
        let id = proposed.candidate.id;

        let approved = approve_candidate(ApproveCandidateOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            id: id.clone(),
        })
        .unwrap();
        let decision_id = approved.candidate.trusted_record_id.clone().unwrap();
        assert_eq!(approved.candidate.status, "approved");

        let reverted = revert_candidate_approval(RevertApprovalOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            id: id.clone(),
        })
        .unwrap();
        assert_eq!(reverted.candidate.status, "pending");
        assert!(reverted.candidate.trusted_record_type.is_none());
        assert!(reverted.candidate.trusted_record_id.is_none());
        assert!(reverted.candidate.evidence[0].trusted_record_type.is_none());

        // The trusted decision is revoked from active retrieval but retained as
        // immutable audit/history rather than hard-deleted.
        let decisions = list_decisions(DecisionListOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            scope: String::new(),
            status: Some("revoked".to_owned()),
        })
        .unwrap();
        assert!(decisions
            .iter()
            .any(|d| d.id == decision_id && d.status == "revoked"));

        // Reverting an already-pending (never approved) candidate is rejected.
        let again = revert_candidate_approval(RevertApprovalOptions {
            project_name: None,
            start_dir: project_dir,
            grafiki_home: Some(home),
            id,
        });
        assert!(again.is_err());
    }

    #[test]
    fn revert_approval_restores_exact_prior_decision_state() {
        let (_temp, home, project_dir) = setup_project();
        let prior = log_decision(LogDecisionOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            title: "Prior decision".to_owned(),
            reasoning: Some("needs reconsideration".to_owned()),
            alternatives: Vec::new(),
            tags: Vec::new(),
            scope: "example-project/core".to_owned(),
            supersedes: None,
        })
        .unwrap();
        update_decision(UpdateDecisionOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            id: prior.decision_id.clone(),
            title: None,
            reasoning: None,
            scope: None,
            status: Some("revisit".to_owned()),
        })
        .unwrap();
        let proposed = propose_candidate(ProposeCandidateOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            source_type: "capture:llm".to_owned(),
            source: None,
            record_type: "decision".to_owned(),
            payload: serde_json::json!({
                "title": "Replacement decision",
                "supersedes": prior.decision_id,
            }),
            scope: "example-project/core".to_owned(),
            confidence: 0.8,
            rationale: None,
            evidence: Vec::new(),
        })
        .unwrap();
        approve_candidate(ApproveCandidateOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            id: proposed.candidate.id.clone(),
        })
        .unwrap();
        revert_candidate_approval(RevertApprovalOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            id: proposed.candidate.id,
        })
        .unwrap();

        let (_project, connection) = resolve_and_open(None, project_dir, Some(home)).unwrap();
        let restored: (String, Option<String>) = connection
            .query_row(
                "SELECT status, superseded_by FROM decisions WHERE id = ?1",
                [&prior.decision_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(restored, ("revisit".to_owned(), None));
    }

    #[test]
    fn reopen_candidate_undoes_a_wrong_reject() {
        // don-norman-design-critic finding: reject was the only terminal action
        // in the queue — every button disables once status != "pending". A wrong
        // reject had no way back at all.
        let (_temp, home, project_dir) = setup_project();
        let proposed = propose_candidate(ProposeCandidateOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            source_type: "capture:llm".to_owned(),
            source: None,
            record_type: "context".to_owned(),
            payload: serde_json::json!({
                "key": "reopen-test", "title": "Good note", "category": "reference",
                "content": "keep this"
            }),
            scope: String::new(),
            confidence: 0.6,
            rationale: None,
            evidence: Vec::new(),
        })
        .unwrap();
        let id = proposed.candidate.id;

        reject_candidate(RejectCandidateOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            id: id.clone(),
            rationale: Some("clicked the wrong one".to_owned()),
        })
        .unwrap();

        let reopened = reopen_candidate(ReopenCandidateOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            id: id.clone(),
        })
        .unwrap();
        assert_eq!(reopened.candidate.status, "pending");

        // Reopening a pending (never rejected) candidate is rejected.
        let again = reopen_candidate(ReopenCandidateOptions {
            project_name: None,
            start_dir: project_dir,
            grafiki_home: Some(home),
            id,
        });
        assert!(again.is_err());
    }

    #[test]
    fn ledger_sessions_carry_recent_memory_titles() {
        // don-norman-design-critic finding: Home rows said "N captured events"
        // instead of what was actually LEARNED — the whole point of the product.
        let (_temp, home, project_dir) = setup_project();
        let scope = "example-project/core";

        let capture = start_capture_session(StartCaptureOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            scope: scope.to_owned(),
            source_app: Some("test".to_owned()),
            consent_profile: None,
            redaction_profile: None,
        })
        .unwrap();
        let event = ingest_capture_event(IngestCaptureEventOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            capture_id: Some(capture.capture.id.clone()),
            scope: scope.to_owned(),
            source_type: "terminal".to_owned(),
            source: None,
            title: None,
            text: Some("decided to pin the CI timezone".to_owned()),
            payload: None,
            metadata: None,
            privacy_level: None,
            redacted: false,
            captured_at: None,
        })
        .unwrap();
        propose_candidate(ProposeCandidateOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            source_type: "capture:llm".to_owned(),
            source: Some(capture.capture.id.clone()),
            record_type: "decision".to_owned(),
            payload: serde_json::json!({ "title": "Pin CI to UTC", "reasoning": "flaky tests" }),
            scope: scope.to_owned(),
            confidence: 0.6,
            rationale: None,
            evidence: vec![EvidenceInput {
                source_event_id: Some(event.event.id.clone()),
                source_type: "terminal".to_owned(),
                source: None,
                title: None,
                excerpt: "decided to pin the CI timezone".to_owned(),
                uri: None,
                byte_start: None,
                byte_end: None,
                line_start: None,
                line_end: None,
                captured_at: None,
            }],
        })
        .unwrap();

        let ledger = capture_ledger(CaptureLedgerOptions {
            project_name: None,
            start_dir: project_dir,
            grafiki_home: Some(home),
            limit: 20,
        })
        .unwrap();
        let session = ledger
            .sessions
            .iter()
            .find(|s| s.id == capture.capture.id)
            .unwrap();
        assert_eq!(
            session.recent_memory_titles,
            vec!["Pin CI to UTC".to_owned()]
        );
    }

    #[test]
    fn approval_claim_blocks_concurrent_and_allows_stale_takeover() {
        // Audit: check-then-act approval let two concurrent calls both create a
        // trusted record. The claim (reviewed_at while still pending) must make
        // the second caller fail fast — and a stale claim (crash mid-approval,
        // >15 min old) must be retakeable.
        let (_temp, home, project_dir) = setup_project();

        let proposed = propose_candidate(ProposeCandidateOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            source_type: "capture:llm".to_owned(),
            source: None,
            record_type: "decision".to_owned(),
            payload: serde_json::json!({ "title": "Claimed", "reasoning": "race test" }),
            scope: String::new(),
            confidence: 0.6,
            rationale: None,
            evidence: Vec::new(),
        })
        .unwrap();
        let id = proposed.candidate.id;

        // Another caller holds a FRESH claim → approval must refuse.
        let (_p, connection) =
            resolve_and_open(None, project_dir.clone(), Some(home.clone())).unwrap();
        connection
            .execute(
                "UPDATE extraction_candidates \
                 SET reviewed_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE id = ?1",
                rusqlite::params![&id],
            )
            .unwrap();
        drop(connection);
        let blocked = approve_candidate(ApproveCandidateOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            id: id.clone(),
        });
        assert!(
            blocked.is_err() && blocked.unwrap_err().to_string().contains("being approved"),
            "a fresh claim must block a second approval"
        );
        let reject_during_approval = reject_candidate(RejectCandidateOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            id: id.clone(),
            rationale: None,
        });
        assert!(reject_during_approval
            .unwrap_err()
            .to_string()
            .contains("being approved"));
        let edit_during_approval = edit_candidate(EditCandidateOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            id: id.clone(),
            record_type: None,
            payload: None,
            scope: None,
            confidence: None,
            rationale: None,
        });
        assert!(edit_during_approval
            .unwrap_err()
            .to_string()
            .contains("being approved"));

        // A STALE claim (crashed approval) is retakeable.
        let (_p, connection) =
            resolve_and_open(None, project_dir.clone(), Some(home.clone())).unwrap();
        connection
            .execute(
                "UPDATE extraction_candidates \
                 SET reviewed_at = '2020-01-01T00:00:00Z' WHERE id = ?1",
                rusqlite::params![&id],
            )
            .unwrap();
        drop(connection);
        let approved = approve_candidate(ApproveCandidateOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            id: id.clone(),
        })
        .unwrap();
        assert_eq!(approved.candidate.status, "approved");

        // And an approved candidate stays terminal.
        let again = approve_candidate(ApproveCandidateOptions {
            project_name: None,
            start_dir: project_dir,
            grafiki_home: Some(home),
            id,
        });
        assert!(again.is_err());
    }

    #[test]
    fn approval_retry_reuses_provisional_trusted_pointer() {
        let (_temp, home, project_dir) = setup_project();
        let trusted = log_decision(LogDecisionOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            title: "Recovered trusted decision".to_owned(),
            reasoning: Some("created before a simulated finalize crash".to_owned()),
            alternatives: Vec::new(),
            tags: Vec::new(),
            scope: "example-project/core".to_owned(),
            supersedes: None,
        })
        .unwrap();
        let proposed = propose_candidate(ProposeCandidateOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            source_type: "capture:llm".to_owned(),
            source: None,
            record_type: "decision".to_owned(),
            payload: serde_json::json!({ "title": "Would duplicate without recovery" }),
            scope: "example-project/core".to_owned(),
            confidence: 0.7,
            rationale: None,
            evidence: Vec::new(),
        })
        .unwrap();
        let (_project, connection) =
            resolve_and_open(None, project_dir.clone(), Some(home.clone())).unwrap();
        connection
            .execute(
                "UPDATE extraction_candidates
                 SET reviewed_at = '2020-01-01T00:00:00Z',
                     trusted_record_type = 'decision', trusted_record_id = ?1
                 WHERE id = ?2",
                rusqlite::params![trusted.decision_id, proposed.candidate.id],
            )
            .unwrap();
        drop(connection);

        let approved = approve_candidate(ApproveCandidateOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            id: proposed.candidate.id,
        })
        .unwrap();
        assert_eq!(
            approved.candidate.trusted_record_id.as_deref(),
            Some(trusted.decision_id.as_str())
        );
        let decisions = list_decisions(DecisionListOptions {
            project_name: None,
            start_dir: project_dir,
            grafiki_home: Some(home),
            scope: "example-project/core".to_owned(),
            status: None,
        })
        .unwrap();
        assert_eq!(
            decisions.len(),
            1,
            "retry must not create a duplicate decision"
        );
    }

    #[test]
    fn approval_error_releases_claim_without_clearing_recovery_state() {
        let (_temp, home, project_dir) = setup_project();
        let proposed = propose_candidate(ProposeCandidateOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            source_type: "capture:llm".to_owned(),
            source: None,
            record_type: "decision".to_owned(),
            payload: serde_json::json!({ "reasoning": "title is intentionally missing" }),
            scope: "example-project/core".to_owned(),
            confidence: 0.5,
            rationale: None,
            evidence: Vec::new(),
        })
        .unwrap();
        let result = approve_candidate(ApproveCandidateOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            id: proposed.candidate.id.clone(),
        });
        assert!(result.is_err());
        let (_project, connection) = resolve_and_open(None, project_dir, Some(home)).unwrap();
        let state: (String, Option<String>, Option<String>) = connection
            .query_row(
                "SELECT status, reviewed_at, trusted_record_id
                 FROM extraction_candidates WHERE id = ?1",
                [&proposed.candidate.id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(state, ("pending".to_owned(), None, None));
    }

    #[test]
    fn empty_scope_browses_every_scope() {
        // Audit 2026-07-04: browse surfaces with the default "" scope hid
        // sub-scoped rows (chain of "" is just [""]) — capture-session detail,
        // Memory lists, and the auto-extraction read all missed scoped data.
        let (_temp, home, project_dir) = setup_project();

        ingest_capture_event(IngestCaptureEventOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            capture_id: None,
            scope: "example-project/core".to_owned(),
            source_type: "terminal".to_owned(),
            source: None,
            title: Some("Scoped terminal chunk".to_owned()),
            text: Some("cargo test passed".to_owned()),
            payload: None,
            metadata: None,
            privacy_level: None,
            redacted: false,
            captured_at: None,
        })
        .unwrap();
        log_decision(LogDecisionOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            title: "Scoped decision".to_owned(),
            reasoning: Some("because".to_owned()),
            alternatives: Vec::new(),
            tags: Vec::new(),
            scope: "example-project/core".to_owned(),
            supersedes: None,
        })
        .unwrap();

        let events = list_capture_events(ListCaptureEventsOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            capture_id: None,
            source_type: None,
            scope: String::new(),
            limit: 50,
        })
        .unwrap();
        assert!(
            events.iter().any(|e| e.scope == "example-project/core"),
            "empty-scope event browse must include sub-scoped events"
        );

        let decisions = list_decisions(DecisionListOptions {
            project_name: None,
            start_dir: project_dir,
            grafiki_home: Some(home),
            scope: String::new(),
            status: None,
        })
        .unwrap();
        assert!(
            decisions.iter().any(|d| d.scope == "example-project/core"),
            "empty-scope decision browse must include sub-scoped decisions"
        );
    }

    #[test]
    fn edit_candidate_redacts_like_the_propose_path() {
        // Audit finding: the edit path wrote payload/rationale verbatim, so a
        // reviewer's edit could persist a secret the original proposal would
        // have scrubbed. Editing must run the same redaction pass.
        let (_temp, home, project_dir) = setup_project();

        let proposed = propose_candidate(ProposeCandidateOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            source_type: "capture:llm".to_owned(),
            source: None,
            record_type: "decision".to_owned(),
            payload: serde_json::json!({ "title": "Clean", "reasoning": "no secrets" }),
            scope: String::new(),
            confidence: 0.6,
            rationale: None,
            evidence: Vec::new(),
        })
        .unwrap();

        let edited = edit_candidate(EditCandidateOptions {
            project_name: None,
            start_dir: project_dir,
            grafiki_home: Some(home),
            id: proposed.candidate.id,
            record_type: None,
            payload: Some(serde_json::json!({
                "title": "Use the staging key",
                "reasoning": "no real change",
                "client_secret": "abcdef1234567890clientsecretvalue"
            })),
            scope: None,
            confidence: None,
            rationale: Some("api_key=sk-live-abcdef1234567890".to_owned()),
        })
        .unwrap();

        let blob = serde_json::to_string(&edited.candidate.payload).unwrap();
        assert!(
            !blob.contains("abcdef1234567890clientsecretvalue"),
            "edited payload must be redacted: {blob}"
        );
        assert_eq!(
            edited.candidate.payload["client_secret"],
            "[REDACTED_SECRET]"
        );
        let rationale = edited.candidate.rationale.unwrap_or_default();
        assert!(
            !rationale.contains("sk-live-abcdef1234567890"),
            "edited rationale must be redacted: {rationale}"
        );
    }

    #[test]
    fn empty_scope_lists_the_whole_review_queue() {
        // Regression for the first QA campaign's badge/Review mismatch: the Home
        // badge counts EVERY pending candidate, but listing with the default ""
        // scope hid sub-scoped ones (chain of "" is just [""]). Empty scope now
        // means the whole queue; a non-empty scope still narrows to its chain.
        let (_temp, home, project_dir) = setup_project();

        let propose = |scope: &str, title: &str| {
            propose_candidate(ProposeCandidateOptions {
                project_name: None,
                start_dir: project_dir.clone(),
                grafiki_home: Some(home.clone()),
                source_type: "capture:llm".to_owned(),
                source: Some("recent-capture".to_owned()),
                record_type: "decision".to_owned(),
                payload: serde_json::json!({ "title": title, "reasoning": "because" }),
                scope: scope.to_owned(),
                confidence: 0.6,
                rationale: None,
                evidence: Vec::new(),
            })
            .unwrap()
        };
        propose("", "Root-scoped decision");
        propose("example-project/core", "Sub-scoped decision");

        let list = |scope: &str| {
            list_candidates(ListCandidatesOptions {
                project_name: None,
                start_dir: project_dir.clone(),
                grafiki_home: Some(home.clone()),
                status: Some("pending".to_owned()),
                scope: scope.to_owned(),
                limit: 50,
                order: CandidateOrder::Recent,
            })
            .unwrap()
        };
        assert_eq!(list("").len(), 2, "default scope = the whole review queue");
        assert_eq!(
            list("example-project/core").len(),
            2,
            "a sub-scope still sees its chain (root + itself)"
        );
        assert_eq!(
            list("unrelated").len(),
            1,
            "an unrelated scope narrows to its chain (root only)"
        );
    }

    #[test]
    fn candidate_review_promotes_or_rejects_untrusted_memory() {
        let (_temp, home, project_dir) = setup_project();

        let proposed = propose_candidate(ProposeCandidateOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            source_type: "connector:test".to_owned(),
            source: Some("thread-123".to_owned()),
            record_type: "decision".to_owned(),
            payload: serde_json::json!({
                "title": "Review candidates before trusting them",
                "reasoning": "Connector output should be explicit."
            }),
            scope: "example-project/core".to_owned(),
            confidence: 0.82,
            rationale: Some("Extracted from a synthetic connector event.".to_owned()),
            evidence: vec![EvidenceInput {
                source_event_id: None,
                source_type: "transcript".to_owned(),
                source: Some("thread-123".to_owned()),
                title: Some("Synthetic transcript".to_owned()),
                excerpt: "Connector output should be explicit.".to_owned(),
                uri: Some("grafiki://test/thread-123".to_owned()),
                byte_start: None,
                byte_end: None,
                line_start: None,
                line_end: None,
                captured_at: None,
            }],
        })
        .unwrap();
        assert_eq!(proposed.candidate.status, "pending");
        assert_eq!(proposed.candidate.record_type, "decision");

        let pending = list_candidates(ListCandidatesOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            status: Some("pending".to_owned()),
            scope: "example-project/core".to_owned(),
            limit: 10,
            order: CandidateOrder::Recent,
        })
        .unwrap();
        assert_eq!(pending.len(), 1);

        let edited = edit_candidate(EditCandidateOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            id: proposed.candidate.id.clone(),
            record_type: None,
            payload: Some(serde_json::json!({
                "title": "Review edited candidates before trusting them",
                "reasoning": "Connector output should be explicit, editable, and cited."
            })),
            scope: None,
            confidence: Some(0.9),
            rationale: Some("Edited during human review.".to_owned()),
        })
        .unwrap();
        assert_eq!(edited.candidate.status, "pending");
        assert_eq!(edited.candidate.confidence, 0.9);

        let approved = approve_candidate(ApproveCandidateOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            id: proposed.candidate.id.clone(),
        })
        .unwrap();
        assert_eq!(approved.candidate.status, "approved");
        assert_eq!(
            approved.candidate.trusted_record_type.as_deref(),
            Some("decision")
        );
        assert!(approved.candidate.trusted_record_id.is_some());
        assert_eq!(approved.candidate.evidence.len(), 1);
        assert_eq!(
            approved.candidate.evidence[0]
                .trusted_record_type
                .as_deref(),
            Some("decision")
        );

        let decisions = list_decisions(DecisionListOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            scope: "example-project/core".to_owned(),
            status: None,
        })
        .unwrap();
        assert_eq!(decisions.len(), 1);
        assert_eq!(
            decisions[0].title,
            "Review edited candidates before trusting them"
        );

        let rejected_source = propose_candidate(ProposeCandidateOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            source_type: "connector:test".to_owned(),
            source: Some("thread-456".to_owned()),
            record_type: "state".to_owned(),
            payload: serde_json::json!({
                "key": "unclear-work",
                "title": "Ambiguous extracted work"
            }),
            scope: "example-project/core".to_owned(),
            confidence: 0.3,
            rationale: None,
            evidence: Vec::new(),
        })
        .unwrap();
        let rejected = reject_candidate(RejectCandidateOptions {
            project_name: None,
            start_dir: project_dir,
            grafiki_home: Some(home),
            id: rejected_source.candidate.id,
            rationale: Some("Too ambiguous.".to_owned()),
        })
        .unwrap();
        assert_eq!(rejected.candidate.status, "rejected");
        assert_eq!(
            rejected.candidate.rationale.as_deref(),
            Some("Too ambiguous.")
        );
    }

    #[test]
    fn observation_candidate_supersedes_prior_observation() {
        let (_temp, home, project_dir) = setup_project();
        let scope = "example-project/core";

        // Old fact.
        let old = save_entity(SaveEntityOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            name: "Deploy Target".to_owned(),
            entity_type: "service".to_owned(),
            observe: Some("We deploy to AWS us-east-1".to_owned()),
            category: "architecture".to_owned(),
            scope: scope.to_owned(),
            relate: None,
        })
        .unwrap();
        let old_obs = old.observation_id.expect("old observation id");

        // valid_from is second-precision; ensure the superseding fact is strictly
        // newer so recency arbitration applies.
        std::thread::sleep(std::time::Duration::from_millis(1100));

        let proposed = propose_candidate(ProposeCandidateOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            source_type: "connector:test".to_owned(),
            source: None,
            record_type: "observation".to_owned(),
            payload: serde_json::json!({
                "entity_name": "Deploy Target",
                "content": "We deploy to GCP europe-west1",
                "category": "architecture",
                "supersedes": old_obs,
            }),
            scope: scope.to_owned(),
            confidence: 0.9,
            rationale: None,
            evidence: Vec::new(),
        })
        .unwrap();

        approve_candidate(ApproveCandidateOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            id: proposed.candidate.id.clone(),
        })
        .unwrap();

        // New fact surfaced.
        let search_new = search_memory(SearchMemoryOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            query: "europe-west1".to_owned(),
            record_type: "all".to_owned(),
            mode: SearchMode::Keyword,
            scope: scope.to_owned(),
            limit: 10,
            temporal_weight: 0.0,
        })
        .unwrap();
        assert!(
            search_new
                .results
                .iter()
                .any(|r| r.snippet.contains("europe-west1")),
            "new observation should be retrievable"
        );

        // Stale fact suppressed (valid_to set → excluded from search).
        let search_stale = search_memory(SearchMemoryOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            query: "us-east-1".to_owned(),
            record_type: "all".to_owned(),
            mode: SearchMode::Keyword,
            scope: scope.to_owned(),
            limit: 10,
            temporal_weight: 0.0,
        })
        .unwrap();
        assert!(
            !search_stale
                .results
                .iter()
                .any(|r| r.snippet.contains("us-east-1")),
            "stale observation must be suppressed after supersession"
        );

        revert_candidate_approval(RevertApprovalOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            id: proposed.candidate.id,
        })
        .unwrap();
        let restored = search_memory(SearchMemoryOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            query: "us-east-1".to_owned(),
            record_type: "all".to_owned(),
            mode: SearchMode::Keyword,
            scope: scope.to_owned(),
            limit: 10,
            temporal_weight: 0.0,
        })
        .unwrap();
        assert!(
            restored.results.iter().any(|result| result.id == old_obs),
            "undo must reopen the predecessor's exact validity window"
        );
        let (_project, connection) = resolve_and_open(None, project_dir, Some(home)).unwrap();
        let valid_to: Option<String> = connection
            .query_row(
                "SELECT valid_to FROM observations WHERE id = ?1",
                [&old_obs],
                |row| row.get(0),
            )
            .unwrap();
        assert!(valid_to.is_none());
    }

    #[test]
    fn deterministic_slot_conflict_routes_supersedes_without_model() {
        let (_temp, home, project_dir) = setup_project();
        let scope = "example-project/core";

        // Prior single-valued fact, stored as a canonical slot line.
        let old = save_entity(SaveEntityOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            name: "auth-service".to_owned(),
            entity_type: "service".to_owned(),
            observe: Some("status: active".to_owned()),
            category: "architecture".to_owned(),
            scope: scope.to_owned(),
            relate: None,
        })
        .unwrap();
        let old_obs = old.observation_id.expect("old observation id");

        // A new value for the same single-valued slot — explicit predicate/value,
        // no embedding model required. Detection runs on the default build.
        let proposed = propose_candidate(ProposeCandidateOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            source_type: "connector:test".to_owned(),
            source: None,
            record_type: "observation".to_owned(),
            payload: serde_json::json!({
                "entity_name": "auth-service",
                "content": "status: deprecated",
                "predicate": "status",
                "value": "deprecated",
                "category": "architecture",
            }),
            scope: scope.to_owned(),
            confidence: 0.9,
            rationale: None,
            evidence: Vec::new(),
        })
        .unwrap();

        assert_eq!(
            proposed
                .candidate
                .payload
                .get("supersedes")
                .and_then(|v| v.as_str()),
            Some(old_obs.as_str()),
            "deterministic detection should point supersedes at the prior single-valued fact"
        );
        assert_eq!(
            proposed
                .candidate
                .payload
                .get("conflict_detector")
                .and_then(|v| v.as_str()),
            Some("slot"),
            "the deterministic detector must tag itself for the gate UX"
        );
        assert_eq!(
            proposed
                .candidate
                .payload
                .get("conflict_predicate")
                .and_then(|v| v.as_str()),
            Some("status")
        );
        assert_eq!(
            proposed
                .candidate
                .payload
                .get("conflict_kind")
                .and_then(|v| v.as_str()),
            Some("review"),
            "deterministic conflicts route to review, never an auto-applied write"
        );
    }

    #[test]
    fn deterministic_slot_detection_ignores_multi_valued_predicates() {
        let (_temp, home, project_dir) = setup_project();
        let scope = "example-project/core";

        // "language" is multi-valued — two values coexist, never a conflict.
        save_entity(SaveEntityOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            name: "alice".to_owned(),
            entity_type: "person".to_owned(),
            observe: Some("language: english".to_owned()),
            category: "general".to_owned(),
            scope: scope.to_owned(),
            relate: None,
        })
        .unwrap();

        let proposed = propose_candidate(ProposeCandidateOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            source_type: "connector:test".to_owned(),
            source: None,
            record_type: "observation".to_owned(),
            payload: serde_json::json!({
                "entity_name": "alice",
                "content": "language: french",
                "predicate": "language",
                "value": "french",
                "category": "general",
            }),
            scope: scope.to_owned(),
            confidence: 0.9,
            rationale: None,
            evidence: Vec::new(),
        })
        .unwrap();

        assert!(
            proposed.candidate.payload.get("supersedes").is_none(),
            "a multi-valued attribute must never trigger a (false) supersession"
        );
    }

    #[test]
    fn deterministic_slot_detection_does_not_mis_parse_prose() {
        // Regression: registry words like "state"/"status" also lead ordinary
        // prose ("state: nodes are draining gracefully"). Such a sentence must
        // NOT be read as a single-valued slot, or two unrelated notes would
        // false-supersede. The single-token value guard is what prevents it.
        let (_temp, home, project_dir) = setup_project();
        let scope = "example-project/core";

        save_entity(SaveEntityOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            name: "cluster-a".to_owned(),
            entity_type: "service".to_owned(),
            observe: Some("state: nodes are draining gracefully".to_owned()),
            category: "general".to_owned(),
            scope: scope.to_owned(),
            relate: None,
        })
        .unwrap();

        // Content-only (no explicit predicate/value) prose with the same lead word.
        let proposed = propose_candidate(ProposeCandidateOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            source_type: "connector:test".to_owned(),
            source: None,
            record_type: "observation".to_owned(),
            payload: serde_json::json!({
                "entity_name": "cluster-a",
                "content": "state: nodes rejoined after the network partition",
                "category": "general",
            }),
            scope: scope.to_owned(),
            confidence: 0.9,
            rationale: None,
            evidence: Vec::new(),
        })
        .unwrap();

        assert!(
            proposed.candidate.payload.get("supersedes").is_none(),
            "prose beginning with a registry word must not be parsed as a slot"
        );
    }

    #[cfg(feature = "fastembed")]
    #[test]
    fn auto_detects_observation_conflict_via_embeddings() {
        let (_temp, home, project_dir) = setup_project();
        let scope = "example-project/core";

        // Seed a fact and build its embedding.
        save_entity(SaveEntityOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            name: "Deploy Target".to_owned(),
            entity_type: "service".to_owned(),
            observe: Some("We deploy the application to AWS us-east-1".to_owned()),
            category: "architecture".to_owned(),
            scope: scope.to_owned(),
            relate: None,
        })
        .unwrap();
        process_embedding_jobs(ProcessEmbeddingsOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            scope: "*".to_owned(),
            limit: 1000,
            rebuild: false,
        })
        .unwrap();

        // Propose a contradicting fact WITHOUT an explicit supersedes.
        let proposed = propose_candidate(ProposeCandidateOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            source_type: "transcript".to_owned(),
            source: None,
            record_type: "observation".to_owned(),
            payload: serde_json::json!({
                "entity_name": "Deploy Target",
                "content": "We now deploy the application to GCP europe-west1",
            }),
            scope: scope.to_owned(),
            confidence: 0.9,
            rationale: None,
            evidence: Vec::new(),
        })
        .unwrap();

        // Detection should have auto-annotated a supersedes hint (routed to review).
        assert!(
            proposed.candidate.payload.get("supersedes").is_some(),
            "automated detection should suggest a supersedes target"
        );
        assert_eq!(
            proposed
                .candidate
                .payload
                .get("conflict_kind")
                .and_then(|v| v.as_str()),
            Some("review")
        );
    }

    #[test]
    fn candidate_bulk_review_reports_successes_and_errors() {
        let (_temp, home, project_dir) = setup_project();
        let first = propose_candidate(ProposeCandidateOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            source_type: "connector:test".to_owned(),
            source: Some("bulk-1".to_owned()),
            record_type: "state".to_owned(),
            payload: serde_json::json!({
                "key": "bulk-one",
                "title": "Bulk one"
            }),
            scope: "example-project/core".to_owned(),
            confidence: 0.4,
            rationale: None,
            evidence: Vec::new(),
        })
        .unwrap();
        let second = propose_candidate(ProposeCandidateOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            source_type: "connector:test".to_owned(),
            source: Some("bulk-2".to_owned()),
            record_type: "state".to_owned(),
            payload: serde_json::json!({
                "key": "bulk-two",
                "title": "Bulk two"
            }),
            scope: "example-project/core".to_owned(),
            confidence: 0.4,
            rationale: None,
            evidence: Vec::new(),
        })
        .unwrap();

        let report = bulk_review_candidates(BulkCandidateReviewOptions {
            project_name: None,
            start_dir: project_dir,
            grafiki_home: Some(home),
            action: "reject".to_owned(),
            ids: vec![
                first.candidate.id,
                second.candidate.id,
                "missing-candidate".to_owned(),
            ],
            rationale: Some("Bulk rejected noisy candidates.".to_owned()),
        })
        .unwrap();

        assert_eq!(report.requested, 3);
        assert_eq!(report.succeeded, 2);
        assert_eq!(report.failed, 1);
        assert_eq!(report.results[0].candidate.status, "rejected");
        assert_eq!(report.errors[0].id, "missing-candidate");
    }

    #[test]
    fn state_events_and_session_log_round_trip() {
        let (_temp, home, project_dir) = setup_project();
        let session_id = start_codex(home.clone(), project_dir.clone());

        let state = upsert_state(UpsertStateOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            key: "memory-loop".to_owned(),
            title: "Build memory loop".to_owned(),
            status: "blocked".to_owned(),
            owner: Some("vishal".to_owned()),
            details: Some("Waiting on command surface".to_owned()),
            blockers: vec!["CLI polish".to_owned()],
            depends_on: vec!["schema".to_owned()],
            scope: "example-project/core".to_owned(),
            priority: "high".to_owned(),
        })
        .unwrap();

        let listed_state = list_state(StateListOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            status: Some("blocked".to_owned()),
            scope: "example-project/core".to_owned(),
        })
        .unwrap();

        let events = list_events(EventListOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            scope: "example-project/core".to_owned(),
            since: None,
            limit: 10,
        })
        .unwrap();

        let sessions = list_sessions(SessionLogOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            scope: "example-project/core".to_owned(),
            session_type: Some("codex".to_owned()),
            limit: 10,
        })
        .unwrap();

        let deleted = delete_state(DeleteStateOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            key: "memory-loop".to_owned(),
        })
        .unwrap();
        let current = list_state(StateListOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            status: None,
            scope: "example-project/core".to_owned(),
        })
        .unwrap();
        let history = list_state(StateListOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            status: Some("abandoned".to_owned()),
            scope: "example-project/core".to_owned(),
        })
        .unwrap();

        assert_eq!(state.key, "memory-loop");
        assert_eq!(state.status, "blocked");
        assert_eq!(listed_state.len(), 1);
        assert!(events
            .events
            .iter()
            .any(|event| event.event_type == "state_changed"));
        assert!(sessions
            .sessions
            .iter()
            .any(|session| session.id == session_id));
        assert_eq!(deleted.key, "memory-loop");
        assert_eq!(deleted.status, "abandoned");
        assert!(current.is_empty());
        assert_eq!(
            history.len(),
            1,
            "retired state must remain queryable as history"
        );
    }

    #[test]
    fn session_records_can_be_corrected_and_completed() {
        let (_temp, home, project_dir) = setup_project();
        let session_id = start_codex(home.clone(), project_dir.clone());

        let corrected = update_session(UpdateSessionOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            id: session_id.clone(),
            session_type: Some("cursor".to_owned()),
            status: Some("active".to_owned()),
            scope: Some("example-project/desktop".to_owned()),
            goal: Some("Maintain desktop session records".to_owned()),
            summary: Some("Session metadata was corrected from the desktop.".to_owned()),
            accomplishments: Some(vec!["Wired edit flow".to_owned()]),
            remaining: Some(vec!["Ship signed build".to_owned()]),
            files_changed: Some(vec!["apps/grafiki-desktop/src/App.tsx".to_owned()]),
        })
        .unwrap();

        assert_eq!(corrected.id, session_id);
        assert_eq!(corrected.session_type, "cursor");
        assert_eq!(corrected.status, "active");
        assert_eq!(corrected.scope, "example-project/desktop");
        assert_eq!(
            corrected.goal.as_deref(),
            Some("Maintain desktop session records")
        );
        assert_eq!(corrected.accomplishments, vec!["Wired edit flow"]);
        assert_eq!(corrected.remaining, vec!["Ship signed build"]);
        assert_eq!(
            corrected.files_changed,
            vec!["apps/grafiki-desktop/src/App.tsx"]
        );
        assert!(corrected.ended_at.is_none());

        let completed = update_session(UpdateSessionOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            id: session_id.clone(),
            session_type: None,
            status: Some("completed".to_owned()),
            scope: None,
            goal: None,
            summary: Some("Desktop session record is complete.".to_owned()),
            accomplishments: None,
            remaining: None,
            files_changed: None,
        })
        .unwrap();

        assert_eq!(completed.status, "completed");
        assert_eq!(
            completed.summary.as_deref(),
            Some("Desktop session record is complete.")
        );
        assert_eq!(completed.accomplishments, vec!["Wired edit flow"]);
        assert!(completed.ended_at.is_some());

        let filtered = list_sessions(SessionLogOptions {
            project_name: None,
            start_dir: project_dir,
            grafiki_home: Some(home),
            scope: "example-project/desktop".to_owned(),
            session_type: Some("cursor".to_owned()),
            limit: 10,
        })
        .unwrap();

        assert_eq!(filtered.sessions.len(), 1);
        assert_eq!(filtered.sessions[0].id, session_id);
        assert_eq!(filtered.sessions[0].remaining, vec!["Ship signed build"]);
    }

    #[test]
    fn graph_traversal_returns_related_entities_and_relations() {
        let (_temp, home, project_dir) = setup_project();
        start_codex(home.clone(), project_dir.clone());

        save_entity(SaveEntityOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            name: "Database".to_owned(),
            entity_type: "service".to_owned(),
            observe: None,
            category: "general".to_owned(),
            scope: "example-project/core".to_owned(),
            relate: None,
        })
        .unwrap();
        save_entity(SaveEntityOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            name: "Auth Service".to_owned(),
            entity_type: "service".to_owned(),
            observe: None,
            category: "general".to_owned(),
            scope: "example-project/core".to_owned(),
            relate: Some("database:depends_on".to_owned()),
        })
        .unwrap();
        save_entity(SaveEntityOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            name: "API Gateway".to_owned(),
            entity_type: "service".to_owned(),
            observe: None,
            category: "general".to_owned(),
            scope: "example-project/core".to_owned(),
            relate: Some("auth-service:calls".to_owned()),
        })
        .unwrap();

        let graph = get_graph(GraphOptions {
            project_name: None,
            start_dir: project_dir,
            grafiki_home: Some(home),
            entity_id: "auth-service".to_owned(),
            depth: 1,
        })
        .unwrap();

        assert_eq!(graph.root, "auth-service");
        assert_eq!(graph.entities.len(), 3);
        assert_eq!(graph.relations.len(), 2);
        assert!(graph
            .relations
            .iter()
            .any(|relation| relation.relation == "depends_on"));
        assert!(graph
            .entities
            .iter()
            .any(|entity| entity.id == "api-gateway"));
    }

    #[test]
    fn relation_records_can_be_listed_updated_and_removed() {
        let (_temp, home, project_dir) = setup_project();
        start_codex(home.clone(), project_dir.clone());

        save_entity(SaveEntityOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            name: "Database".to_owned(),
            entity_type: "service".to_owned(),
            observe: None,
            category: "general".to_owned(),
            scope: "example-project/core".to_owned(),
            relate: None,
        })
        .unwrap();
        let saved = save_entity(SaveEntityOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            name: "Auth Service".to_owned(),
            entity_type: "service".to_owned(),
            observe: None,
            category: "general".to_owned(),
            scope: "example-project/core".to_owned(),
            relate: Some("database:depends_on".to_owned()),
        })
        .unwrap();

        let listed = list_relations(RelationListOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            scope: "example-project/core".to_owned(),
            relation: None,
        })
        .unwrap();
        assert_eq!(listed.len(), 1);
        let relation_id = saved.relation_id.unwrap();
        assert_eq!(listed[0].id, relation_id);

        let updated = update_relation(UpdateRelationOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            id: relation_id.clone(),
            relation: Some("uses".to_owned()),
            weight: Some(2.5),
            confidence: Some(0.75),
            source_type: Some("inferred".to_owned()),
            source: Some("desktop maintenance".to_owned()),
        })
        .unwrap();

        assert_eq!(updated.relation, "uses");
        assert_eq!(updated.weight, 2.5);
        assert_eq!(updated.confidence, 0.75);
        assert_eq!(updated.source_type, "INFERRED");
        assert_eq!(updated.source.as_deref(), Some("desktop maintenance"));

        let uses = list_relations(RelationListOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            scope: "example-project/core".to_owned(),
            relation: Some("uses".to_owned()),
        })
        .unwrap();
        assert_eq!(uses.len(), 1);

        let deleted = delete_relation(DeleteRelationOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            id: relation_id,
        })
        .unwrap();
        assert_eq!(deleted.relation, "uses");

        let remaining = list_relations(RelationListOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            scope: "example-project/core".to_owned(),
            relation: None,
        })
        .unwrap();
        let graph = get_graph(GraphOptions {
            project_name: None,
            start_dir: project_dir,
            grafiki_home: Some(home),
            entity_id: "auth-service".to_owned(),
            depth: 1,
        })
        .unwrap();

        assert!(remaining.is_empty());
        assert!(graph.relations.is_empty());
    }

    #[test]
    fn relation_upsert_returns_and_audits_the_persisted_id() {
        let (_temp, home, project_dir) = setup_project();
        start_codex(home.clone(), project_dir.clone());
        save_entity(SaveEntityOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            name: "Database".to_owned(),
            entity_type: "service".to_owned(),
            observe: None,
            category: "general".to_owned(),
            scope: "example-project/core".to_owned(),
            relate: None,
        })
        .unwrap();
        let save = || {
            save_entity(SaveEntityOptions {
                project_name: None,
                start_dir: project_dir.clone(),
                grafiki_home: Some(home.clone()),
                name: "Auth Service".to_owned(),
                entity_type: "service".to_owned(),
                observe: None,
                category: "general".to_owned(),
                scope: "example-project/core".to_owned(),
                relate: Some("database:depends_on".to_owned()),
            })
            .unwrap()
        };
        let first = save().relation_id.unwrap();
        let second = save().relation_id.unwrap();
        assert_eq!(first, second, "upsert must return the existing relation id");

        let (_project, connection) = resolve_and_open(None, project_dir, Some(home)).unwrap();
        let rows: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM relations WHERE id = ?1",
                [&second],
                |row| row.get(0),
            )
            .unwrap();
        let audit_rows: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM events WHERE target_type = 'relation' AND target_id = ?1",
                [&second],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(rows, 1);
        assert_eq!(audit_rows, 2);
    }

    #[test]
    fn keyword_all_interleaves_record_types_before_global_limit() {
        let (_temp, home, project_dir) = setup_project();
        for index in 0..6 {
            save_entity(SaveEntityOptions {
                project_name: None,
                start_dir: project_dir.clone(),
                grafiki_home: Some(home.clone()),
                name: format!("Auth Component {index}"),
                entity_type: "service".to_owned(),
                observe: None,
                category: "general".to_owned(),
                scope: "example-project/core".to_owned(),
                relate: None,
            })
            .unwrap();
        }
        let exact = save_entity(SaveEntityOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            name: "Login".to_owned(),
            entity_type: "service".to_owned(),
            observe: Some("auth uses exact rotating credentials".to_owned()),
            category: "architecture".to_owned(),
            scope: "example-project/core".to_owned(),
            relate: None,
        })
        .unwrap()
        .observation_id
        .unwrap();

        let results = search_memory(SearchMemoryOptions {
            project_name: None,
            start_dir: project_dir,
            grafiki_home: Some(home),
            query: "auth".to_owned(),
            record_type: "all".to_owned(),
            mode: SearchMode::Keyword,
            scope: "example-project/core".to_owned(),
            limit: 5,
            temporal_weight: 0.0,
        })
        .unwrap()
        .results;
        assert!(
            results.iter().any(|result| result.id == exact),
            "an observation must not be starved by entity hits: {results:?}"
        );
    }

    #[test]
    fn entity_ids_are_nonempty_and_collision_safe() {
        let (_temp, home, project_dir) = setup_project();
        let save = |name: &str| {
            save_entity(SaveEntityOptions {
                project_name: None,
                start_dir: project_dir.clone(),
                grafiki_home: Some(home.clone()),
                name: name.to_owned(),
                entity_type: "concept".to_owned(),
                observe: None,
                category: "general".to_owned(),
                scope: "example-project/core".to_owned(),
                relate: None,
            })
            .unwrap()
        };
        let first = save("Auth/API");
        let second = save("Auth API");
        let unicode = save("认证服务");
        assert_ne!(first.entity_id, second.entity_id);
        assert!(!unicode.entity_id.is_empty());
        assert!(unicode.entity_id.starts_with("entity-"));

        let entities = list_entities(super::EntityListOptions {
            project_name: None,
            start_dir: project_dir,
            grafiki_home: Some(home),
            scope: "example-project/core".to_owned(),
            entity_type: None,
        })
        .unwrap();
        assert_eq!(entities.len(), 3);
    }

    #[test]
    fn report_summarizes_graph_shape() {
        let (_temp, home, project_dir) = setup_project();
        start_codex(home.clone(), project_dir.clone());

        save_entity(SaveEntityOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            name: "Database".to_owned(),
            entity_type: "service".to_owned(),
            observe: None,
            category: "general".to_owned(),
            scope: "example-project/core".to_owned(),
            relate: None,
        })
        .unwrap();
        save_entity(SaveEntityOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            name: "Auth Service".to_owned(),
            entity_type: "service".to_owned(),
            observe: Some("Owns token refresh".to_owned()),
            category: "architecture".to_owned(),
            scope: "example-project/core".to_owned(),
            relate: Some("database:depends_on".to_owned()),
        })
        .unwrap();

        let report = generate_report(ProjectReportOptions {
            project_name: None,
            start_dir: project_dir,
            grafiki_home: Some(home),
            scope: "example-project/core".to_owned(),
        })
        .unwrap();

        assert_eq!(report.entity_count, 2);
        assert_eq!(report.relation_count, 1);
        assert_eq!(report.observation_count, 1);
        assert_eq!(report.active_session_count, 1);
        assert!(report
            .god_nodes
            .iter()
            .any(|node| node.id == "auth-service"));
        assert!(!report.suggested_queries.is_empty());
    }

    #[test]
    fn export_bundle_contains_scoped_memory() {
        let (_temp, home, project_dir) = setup_project();
        start_codex(home.clone(), project_dir.clone());

        log_decision(LogDecisionOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            title: "Keep exports local".to_owned(),
            reasoning: Some("Users should own their memory data".to_owned()),
            alternatives: Vec::new(),
            tags: Vec::new(),
            scope: "example-project/core".to_owned(),
            supersedes: None,
        })
        .unwrap();
        save_entity(SaveEntityOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            name: "Database".to_owned(),
            entity_type: "service".to_owned(),
            observe: None,
            category: "general".to_owned(),
            scope: "example-project/core".to_owned(),
            relate: None,
        })
        .unwrap();
        save_entity(SaveEntityOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            name: "Auth Service".to_owned(),
            entity_type: "service".to_owned(),
            observe: Some("JWT refresh uses rotating tokens".to_owned()),
            category: "architecture".to_owned(),
            scope: "example-project/core".to_owned(),
            relate: Some("database:depends_on".to_owned()),
        })
        .unwrap();
        upsert_state(UpsertStateOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            key: "export-command".to_owned(),
            title: "Build export command".to_owned(),
            status: "in-progress".to_owned(),
            owner: None,
            details: None,
            blockers: Vec::new(),
            depends_on: Vec::new(),
            scope: "example-project/core".to_owned(),
            priority: "medium".to_owned(),
        })
        .unwrap();

        let bundle = export_memory(ExportOptions {
            project_name: None,
            start_dir: project_dir,
            grafiki_home: Some(home),
            scope: "example-project/core".to_owned(),
        })
        .unwrap();

        assert_eq!(bundle.entities.len(), 2);
        assert_eq!(bundle.relations.len(), 1);
        assert_eq!(bundle.observations.len(), 1);
        assert_eq!(bundle.decisions.len(), 1);
        assert_eq!(bundle.state.len(), 1);
        assert_eq!(bundle.sessions.len(), 1);
    }

    #[test]
    fn import_bundle_merges_exported_memory() {
        let (temp, home, source_dir) = setup_project();
        start_codex(home.clone(), source_dir.clone());

        log_decision(LogDecisionOptions {
            project_name: None,
            start_dir: source_dir.clone(),
            grafiki_home: Some(home.clone()),
            title: "Keep imports local".to_owned(),
            reasoning: Some("Project memory should stay portable.".to_owned()),
            alternatives: vec![],
            scope: "example-project/core".to_owned(),
            tags: vec![],
            supersedes: None,
        })
        .unwrap();

        save_entity(SaveEntityOptions {
            project_name: None,
            start_dir: source_dir.clone(),
            grafiki_home: Some(home.clone()),
            name: "Database".to_owned(),
            entity_type: "service".to_owned(),
            observe: None,
            category: "general".to_owned(),
            relate: None,
            scope: "example-project/core".to_owned(),
        })
        .unwrap();
        save_entity(SaveEntityOptions {
            project_name: None,
            start_dir: source_dir.clone(),
            grafiki_home: Some(home.clone()),
            name: "Auth Service".to_owned(),
            entity_type: "service".to_owned(),
            observe: Some("JWT refresh uses rotating tokens".to_owned()),
            category: "architecture".to_owned(),
            relate: Some("database:depends_on".to_owned()),
            scope: "example-project/core".to_owned(),
        })
        .unwrap();

        let bundle = export_memory(ExportOptions {
            project_name: None,
            start_dir: source_dir,
            grafiki_home: Some(home.clone()),
            scope: "example-project/core".to_owned(),
        })
        .unwrap();

        let target_dir = temp.path().join("target-project");
        init_project(InitOptions {
            project_name: None,
            project_dir: target_dir.clone(),
            grafiki_home: Some(home.clone()),
        })
        .unwrap();

        let report = import_memory(ImportOptions {
            project_name: None,
            start_dir: target_dir.clone(),
            grafiki_home: Some(home.clone()),
            bundle,
        })
        .unwrap();

        let search = search_memory(SearchMemoryOptions {
            project_name: None,
            start_dir: target_dir.clone(),
            grafiki_home: Some(home.clone()),
            query: "rotating".to_owned(),
            record_type: "all".to_owned(),
            mode: SearchMode::Keyword,
            scope: "example-project/core".to_owned(),
            limit: 10,
            temporal_weight: 0.0,
        })
        .unwrap();

        let graph = get_graph(GraphOptions {
            project_name: None,
            start_dir: target_dir,
            grafiki_home: Some(home),
            entity_id: "auth-service".to_owned(),
            depth: 1,
        })
        .unwrap();

        assert_eq!(report.entities, 2);
        assert_eq!(report.relations, 1);
        assert_eq!(report.observations, 1);
        assert_eq!(report.decisions, 1);
        assert_eq!(report.skipped_relations, 0);
        assert!(search
            .results
            .iter()
            .any(|result| result.snippet.contains("rotating tokens")));
        assert_eq!(graph.relations.len(), 1);
    }

    #[test]
    fn soft_deleted_observation_absent_from_search() {
        let (_temp, home, project_dir) = setup_project();

        save_entity(SaveEntityOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            name: "Auth Service".to_owned(),
            entity_type: "service".to_owned(),
            observe: Some("JWT refresh uses rotating tokens".to_owned()),
            category: "architecture".to_owned(),
            scope: "example-project/core".to_owned(),
            relate: None,
        })
        .unwrap();

        let search_for = |query: &str| {
            search_memory(SearchMemoryOptions {
                project_name: None,
                start_dir: project_dir.clone(),
                grafiki_home: Some(home.clone()),
                query: query.to_owned(),
                record_type: "all".to_owned(),
                mode: SearchMode::Keyword,
                scope: "example-project/core".to_owned(),
                limit: 10,
                temporal_weight: 0.0,
            })
            .unwrap()
        };

        assert!(
            search_for("rotating")
                .results
                .iter()
                .any(|result| result.record_type == "observation"),
            "observation should be findable before deletion"
        );

        let observation_id = list_observations(ObservationListOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            scope: "example-project/core".to_owned(),
            category: None,
        })
        .unwrap()[0]
            .id
            .clone();

        delete_observation(DeleteObservationOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            id: observation_id,
        })
        .unwrap();

        assert!(
            !search_for("rotating")
                .results
                .iter()
                .any(|result| result.record_type == "observation"),
            "soft-deleted observation must not appear in keyword search/ask"
        );
    }

    #[test]
    fn redaction_preserves_whitespace_for_clean_text() {
        let original =
            "line one\nline two with  double  spaces\n\ttabbed line\nno secrets here at all\n";
        let mut text = original.to_owned();
        let changed = super::redact_sensitive_text(&mut text);
        assert!(!changed, "secret-free text must not be flagged as redacted");
        assert_eq!(
            text, original,
            "secret-free text must round-trip byte-for-byte"
        );
    }

    #[test]
    fn redaction_covers_known_token_formats() {
        let mut text = String::from(
            "openai sk-ABCDEFGHIJKLMNOPQRSTUVWX\ngithub ghp_ABCDEFGHIJKLMNOPQRST\ngitlab glpat-ABCDEFGHIJKL\nslack xoxb-111-222-abcdefghij\naws AKIAIOSFODNN7EXAMPLE\ngoogle AIzaSyABCDEFGHIJKLMNOPQRSTU\nanthropic sk-ant-api03-ABCDEFGHIJKL",
        );
        let changed = super::redact_sensitive_text(&mut text);
        assert!(changed);
        for leaked in [
            "sk-ABCDEFGHIJKLMNOPQRSTUVWX",
            "ghp_ABCDEFGHIJKLMNOPQRST",
            "glpat-ABCDEFGHIJKL",
            "xoxb-111-222-abcdefghij",
            "AKIAIOSFODNN7EXAMPLE",
            "AIzaSyABCDEFGHIJKLMNOPQRSTU",
            "sk-ant-api03-ABCDEFGHIJKL",
        ] {
            assert!(!text.contains(leaked), "token must be redacted: {leaked}");
        }
        assert!(text.contains("[REDACTED_GITHUB_TOKEN]"));
        assert!(text.contains("[REDACTED_ANTHROPIC_KEY]"));
        // Newline structure is preserved (no whitespace collapse).
        assert_eq!(text.lines().count(), 7);
    }

    // --- C13: redaction-profile (none / default / strict) ------------------

    #[test]
    fn redact_emails_is_precise() {
        // Real addresses, including ones embedded in punctuation, are redacted.
        assert_eq!(
            super::redact_emails("ping alice@example.com or <bob.smith+ci@corp.io>."),
            "ping [REDACTED_EMAIL] or <[REDACTED_EMAIL]>."
        );
        // UTF-8 (RFC 6531) local parts are captured WHOLE, never partially leaked.
        for utf8 in ["café@example.com", "用户@example.com", "münch@example.com"] {
            assert_eq!(
                super::redact_emails(utf8),
                "[REDACTED_EMAIL]",
                "UTF-8 local part must be fully redacted: {utf8}"
            );
        }
        // Non-emails are left alone (precision).
        for benign in [
            "@handle",
            "user@",
            "a@localhost",
            "see foo @ bar",
            "x@1.2.3.4",
        ] {
            assert_eq!(
                super::redact_emails(benign),
                benign,
                "must not redact non-email: {benign}"
            );
        }
    }

    #[test]
    fn redaction_profile_parse_falls_back_to_default() {
        assert_eq!(
            super::RedactionProfile::parse("none"),
            super::RedactionProfile::None
        );
        assert_eq!(
            super::RedactionProfile::parse("STRICT"),
            super::RedactionProfile::Strict
        );
        // Unknown / empty must NEVER disable redaction.
        for raw in ["", "default", "typo", "  "] {
            assert_eq!(
                super::RedactionProfile::parse(raw),
                super::RedactionProfile::Default,
                "unknown profile {raw:?} must fall back to Default"
            );
        }
    }

    #[test]
    fn redaction_profile_dispatch_layers_correctly() {
        let secret = "api_key=AbCdEf0123456789AbCdEf0123456789";
        let email = "contact dev@example.com";

        // None: nothing is touched (raw capture log; the gate still redacts later).
        let mut t = format!("{secret}\n{email}");
        assert!(!super::redact_text_with_profile(
            &mut t,
            super::RedactionProfile::None
        ));
        assert!(t.contains("AbCdEf0123456789"));
        assert!(t.contains("dev@example.com"));

        // Default: secret redacted, email left (PII is opt-in only).
        let mut t = format!("{secret}\n{email}");
        assert!(super::redact_text_with_profile(
            &mut t,
            super::RedactionProfile::Default
        ));
        assert!(!t.contains("AbCdEf0123456789"));
        assert!(t.contains("dev@example.com"));

        // Strict: both secret AND email redacted.
        let mut t = format!("{secret}\n{email}");
        assert!(super::redact_text_with_profile(
            &mut t,
            super::RedactionProfile::Strict
        ));
        assert!(!t.contains("AbCdEf0123456789"));
        assert!(!t.contains("dev@example.com"));
        assert!(t.contains("[REDACTED_EMAIL]"));
    }

    #[test]
    fn capture_ingest_honors_strict_profile_for_pii() {
        let (_temp, home, project_dir) = setup_project();
        let started = start_capture_session(StartCaptureOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            scope: "example-project".to_owned(),
            source_app: Some("test".to_owned()),
            consent_profile: None,
            redaction_profile: Some("strict".to_owned()),
        })
        .unwrap();

        let event = ingest_capture_event(IngestCaptureEventOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            capture_id: Some(started.capture.id.clone()),
            source_type: "terminal".to_owned(),
            source: None,
            title: Some("ping alice@example.com".to_owned()),
            text: Some("reach me at alice@example.com".to_owned()),
            payload: None,
            metadata: None,
            scope: "example-project".to_owned(),
            privacy_level: None,
            captured_at: None,
            redacted: false,
        })
        .unwrap();

        assert!(event.event.redacted, "strict capture must flag redaction");
        assert!(
            !event
                .event
                .text
                .as_deref()
                .unwrap_or("")
                .contains("alice@example.com"),
            "strict profile must redact the email in captured text"
        );
        assert!(event
            .event
            .text
            .as_deref()
            .unwrap_or("")
            .contains("[REDACTED_EMAIL]"));
    }

    #[test]
    fn capture_ingest_redacts_nested_structured_secrets_before_serialization() {
        let (_temp, home, project_dir) = setup_project();
        let started = start_capture_session(StartCaptureOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            scope: "example-project".to_owned(),
            source_app: Some("test".to_owned()),
            consent_profile: None,
            redaction_profile: Some("default".to_owned()),
        })
        .unwrap();
        let event = ingest_capture_event(IngestCaptureEventOptions {
            project_name: None,
            start_dir: project_dir,
            grafiki_home: Some(home),
            capture_id: Some(started.capture.id),
            source_type: "agent".to_owned(),
            source: None,
            title: None,
            text: None,
            payload: Some(serde_json::json!({
                "client_secret": "plainsecretvalue123",
                "nested": { "password": "hunter2plaintext" },
                "credentials": [{ "value": "nested-credential" }],
                "client_id": "public-id"
            })),
            metadata: Some(serde_json::json!({
                "auth_token": 123456,
                "owner": "alice"
            })),
            scope: "example-project".to_owned(),
            privacy_level: Some("internal".to_owned()),
            captured_at: None,
            redacted: false,
        })
        .unwrap();

        let serialized = serde_json::to_string(&event.event).unwrap();
        for secret in [
            "plainsecretvalue123",
            "hunter2plaintext",
            "nested-credential",
            "123456",
        ] {
            assert!(
                !serialized.contains(secret),
                "structured secret leaked: {serialized}"
            );
        }
        assert_eq!(
            event.event.payload.as_ref().unwrap()["client_id"],
            "public-id"
        );
        assert_eq!(event.event.metadata.as_ref().unwrap()["owner"], "alice");
        assert!(event.event.redacted);
        assert_eq!(event.event.privacy_level, "sensitive");
    }

    // --- M-E6: entropy-gated + additional-prefix secret detection ----------

    #[test]
    fn redaction_covers_additional_provider_prefixes() {
        // Synthetic, leak-safe (never a real key).
        let mut text = String::from(
            "sendgrid SG.AbCdEfGhIjKlMnOpQrSt npm npm_aBcDeFgHiJkLmNoPqRsT123456 \
             do dop_v1_abcdef1234567890 hf hf_aBcDeFgHiJkLmNoPqRsTuV postman PMAK-abc123def456",
        );
        let changed = super::redact_sensitive_text(&mut text);
        assert!(changed);
        for marker in [
            "[REDACTED_SENDGRID_KEY]",
            "[REDACTED_NPM_TOKEN]",
            "[REDACTED_DIGITALOCEAN_TOKEN]",
            "[REDACTED_HUGGINGFACE_TOKEN]",
            "[REDACTED_POSTMAN_KEY]",
        ] {
            assert!(text.contains(marker), "expected {marker} in: {text}");
        }
    }

    #[test]
    fn redaction_catches_high_entropy_secret_without_prefix() {
        // A long, mixed-case+digit, high-entropy token with NO known prefix and NOT assigned to a
        // secret-named key — only the entropy gate can catch it.
        let secret = "Zk7Qp2Lm9Rt4Xy8Bc3Vn6Ws1Dh5Fg0Aj";
        let mut text = format!("rotated the value {secret} yesterday");
        let changed = super::redact_sensitive_text(&mut text);
        assert!(changed, "high-entropy secret should be redacted: {text}");
        assert!(
            !text.contains(secret),
            "the literal must not survive: {text}"
        );
        assert!(text.contains("[REDACTED_HIGH_ENTROPY_SECRET]"));
    }

    #[test]
    fn entropy_gate_does_not_over_redact_hashes_uuids_or_prose() {
        // Precision locks: none of these may be redacted.
        for benign in [
            "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b", // sha1 (lowercase hex, no upper)
            "5d41402abc4b2a76b9719d911017c592",         // md5
            "550e8400-e29b-41d4-a716-446655440000",     // uuid
            "THISISALONGUPPERCASECONSTANTNODIGITS",     // all caps, no digit
            "the quick brown fox jumps over the lazy dog repeatedly", // prose
        ] {
            assert!(
                !super::looks_high_entropy_secret(benign),
                "must NOT flag benign token as secret: {benign}"
            );
            let mut text = format!("context {benign} here");
            super::redact_sensitive_text(&mut text);
            assert!(
                text.contains(benign),
                "benign token was over-redacted: {text}"
            );
        }
    }

    #[test]
    fn assignment_redactor_catches_secret_after_a_benign_field() {
        // Multi-field lines: the secret key follows a benign one. The redactor must key on the
        // identifier ADJACENT to each separator, so the credential is caught (no leak) while the
        // benign leading field is preserved — and prose that merely mentions a secret word after a
        // colon is NOT over-redacted.
        let leaks = [
            (
                "Host: db.internal, password=hunter2plaintextvalue",
                "hunter2plaintextvalue",
            ),
            (
                "name=foo; api_key: AbCdEfGhIjKlMnOpQr",
                "AbCdEfGhIjKlMnOpQr",
            ),
            (
                "remote: gitlab.com, token=hunter2plaintextval",
                "hunter2plaintextval",
            ),
        ];
        for (line, secret) in leaks {
            let (out, changed) = super::redact_text(line);
            assert!(changed, "should redact: {line}");
            assert!(!out.contains(secret), "secret leaked: {out}");
            assert!(out.contains("[REDACTED_SECRET]"), "{out}");
        }
        // Precision: benign leading field preserved; prose mentioning a secret word is untouched.
        let (kept, _) = super::redact_text("name=foo; api_key: AbCdEfGhIjKlMnOpQr");
        assert!(
            kept.contains("name=foo"),
            "benign field must survive: {kept}"
        );
        for benign in [
            "Community theme across 3 entities: Auth Service, JWT Library, Refresh Token Cache.",
            "Keywords: auth, token, refresh, redis",
        ] {
            let (out, _) = super::redact_text(benign);
            assert_eq!(out, benign, "must not over-redact prose: {out}");
        }
    }

    #[test]
    fn shannon_entropy_separates_random_from_repetitive() {
        let random = super::shannon_entropy("Zk7Qp2Lm9Rt4Xy8Bc3Vn6Ws1Dh5Fg0Aj");
        let repetitive = super::shannon_entropy("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
        assert!(
            random > 4.0,
            "random token entropy should be high: {random}"
        );
        assert!(
            repetitive < 0.5,
            "repetitive token entropy should be low: {repetitive}"
        );
        assert_eq!(super::shannon_entropy(""), 0.0);
    }

    #[test]
    fn redact_json_is_key_aware_on_candidate_payloads() {
        // A secret value whose key names a secret but whose value carries no
        // `=`/`:`/token-prefix must still be redacted on the JSON (propose) path —
        // the text passes alone would leave it verbatim.
        let payload = serde_json::json!({
            "client_secret": "abcdef1234567890clientsecretvalue",
            "client_id": "public-123",
            "note": "nothing sensitive here"
        });
        let (redacted, changed) = super::redact_json(&payload);
        assert!(changed);
        assert_eq!(redacted["client_secret"], "[REDACTED_SECRET]");
        // Non-secret keys are left intact.
        assert_eq!(redacted["client_id"], "public-123");
        assert_eq!(redacted["note"], "nothing sensitive here");
        let blob = serde_json::to_string(&redacted).unwrap();
        assert!(!blob.contains("abcdef1234567890clientsecretvalue"));

        // Secret-free payloads round-trip unchanged.
        let clean = serde_json::json!({ "title": "hello", "count": 3 });
        let (_, changed) = super::redact_json(&clean);
        assert!(!changed);
    }

    #[test]
    fn compact_excerpt_handles_multibyte_without_panic() {
        let text = "日本語のテキストです ".repeat(50);
        let out = super::compact_excerpt(&text, 10);
        assert!(out.ends_with("..."));
        assert_eq!(out.chars().count(), 13);
    }

    #[test]
    fn proposed_candidate_payload_and_rationale_are_redacted() {
        let (_temp, home, project_dir) = setup_project();
        propose_candidate(ProposeCandidateOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            source_type: "agent".to_owned(),
            source: Some("test".to_owned()),
            record_type: "observation".to_owned(),
            payload: serde_json::json!({
                "name": "Auth Service",
                "content": "the key is sk-ABCDEFGHIJKLMNOPQRSTUVWX do not share"
            }),
            scope: "example-project/core".to_owned(),
            confidence: 0.6,
            rationale: Some("token ghp_ABCDEFGHIJKLMNOPQRST seen in logs".to_owned()),
            evidence: Vec::new(),
        })
        .unwrap();

        let candidates = list_candidates(ListCandidatesOptions {
            project_name: None,
            start_dir: project_dir,
            grafiki_home: Some(home),
            status: Some("pending".to_owned()),
            scope: "example-project/core".to_owned(),
            limit: 20,
            order: CandidateOrder::Recent,
        })
        .unwrap();
        let dump = serde_json::to_string(&candidates).unwrap();
        assert!(
            !dump.contains("sk-ABCDEFGHIJKLMNOPQRSTUVWX"),
            "payload secret must be redacted before persistence"
        );
        assert!(
            !dump.contains("ghp_ABCDEFGHIJKLMNOPQRST"),
            "rationale secret must be redacted before persistence"
        );
        assert!(dump.contains("REDACTED"));
    }

    #[test]
    fn candidate_and_evidence_insert_roll_back_together() {
        let (_temp, home, project_dir) = setup_project();
        let result = propose_candidate(ProposeCandidateOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            source_type: "agent".to_owned(),
            source: None,
            record_type: "decision".to_owned(),
            payload: serde_json::json!({ "title": "Must not remain" }),
            scope: "example-project/core".to_owned(),
            confidence: 0.5,
            rationale: None,
            evidence: vec![EvidenceInput {
                source_event_id: None,
                source_type: " ".to_owned(),
                source: None,
                title: None,
                excerpt: "invalid evidence".to_owned(),
                uri: None,
                byte_start: None,
                byte_end: None,
                line_start: None,
                line_end: None,
                captured_at: None,
            }],
        });
        assert!(result.is_err());

        let (_project, connection) = resolve_and_open(None, project_dir, Some(home)).unwrap();
        let candidates: i64 = connection
            .query_row("SELECT COUNT(*) FROM extraction_candidates", [], |row| {
                row.get(0)
            })
            .unwrap();
        let evidence: i64 = connection
            .query_row("SELECT COUNT(*) FROM evidence_links", [], |row| row.get(0))
            .unwrap();
        assert_eq!((candidates, evidence), (0, 0));
    }

    #[test]
    fn evidence_order_uses_source_position_not_random_ulid_suffix() {
        let (_temp, home, project_dir) = setup_project();
        let proposed = propose_candidate(ProposeCandidateOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            source_type: "capture:auto".to_owned(),
            source: Some("session-auth".to_owned()),
            record_type: "context".to_owned(),
            payload: serde_json::json!({
                "key": "session-auth",
                "title": "Auth session",
                "content": "Argon2id uses 64 MiB"
            }),
            scope: "example-project/core".to_owned(),
            confidence: 0.8,
            rationale: None,
            evidence: vec![
                EvidenceInput {
                    source_event_id: None,
                    source_type: "transcript".to_owned(),
                    source: Some("session-auth:1".to_owned()),
                    title: None,
                    excerpt: "source fact".to_owned(),
                    uri: None,
                    byte_start: None,
                    byte_end: None,
                    line_start: Some(1),
                    line_end: Some(1),
                    captured_at: Some("2026-01-10T09:00:00Z".to_owned()),
                },
                EvidenceInput {
                    source_event_id: None,
                    source_type: "transcript".to_owned(),
                    source: Some("session-auth:2".to_owned()),
                    title: None,
                    excerpt: "assistant summary".to_owned(),
                    uri: None,
                    byte_start: None,
                    byte_end: None,
                    line_start: Some(2),
                    line_end: Some(2),
                    captured_at: Some("2026-01-10T09:00:00Z".to_owned()),
                },
            ],
        })
        .unwrap();
        let (_project, connection) = resolve_and_open(None, project_dir, Some(home)).unwrap();
        // Force the random-ID order to conflict with source order. The old
        // `ORDER BY created_at, id` path would now return turn 2 first.
        connection
            .execute(
                "UPDATE evidence_links SET id = 'ZZ-source-turn-1'
                 WHERE candidate_id = ?1 AND source = 'session-auth:1'",
                [&proposed.candidate.id],
            )
            .unwrap();
        connection
            .execute(
                "UPDATE evidence_links SET id = 'AA-source-turn-2'
                 WHERE candidate_id = ?1 AND source = 'session-auth:2'",
                [&proposed.candidate.id],
            )
            .unwrap();
        let reloaded =
            super::load_extraction_candidate(&connection, &proposed.candidate.id).unwrap();
        let sources = reloaded
            .evidence
            .iter()
            .filter_map(|evidence| evidence.source.as_deref())
            .collect::<Vec<_>>();
        assert_eq!(sources, vec!["session-auth:1", "session-auth:2"]);
    }

    #[test]
    fn pending_embedding_count_reflects_the_queue() {
        let (_temp, home, project_dir) = setup_project();

        // Fresh project: nothing queued, so the daemon worker will skip (no model
        // load) rather than spin up the embedding provider every poll.
        assert_eq!(
            pending_embedding_count(None, project_dir.clone(), Some(home.clone())).unwrap(),
            0
        );

        // Saving a record enqueues an embedding job → there is now work to do.
        save_entity(SaveEntityOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            name: "Auth Service".to_owned(),
            entity_type: "service".to_owned(),
            observe: Some("JWT refresh uses rotating tokens".to_owned()),
            category: "architecture".to_owned(),
            scope: "example-project/core".to_owned(),
            relate: None,
        })
        .unwrap();
        assert!(
            pending_embedding_count(None, project_dir, Some(home)).unwrap() > 0,
            "saving a record must enqueue embedding work the worker will pick up"
        );
    }

    #[test]
    fn rebuild_revives_failed_embedding_jobs() {
        let (_temp, home, project_dir) = setup_project();
        save_entity(SaveEntityOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            name: "Auth Service".to_owned(),
            entity_type: "service".to_owned(),
            observe: Some("JWT refresh uses rotating tokens".to_owned()),
            category: "architecture".to_owned(),
            scope: "example-project/core".to_owned(),
            relate: None,
        })
        .unwrap();

        {
            let (_project, connection) =
                resolve_and_open(None, project_dir.clone(), Some(home.clone())).unwrap();
            let affected = connection
                .execute(
                    "UPDATE embedding_jobs SET status = 'failed', error = 'boom'",
                    [],
                )
                .unwrap();
            assert!(affected > 0, "expected an embedding job to fail");
        }

        process_embedding_jobs(ProcessEmbeddingsOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            scope: "example-project/core".to_owned(),
            limit: 10,
            rebuild: true,
        })
        .unwrap();

        let (_project, connection) = resolve_and_open(None, project_dir, Some(home)).unwrap();
        let failed: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM embedding_jobs WHERE status = 'failed'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(failed, 0, "rebuild must revive previously-failed jobs");
    }

    #[cfg(feature = "sqlite-vec")]
    #[test]
    fn retiring_a_record_removes_its_sqlite_vec_entry() {
        let (_temp, home, project_dir) = setup_project();
        let saved = save_entity(SaveEntityOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            name: "Vector Target".to_owned(),
            entity_type: "service".to_owned(),
            observe: None,
            category: "general".to_owned(),
            scope: "example-project/core".to_owned(),
            relate: None,
        })
        .unwrap();
        process_embedding_jobs(ProcessEmbeddingsOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            scope: "example-project/core".to_owned(),
            limit: 20,
            rebuild: true,
        })
        .unwrap();
        let (_project, connection) =
            resolve_and_open(None, project_dir.clone(), Some(home.clone())).unwrap();
        let table = super::sqlite_vec_tables(&connection).unwrap().remove(0);
        let before: i64 = connection
            .query_row(
                &format!(
                    "SELECT COUNT(*) FROM {table} WHERE record_type='entity' AND record_id=?1"
                ),
                [&saved.entity_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(before, 1);
        drop(connection);

        delete_entity(DeleteEntityOptions {
            project_name: None,
            start_dir: project_dir.clone(),
            grafiki_home: Some(home.clone()),
            id: saved.entity_id.clone(),
        })
        .unwrap();
        let (_project, connection) = resolve_and_open(None, project_dir, Some(home)).unwrap();
        let after: i64 = connection
            .query_row(
                &format!(
                    "SELECT COUNT(*) FROM {table} WHERE record_type='entity' AND record_id=?1"
                ),
                [&saved.entity_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(after, 0);
    }

    #[cfg(unix)]
    #[test]
    fn home_and_db_have_restricted_permissions() {
        use std::os::unix::fs::PermissionsExt;
        let (_temp, home, _project_dir) = setup_project();
        let home_mode = std::fs::metadata(&home).unwrap().permissions().mode() & 0o777;
        assert_eq!(home_mode, 0o700, "grafiki home should be owner-only");
        let db_mode = std::fs::metadata(home.join("example-project.db"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(db_mode, 0o600, "project database should be owner-only");
    }

    #[test]
    fn export_import_round_trips_state_details_blockers_depends_on() {
        let (temp, home, source_dir) = setup_project();
        upsert_state(UpsertStateOptions {
            project_name: None,
            start_dir: source_dir.clone(),
            grafiki_home: Some(home.clone()),
            key: "migrate-db".to_owned(),
            title: "Migrate DB".to_owned(),
            status: "blocked".to_owned(),
            owner: Some("alice".to_owned()),
            details: Some("waiting on schema sign-off".to_owned()),
            blockers: vec!["schema-review".to_owned(), "staging-access".to_owned()],
            depends_on: vec!["auth-service".to_owned()],
            scope: "example-project/core".to_owned(),
            priority: "high".to_owned(),
        })
        .unwrap();

        let bundle = export_memory(ExportOptions {
            project_name: None,
            start_dir: source_dir,
            grafiki_home: Some(home.clone()),
            scope: "example-project/core".to_owned(),
        })
        .unwrap();
        let exported = bundle.state.iter().find(|s| s.key == "migrate-db").unwrap();
        assert_eq!(
            exported.details.as_deref(),
            Some("waiting on schema sign-off")
        );
        assert_eq!(exported.blockers, vec!["schema-review", "staging-access"]);
        assert_eq!(exported.depends_on, vec!["auth-service"]);

        let target_dir = temp.path().join("target-project");
        init_project(InitOptions {
            project_name: None,
            project_dir: target_dir.clone(),
            grafiki_home: Some(home.clone()),
        })
        .unwrap();
        import_memory(ImportOptions {
            project_name: None,
            start_dir: target_dir.clone(),
            grafiki_home: Some(home.clone()),
            bundle,
        })
        .unwrap();

        let imported = list_state(StateListOptions {
            project_name: None,
            start_dir: target_dir,
            grafiki_home: Some(home),
            status: None,
            scope: "example-project/core".to_owned(),
        })
        .unwrap();
        let item = imported.iter().find(|s| s.key == "migrate-db").unwrap();
        assert_eq!(item.details.as_deref(), Some("waiting on schema sign-off"));
        assert_eq!(item.blockers, vec!["schema-review", "staging-access"]);
        assert_eq!(item.depends_on, vec!["auth-service"]);
    }

    #[test]
    fn export_import_round_trips_context_sessions_and_supersession() {
        let (temp, home, source_dir) = setup_project();
        start_codex(home.clone(), source_dir.clone());
        handoff_session(HandoffOptions {
            project_name: None,
            start_dir: source_dir.clone(),
            grafiki_home: Some(home.clone()),
            session_id: None,
        })
        .unwrap();
        add_context(AddContextOptions {
            project_name: None,
            start_dir: source_dir.clone(),
            grafiki_home: Some(home.clone()),
            key: "auth-guide".to_owned(),
            title: "Auth Guide".to_owned(),
            category: "guide".to_owned(),
            scope: "example-project/core".to_owned(),
            content: "Rotating refresh tokens prevent replay.".to_owned(),
        })
        .unwrap();
        let first = log_decision(LogDecisionOptions {
            project_name: None,
            start_dir: source_dir.clone(),
            grafiki_home: Some(home.clone()),
            title: "Use rollback journal".to_owned(),
            reasoning: Some("initial".to_owned()),
            alternatives: vec![],
            tags: vec![],
            scope: "example-project/core".to_owned(),
            supersedes: None,
        })
        .unwrap();
        log_decision(LogDecisionOptions {
            project_name: None,
            start_dir: source_dir.clone(),
            grafiki_home: Some(home.clone()),
            title: "Use WAL".to_owned(),
            reasoning: Some("better concurrency".to_owned()),
            alternatives: vec![],
            tags: vec![],
            scope: "example-project/core".to_owned(),
            supersedes: Some(first.decision_id.clone()),
        })
        .unwrap();

        let bundle = export_memory(ExportOptions {
            project_name: None,
            start_dir: source_dir,
            grafiki_home: Some(home.clone()),
            scope: "example-project/core".to_owned(),
        })
        .unwrap();
        assert!(bundle
            .context
            .iter()
            .any(|c| c.content.contains("Rotating")));
        assert!(bundle.sessions.len() >= 2);
        assert!(bundle.decisions.iter().any(|d| d.superseded_by.is_some()));

        let target_dir = temp.path().join("target-project");
        init_project(InitOptions {
            project_name: None,
            project_dir: target_dir.clone(),
            grafiki_home: Some(home.clone()),
        })
        .unwrap();
        let report = import_memory(ImportOptions {
            project_name: None,
            start_dir: target_dir.clone(),
            grafiki_home: Some(home.clone()),
            bundle,
        })
        .unwrap();
        assert!(report.context >= 1, "context must be imported");
        assert!(report.sessions >= 2, "sessions must be imported");

        // Re-export from the target project to confirm a faithful round-trip.
        let round = export_memory(ExportOptions {
            project_name: None,
            start_dir: target_dir,
            grafiki_home: Some(home),
            scope: "example-project/core".to_owned(),
        })
        .unwrap();
        assert!(
            round.context.iter().any(|c| c.content.contains("Rotating")),
            "context content must survive the round-trip"
        );
        assert!(
            round.sessions.len() >= 2,
            "sessions must survive the round-trip"
        );
        assert!(
            round.decisions.iter().any(|d| d.superseded_by.is_some()),
            "decision supersession must survive the round-trip"
        );
    }
}
