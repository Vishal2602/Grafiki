import type { ExtractionCandidate, ProjectSnapshot, SearchResult } from "./types";

// Browser-preview fixtures are intentionally isolated in this lazy chunk. The
// packaged Tauri app never downloads or evaluates fabricated project data.
export const mockCandidates: ExtractionCandidate[] = [
  {
    id: "01JCANDIDATE001",
    source_type: "assistant",
    source: "desktop-session",
    record_type: "decision",
    payload: {
      title: "Candidate review stays separate from trusted memory",
      reasoning: "Extracted memory should be reviewed before it becomes durable project truth.",
      status: "active",
      tags: ["desktop", "trust"],
    },
    scope: "grafiki/desktop",
    confidence: 0.91,
    status: "pending",
    rationale: "Repeated in a handoff and implementation notes.",
    trusted_record_type: null,
    trusted_record_id: null,
    created_at: new Date(Date.now() - 1000 * 60 * 20).toISOString(),
    reviewed_at: null,
  },
  {
    id: "01JCANDIDATE002",
    source_type: "import",
    source: "preview-json",
    record_type: "context",
    payload: {
      key: "desktop-review-workflow",
      title: "Desktop review workflow",
      category: "architecture",
      content: "Grafiki should surface candidate memory as a review queue before approval.",
    },
    scope: "grafiki/desktop",
    confidence: 0.84,
    status: "pending",
    rationale: "Useful but not yet promoted into trusted context.",
    trusted_record_type: null,
    trusted_record_id: null,
    created_at: new Date(Date.now() - 1000 * 60 * 42).toISOString(),
    reviewed_at: null,
  },
];

export const mockSearchResults: SearchResult[] = [
  {
    record_type: "decision",
    id: "01JDESKTOP001",
    title: "Desktop shell is a memory console",
    snippet:
      "Grafiki Desktop opens into a working console with panes for search, graph, sessions, decisions, context, and settings.",
    scope: "grafiki/desktop",
    score: 0.94,
  },
  {
    record_type: "context",
    id: "01JDESKTOP002",
    title: "URL-synced pane layout",
    snippet:
      "Pane state is encoded into the route so layouts can be restored, shared, bookmarked, and debugged.",
    scope: "grafiki/desktop",
    score: 0.9,
  },
  {
    record_type: "state",
    id: "01JDESKTOP003",
    title: "Retrieval quality completed",
    snippet:
      "Hybrid search now exposes scores, embedding freshness, provider metadata, and larger topic-separation fixtures.",
    scope: "grafiki/search",
    score: 0.86,
  },
];

export const mockSnapshot: ProjectSnapshot = {
  start_dir: "/path/to/your/project",
  scope: "",
  memory_available: true,
  project: {
    project: "Grafiki",
    project_dir: "/path/to/your/project",
    db_path: "~/.grafiki/Preview.db",
    marker_path: "/path/to/your/project/.grafiki",
  },
  status: {
    project: "Grafiki",
    scope: "",
    active_sessions: ["desktop-foundation"],
    active_state: ["Build Tauri shell", "Wire pane manager"],
    recent_decisions: ["Macro-inspired, AI-memory-only desktop"],
    recent_events: ["Desktop plan added", "Retrieval quality completed"],
  },
  report: {
    project: "Grafiki",
    scope: "",
    entity_count: 38,
    relation_count: 64,
    observation_count: 147,
    decision_count: 12,
    active_session_count: 1,
    god_nodes: [
      { id: "grafiki", name: "Grafiki", entity_type: "concept", scope: "grafiki", degree: 8 },
      { id: "desktop", name: "Desktop", entity_type: "module", scope: "grafiki/desktop", degree: 5 },
    ],
    orphan_entities: [
      { id: "retrieval", name: "Retrieval", entity_type: "module", scope: "grafiki/search", degree: 0 },
    ],
    suggested_queries: [
      "What should a new AI session know?",
      "Which decisions affect desktop architecture?",
      "What context is stale?",
    ],
  },
  embedding: {
    project: "Grafiki",
    scope: "",
    runtime: {
      requested_provider: "auto",
      provider: "deterministic",
      model: "deterministic-test",
      dimension: 64,
      vector_backend: "sqlite-vec",
      embeddable_records: 147,
      indexed_records: 142,
      fresh_records: 139,
      missing_or_stale_records: 8,
      note: null,
    },
    pending: 3,
    embedded: 142,
    failed: 0,
    skipped: 2,
  },
  error: null,
};
