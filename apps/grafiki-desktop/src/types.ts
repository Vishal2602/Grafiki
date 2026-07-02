export type PaneKind =
  | "home"
  | "chat"
  | "terminal"
  | "candidates"
  | "settings"
  | "detail";

export type SearchMode = "keyword" | "semantic" | "hybrid";

export type CaptureType = "decision" | "observation" | "state" | "context" | "handoff" | "relation";

export interface PaneState {
  id: string;
  kind: PaneKind;
  title: string;
  query?: string;
  mode?: SearchMode;
  scope?: string;
  entityId?: string;
  recordId?: string;
  recordType?: string;
  captureType?: CaptureType;
}

export interface LayoutState {
  activePaneId: string;
  panes: PaneState[];
}

export interface ProjectMeta {
  project: string;
  project_dir: string;
  db_path: string;
  marker_path?: string | null;
}

export interface ProjectReport {
  project: string;
  scope: string;
  entity_count: number;
  relation_count: number;
  observation_count: number;
  decision_count: number;
  active_session_count: number;
  god_nodes: NodeDegree[];
  orphan_entities: NodeDegree[];
  suggested_queries: string[];
}

export interface NodeDegree {
  id: string;
  name: string;
  entity_type: string;
  scope: string;
  degree: number;
}

export interface StatusReport {
  project: string;
  scope: string;
  active_sessions: string[];
  active_state: string[];
  recent_decisions: string[];
  recent_events: string[];
}

export interface EmbeddingStatusReport {
  project: string;
  scope: string;
  runtime: {
    requested_provider: string;
    provider: string;
    model: string;
    dimension?: number | null;
    vector_backend: string;
    embeddable_records: number;
    indexed_records: number;
    fresh_records: number;
    missing_or_stale_records: number;
    note?: string | null;
  };
  pending: number;
  embedded: number;
  failed: number;
  skipped: number;
}

export interface ProjectSnapshot {
  start_dir: string;
  scope: string;
  memory_available: boolean;
  project?: ProjectMeta | null;
  status?: StatusReport | null;
  report?: ProjectReport | null;
  embedding?: EmbeddingStatusReport | null;
  error?: string | null;
}

export interface SearchResult {
  record_type: string;
  id: string;
  title: string;
  snippet: string;
  scope: string;
  score?: number | null;
  evidence?: EvidenceLink[];
}

export interface EvidenceLink {
  id: string;
  candidate_id?: string | null;
  trusted_record_type?: string | null;
  trusted_record_id?: string | null;
  source_event_id?: string | null;
  source_type: string;
  source?: string | null;
  title?: string | null;
  excerpt: string;
  uri?: string | null;
  byte_start?: number | null;
  byte_end?: number | null;
  line_start?: number | null;
  line_end?: number | null;
  captured_at?: string | null;
  created_at: string;
}

export interface AgentQueryLogItem {
  id: string;
  agent: string;
  question: string;
  scope: string;
  returned_ids: string[];
  retrieval_mode: string;
  fallback?: string | null;
  latency_ms: number;
  created_at: string;
}

export interface SearchReport {
  project: string;
  query: string;
  mode: SearchMode;
  semantic_available: boolean;
  fallback?: string | null;
  results: SearchResult[];
}

export interface ChatCitation {
  index: number;
  record_type: string;
  id: string;
  title: string;
  snippet: string;
}

export interface ChatReply {
  question: string;
  scope: string;
  answer: string;
  citations: ChatCitation[];
  used_memory: boolean;
  flagged_injection?: boolean;
}

export interface GraphEntity {
  id: string;
  name: string;
  entity_type: string;
  scope: string;
}

export interface GraphRelation {
  id: string;
  from_entity: string;
  to_entity: string;
  relation: string;
  weight: number;
  confidence: number;
  source_type: string;
  source?: string | null;
}

export interface GraphReport {
  project: string;
  root: string;
  depth: number;
  entities: GraphEntity[];
  relations: GraphRelation[];
}

export interface DetailMetadata {
  label: string;
  value: string;
}

export interface RelatedMemoryRecord {
  record_type: string;
  id: string;
  title: string;
  relation: string;
}

export interface DetailEvent {
  id: string;
  event_type: string;
  summary: string;
  created_at: string;
}

export interface MemoryRecordDetail {
  record_type: string;
  id: string;
  title: string;
  scope: string;
  body: string;
  metadata: DetailMetadata[];
  related: RelatedMemoryRecord[];
  events: DetailEvent[];
  focus_entity_id?: string | null;
}

