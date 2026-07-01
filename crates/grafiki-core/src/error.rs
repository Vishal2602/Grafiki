#[derive(thiserror::Error, Debug)]
pub enum GrafikiError {
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("Invalid scope format: {0}")]
    InvalidScope(String),

    #[error("Invalid project name: {0}")]
    InvalidProjectName(String),

    #[error("Invalid session type: {0}")]
    InvalidSessionType(String),

    #[error("Invalid entity type: {0}")]
    InvalidEntityType(String),

    #[error("Invalid observation category: {0}")]
    InvalidObservationCategory(String),

    #[error("Invalid relation type: {0}")]
    InvalidRelationType(String),

    #[error("Invalid relation source type: {0}")]
    InvalidRelationSourceType(String),

    #[error("Invalid relation confidence: {0}")]
    InvalidRelationConfidence(f64),

    #[error("Invalid context category: {0}")]
    InvalidContextCategory(String),

    #[error("Invalid session status: {0}")]
    InvalidSessionStatus(String),

    #[error("Invalid state status: {0}")]
    InvalidStateStatus(String),

    #[error("Invalid state priority: {0}")]
    InvalidStatePriority(String),

    #[error("Invalid search mode: {0}")]
    InvalidSearchMode(String),

    #[error("Invalid record type: {0}")]
    InvalidRecordType(String),

    #[error("Invalid decision status: {0}")]
    InvalidDecisionStatus(String),

    #[error("Invalid candidate memory: {0}")]
    InvalidCandidate(String),

    #[error("Invalid capture config: {0}")]
    InvalidCaptureConfig(String),

    #[error("Embedding error: {0}")]
    Embedding(String),

    #[error("Code indexing error: {0}")]
    CodeIndex(String),

    #[error("Chat model error: {0}")]
    Chat(String),

    #[error("Entity not found: {0}")]
    EntityNotFound(String),

    #[error("Session not found: {0}")]
    SessionNotFound(String),

    #[error("Context key not found: {0}")]
    ContextNotFound(String),

    #[error("State key not found: {0}")]
    StateNotFound(String),

    #[error("Decision not found: {0}")]
    DecisionNotFound(String),

    #[error("Observation not found: {0}")]
    ObservationNotFound(String),

    #[error("Relation not found: {0}")]
    RelationNotFound(String),

    #[error("Candidate not found: {0}")]
    CandidateNotFound(String),

    #[error("Context content is required. Pass --content or --file.")]
    MissingContextContent,

    #[error("No active session. Run 'grafiki start' first or pass --session.")]
    NoActiveSession,

    #[error("Could not determine home directory. Set GRAFIKI_HOME or HOME.")]
    MissingHomeDir,

    #[error("Project not initialized: {0}. Run 'grafiki init' first.")]
    ProjectNotInitialized(String),

    #[error(
        "Database schema version {found} is newer than this Grafiki build supports ({supported}). \
         Upgrade Grafiki to open this project."
    )]
    SchemaVersionTooNew { found: i64, supported: i64 },
}

pub type Result<T> = std::result::Result<T, GrafikiError>;
