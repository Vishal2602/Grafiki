// These lints fire on a few intentional, pre-existing API shapes (wide option
// structs, a large report enum variant, &PathBuf in signatures). Refactoring
// them is churn without benefit, so allow them crate-wide to keep `clippy
// -D warnings` green in CI.
#![allow(
    clippy::too_many_arguments,
    clippy::large_enum_variant,
    clippy::ptr_arg
)]

pub mod confidence;
pub mod conflict;
pub mod db;
pub mod decay;
pub mod embeddings;
pub mod error;
pub mod graph;
pub mod memory;
pub mod project;
pub mod reflection;
pub mod scope;
pub mod session;
pub mod transcript;
pub mod ulid;

pub use conflict::{
    arbitrate, attribute_cardinality, key_conflict, slot_conflict, temporal_relation,
    ArbitrationBasis, Cardinality, ConflictVerdict, FactMeta, Slot, TemporalRelation, Winner,
};
pub use error::{GrafikiError, Result};
pub use graph::{community_modularity, detect_communities, Community};
pub use memory::{
    add_context, approve_candidate, ask_memory, bulk_review_candidates, delete_context,
    delete_decision, delete_entity, delete_observation, delete_relation, delete_state,
    edit_candidate, end_session, export_memory, generate_report, get_capture_status, get_context,
    get_embedding_status, get_graph, get_memory_record_detail, get_status, handoff_session,
    import_memory, ingest_capture_event, list_agent_queries, list_candidates, list_capture_events,
    list_context, list_decisions, list_entities, list_events, list_observations, list_relations,
    list_sessions, list_state, log_decision, process_embedding_jobs, propose_candidate,
    propose_capture_candidates, redact_json, redact_text, reject_candidate, run_reflection,
    save_entity, search_memory, start_capture_session, stop_capture_session, update_context,
    update_decision, update_entity, update_observation, update_relation, update_session,
    upsert_state, AddContextOptions, AgentMemoryBriefing, AgentQueryLogItem,
    ApproveCandidateOptions, AskMemoryOptions, BulkCandidateReviewOptions,
    BulkCandidateReviewReport, CandidateMutationReport, CandidateOrder, CandidateReviewError,
    CaptureCandidateReport, CaptureEvent, CaptureEventReport, CaptureSession, CaptureSessionReport,
    CaptureStatusOptions, CaptureStatusReport, ContextDocument, ContextListOptions, ContextReport,
    ContextSummary, DecisionItem, DecisionListOptions, DecisionReport, DeleteContextOptions,
    DeleteDecisionOptions, DeleteEntityOptions, DeleteObservationOptions, DeleteRelationOptions,
    DeleteStateOptions, DetailEvent, DetailMetadata, EditCandidateOptions,
    EmbeddingMetadataSummary, EmbeddingRuntimeSummary, EmbeddingStatusOptions,
    EmbeddingStatusReport, EndSessionOptions, EndSessionReport, EntityListOptions, EventItem,
    EventListOptions, EventListReport, EvidenceInput, EvidenceLink, ExportBundle, ExportDecision,
    ExportObservation, ExportOptions, ExtractionCandidate, GetContextOptions,
    GetMemoryRecordOptions, GraphEntity, GraphOptions, GraphRelation, GraphReport, HandoffOptions,
    HandoffReport, ImportOptions, ImportReport, IngestCaptureEventOptions, ListAgentQueriesOptions,
    ListCandidatesOptions, ListCaptureEventsOptions, LogDecisionOptions, MemoryRecordDetail,
    NodeDegree, ObservationItem, ObservationListOptions, ProcessEmbeddingsOptions,
    ProcessEmbeddingsReport, ProjectReport, ProjectReportOptions, ProposeCandidateOptions,
    ProposeCaptureCandidatesOptions, RejectCandidateOptions, RelatedMemoryRecord,
    RelationListOptions, SaveEntityOptions, SaveEntityReport, SearchMemoryOptions, SearchMode,
    SearchReport, SearchResult, SessionLogItem, SessionLogOptions, SessionLogReport,
    StartCaptureOptions, StateItem, StateListOptions, StateReport, StatusOptions, StatusReport,
    StopCaptureOptions, UpdateContextOptions, UpdateDecisionOptions, UpdateEntityOptions,
    UpdateObservationOptions, UpdateRelationOptions, UpdateSessionOptions, UpsertStateOptions,
};
pub use project::{
    init_project, load_capture_config, resolve_project, update_capture_config, CaptureConfig,
    CaptureConfigOptions, CaptureConfigReport, CaptureSourceConfig, CaptureSourceUpdates,
    InitImportedFile, InitOptions, InitReport, ProjectContext, ProjectResolveOptions,
    UpdateCaptureConfigOptions,
};
pub use reflection::{CommunityDetail, ReflectionReport, RunReflectionOptions};
pub use scope::{Scope, ScopeChain};
pub use session::{start_session, StartSessionOptions, StartSessionReport};
pub use transcript::{
    import_agent_transcripts, AgentTranscriptImportReport, ImportAgentTranscriptsOptions,
    TranscriptImportSource,
};