export interface ContextSummary {
  key: string;
  title: string;
  category: string;
  scope: string;
  version: number;
}

export interface StateItem {
  key: string;
  title: string;
  status: string;
  priority: string;
  owner?: string | null;
  scope: string;
}

export interface DecisionItem {
  id: string;
  title: string;
  status: string;
  scope: string;
  reasoning?: string | null;
}

export interface SessionLogItem {
  id: string;
  session_type: string;
  status: string;
  scope: string;
  goal?: string | null;
  summary?: string | null;
  accomplishments: string[];
  remaining: string[];
  files_changed: string[];
  decisions_made: string[];
  entities_touched: string[];
  handoff_context?: string | null;
  parent_session?: string | null;
  child_session?: string | null;
  started_at: string;
  ended_at?: string | null;
}

export type CandidateRecordType = "entity" | "observation" | "decision" | "context" | "state";
export type CandidateStatus = "pending" | "approved" | "rejected";

export interface ExtractionCandidate {
  id: string;
  source_type: string;
  source?: string | null;
  record_type: CandidateRecordType;
  payload: Record<string, unknown>;
  scope: string;
  confidence: number;
  status: CandidateStatus;
  rationale?: string | null;
  trusted_record_type?: string | null;
  trusted_record_id?: string | null;
  created_at: string;
  reviewed_at?: string | null;
  evidence?: EvidenceLink[];
}

export interface CandidateMutationResult {
  candidate: ExtractionCandidate;
  message: string;
}

export interface CandidateReviewError {
  id: string;
  error: string;
}

export interface BulkCandidateReviewResult {
  action: "approve" | "reject";
  requested: number;
  succeeded: number;
  failed: number;
  results: CandidateMutationResult[];
  errors: CandidateReviewError[];
}

export interface AutoCaptureInput {
  startDir?: string;
  scope?: string;
  source?: string;
  limit?: number;
}

export interface AutoCaptureResult {
  scope: string;
  source: string;
  path: string;
  git_root?: string | null;
  changed_files: string[];
  staged_files: string[];
  unstaged_files: string[];
  untracked_files: string[];
  diff_stat: string;
  last_commit?: string | null;
  candidates: CandidateMutationResult[];
  message: string;
}

export interface RawCaptureSession {
  id: string;
  project: string;
  scope: string;
  status: string;
  source_app?: string | null;
  consent_profile: string;
  redaction_profile: string;
  started_at: string;
  ended_at?: string | null;
}

export interface RawCaptureEvent {
  id: string;
  capture_session: string;
  source_type: string;
  source?: string | null;
  title?: string | null;
  text?: string | null;
  payload?: Record<string, unknown> | null;
  metadata?: Record<string, unknown> | null;
  privacy_level: string;
  redacted: boolean;
  scope: string;
  captured_at: string;
  created_at: string;
}

export interface RawCaptureSessionResult {
  capture: RawCaptureSession;
  message: string;
}

export interface RawCaptureEventResult {
  event: RawCaptureEvent;
  message: string;
}

export interface RawCaptureStatus {
  project: string;
  scope: string;
  active_sessions: RawCaptureSession[];
  recent_events: RawCaptureEvent[];
  event_count: number;
}

export interface RawCaptureCandidateResult {
  capture_id?: string | null;
  events_summarized: number;
  candidates: CandidateMutationResult[];
  message: string;
}

export interface CaptureSourceConfig {
  git: boolean;
  transcripts: boolean;
  terminal: boolean;
  files: boolean;
  ide: boolean;
  screen: boolean;
  browser: boolean;
  audio: boolean;
  system: boolean;
}

export interface CaptureConfig {
  version: number;
  sources: CaptureSourceConfig;
  blocked_paths: string[];
  blocked_apps: string[];
  redaction_profile: string;
  terminal_output: "off" | "digest" | "full";
  screen_policy: "off" | "manual" | "allowlist";
  browser_policy: "off" | "allowlist";
}

export interface CaptureConfigReport {
  project: string;
  project_dir: string;
  config_path: string;
  created: boolean;
  config: CaptureConfig;
}

export interface CaptureConfigUpdateInput {
  startDir?: string;
  git?: boolean;
  transcripts?: boolean;
  terminal?: boolean;
  files?: boolean;
  ide?: boolean;
  screen?: boolean;
  browser?: boolean;
  audio?: boolean;
  system?: boolean;
  addBlockedPaths?: string[];
  removeBlockedPaths?: string[];
  addBlockedApps?: string[];
  removeBlockedApps?: string[];
  redactionProfile?: string;
  terminalOutput?: "off" | "digest" | "full";
  screenPolicy?: "off" | "manual" | "allowlist";
  browserPolicy?: "off" | "allowlist";
}

export interface AgentTranscriptImportInput {
  startDir?: string;
  scope?: string;
  agent: "codex" | "claude-code" | "cursor" | "generic";
  input?: string;
  limit?: number;
  summarize?: boolean;
}

export interface AgentTranscriptImportSource {
  path: string;
  events: number;
  skipped?: string | null;
}

export interface AgentTranscriptImportResult {
  agent: string;
  scope: string;
  capture_id: string;
  files_scanned: number;
  files_imported: number;
  events_imported: number;
  sources: AgentTranscriptImportSource[];
  candidates?: RawCaptureCandidateResult | null;
  message: string;
}

export interface CaptureMemoryInput {
  startDir?: string;
  captureType: CaptureType;
  title: string;
  scope?: string;
  content?: string;
  key?: string;
  entityType?: string;
  category?: string;
  status?: string;
  priority?: string;
  relationTarget?: string;
  relationType?: string;
  tags?: string;
  alternatives?: string;
  supersedes?: string;
}

export interface CaptureMemoryResult {
  record_type: string;
  id: string;
  title: string;
  scope: string;
  message: string;
}

export interface DeleteMemoryResult {
  record_type: string;
  id: string;
  title: string;
  scope: string;
  message: string;
}

export interface UpdateMemoryInput {
  startDir?: string;
  recordType: "context" | "state" | "decision" | "entity" | "observation" | "relation" | "session";
  id: string;
  title?: string;
  scope?: string;
  content?: string;
  category?: string;
  entityType?: string;
  status?: string;
  priority?: string;
  relation?: string;
  weight?: number;
  confidence?: number;
  sourceType?: string;
  source?: string;
  sessionType?: string;
  goal?: string;
  summary?: string;
  accomplishments?: string;
  remaining?: string;
  filesChanged?: string;
}

export interface UpdateMemoryResult {
  record_type: string;
  id: string;
  title: string;
  scope: string;
  message: string;
}

export interface ExportFileResult {
  output_path: string;
  records: number;
  message: string;
}

export interface ImportMemoryResult {
  project: string;
  source_project: string;
  entities: number;
  relations: number;
  skipped_relations: number;
  observations: number;
  decisions: number;
  state: number;
  context_skipped: number;
  sessions_skipped: number;
}

export interface ProcessEmbeddingsResult {
  project: string;
  scope: string;
  provider: string;
  model: string;
  dimension: number;
  enqueued: number;
  processed: number;
  skipped: number;
  failed: number;
  pending_remaining: number;
}

export interface DaemonStatus {
  project: string;
  running: boolean;
  pid?: number | null;
  host?: string | null;
  port?: number | null;
  url?: string | null;
  pid_path: string;
  log_path: string;
  cli_path?: string | null;
  cli_available: boolean;
  token?: string | null;
  message: string;
}

export interface DaemonStartResult {
  project: string;
  running: boolean;
  already_running: boolean;
  pid: number;
  host: string;
  port: number;
  url: string;
  pid_path: string;
  log_path: string;
  cli_path: string;
  token?: string | null;
  message: string;
}

export interface DaemonStopResult {
  project: string;
  stopped: boolean;
  pid?: number | null;
  pid_path: string;
  cli_path: string;
  message: string;
}

export interface InitProjectResult {
  project: string;
  project_dir: string;
  db_path: string;
  marker_path: string;
  imported_files?: Array<{ path: string; source_type: string; candidate_id: string }>;
  proposed_candidates?: number;
  trusted_records?: number;
  skipped_sources?: string[];
  decisions_found?: number;
  rules_found?: number;
  next_agent_setup?: string;
}

export interface StartSessionInput {
  startDir?: string;
  sessionType: string;
  goal: string;
  scope?: string;
}

export interface StartSessionResult {
  session_id: string;
  project: string;
  session_type: string;
  goal: string;
  scope: string;
  briefing: string;
}

export interface EndSessionInput {
  startDir?: string;
  sessionId?: string;
  status: string;
  summary?: string;
  accomplishments?: string;
  remaining?: string;
  filesChanged?: string;
}

export interface EndSessionResult {
  session_id: string;
  project: string;
  status: string;
  summary?: string | null;
}

export interface HandoffSessionInput {
  startDir?: string;
  sessionId?: string;
}

export interface HandoffSessionResult {
  parent_session_id: string;
  child_session_id: string;
  project: string;
  scope: string;
  handoff_context: string;
}
