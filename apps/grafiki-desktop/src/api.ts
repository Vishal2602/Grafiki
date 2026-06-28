import { open, save, confirm as tauriConfirm } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";
import type {
  AgentTranscriptImportInput,
  AgentTranscriptImportResult,
  AutoCaptureInput,
  AutoCaptureResult,
  AgentQueryLogItem,
  BulkCandidateReviewResult,
  CandidateMutationResult,
  CaptureConfigReport,
  CaptureConfigUpdateInput,
  CaptureMemoryInput,
  CaptureMemoryResult,
  ContextSummary,
  DaemonStartResult,
  DaemonStatus,
  DaemonStopResult,
  DecisionItem,
  DeleteMemoryResult,
  EndSessionInput,
  EndSessionResult,
  ExportFileResult,
  ExtractionCandidate,
  GraphRelation,
  GraphReport,
  HandoffSessionInput,
  HandoffSessionResult,
  ImportMemoryResult,
  InitProjectResult,
  MemoryRecordDetail,
  ProcessEmbeddingsResult,
  ProjectSnapshot,
  RawCaptureCandidateResult,
  RawCaptureEvent,
  RawCaptureEventResult,
  RawCaptureSessionResult,
  RawCaptureStatus,
  SearchMode,
  SearchReport,
  SessionLogItem,
  StateItem,
  StartSessionInput,
  StartSessionResult,
  UpdateMemoryInput,
  UpdateMemoryResult,
} from "./types";

const hasTauri = () => "__TAURI_INTERNALS__" in window;

/// True when running outside the Tauri shell (e.g. a plain browser), where the
/// API returns mock data and mutations are not persisted. The UI surfaces this
/// so demos/QA cannot mistake preview behavior for a working backend.
export const isPreviewMode = () => !hasTauri();

/// A reliable yes/no confirmation. Uses the Tauri dialog plugin inside the app
/// (window.confirm can be suppressed by the webview) and falls back to
/// window.confirm in browser preview. Returns true when the user confirms.
export async function confirmDialog(
  message: string,
  opts?: { title?: string; kind?: "info" | "warning" | "error"; okLabel?: string },
): Promise<boolean> {
  if (!hasTauri()) {
    return window.confirm(message);
  }
  return tauriConfirm(message, {
    title: opts?.title ?? "Grafiki",
    kind: opts?.kind ?? "warning",
    okLabel: opts?.okLabel,
  });
}

export async function getProjectSnapshot(input: { startDir?: string; scope?: string } = {}): Promise<ProjectSnapshot> {
  if (!hasTauri()) return mockSnapshot;

  try {
    return await invoke<ProjectSnapshot>("get_project_snapshot", {
      request: { startDir: input.startDir ?? "", scope: input.scope ?? "" },
    });
  } catch (error) {
    return {
      ...mockSnapshot,
      memory_available: false,
      error: String(error),
    };
  }
}

export async function searchProjectMemory(input: {
  startDir?: string;
  query: string;
  mode: SearchMode;
  scope?: string;
  recordType?: string;
  limit?: number;
}): Promise<SearchReport> {
  if (!hasTauri()) {
    const recordType = input.recordType ?? "all";
    return {
      project: "Grafiki",
      query: input.query,
      mode: input.mode,
      semantic_available: true,
      results: mockSearchResults.filter((result) =>
        (recordType === "all" || result.record_type === recordType) &&
        `${result.title} ${result.snippet} ${result.record_type}`.toLowerCase().includes(input.query.toLowerCase()),
      ),
    };
  }

  return invoke<SearchReport>("search_project_memory", {
    request: {
      query: input.query,
      mode: input.mode,
      startDir: input.startDir ?? "",
      scope: input.scope ?? "",
      limit: input.limit ?? 20,
      recordType: input.recordType ?? "all",
    },
  });
}

export async function getMemoryGraph(input: {
  startDir?: string;
  entityId: string;
  depth?: number;
}): Promise<GraphReport> {
  if (!hasTauri()) {
    return {
      project: "Grafiki",
      root: input.entityId,
      depth: input.depth ?? 2,
      entities: [
        { id: "grafiki", name: "Grafiki", entity_type: "concept", scope: "grafiki" },
        { id: "desktop", name: "Desktop", entity_type: "module", scope: "grafiki/desktop" },
        { id: "retrieval", name: "Retrieval", entity_type: "module", scope: "grafiki/search" },
      ],
      relations: [
        {
          id: "mock-relation-1",
          from_entity: "grafiki",
          to_entity: "desktop",
          relation: "uses",
          weight: 1,
          confidence: 1,
          source_type: "mock",
          source: null,
        },
        {
          id: "mock-relation-2",
          from_entity: "desktop",
          to_entity: "retrieval",
          relation: "depends_on",
          weight: 1,
          confidence: 1,
          source_type: "mock",
          source: null,
        },
      ],
    };
  }

  return invoke<GraphReport>("get_memory_graph", {
    request: {
      entityId: input.entityId,
      startDir: input.startDir ?? "",
      depth: input.depth ?? 2,
    },
  });
}

export async function getMemoryRecord(input: {
  startDir?: string;
  recordType: string;
  id: string;
  scope?: string;
}): Promise<MemoryRecordDetail> {
  if (!hasTauri()) {
    return {
      record_type: input.recordType,
      id: input.id,
      title:
        mockSearchResults.find((result) => result.id === input.id)?.title ??
        `${input.recordType} ${input.id}`,
      scope: input.scope || "grafiki/desktop",
      body:
        mockSearchResults.find((result) => result.id === input.id)?.snippet ??
        "Preview detail body. In the desktop shell this comes from Grafiki core.",
      metadata: [
        { label: "source", value: "preview" },
        { label: "scope", value: input.scope || "grafiki/desktop" },
      ],
      related: [
        { record_type: "entity", id: "grafiki", title: "Grafiki", relation: "belongs_to" },
        { record_type: "entity", id: "desktop", title: "Desktop", relation: "mentions" },
      ],
      events: [
        {
          id: "preview-event",
          event_type: "memory_previewed",
          summary: "Loaded preview memory detail.",
          created_at: new Date().toISOString(),
        },
      ],
      focus_entity_id: input.recordType === "entity" ? input.id : "grafiki",
    };
  }

  return invoke<MemoryRecordDetail>("get_memory_record", {
    request: {
      recordType: input.recordType,
      id: input.id,
      startDir: input.startDir ?? "",
      scope: input.scope ?? "",
    },
  });
}

export async function listProjectContext(input: {
  startDir?: string;
  scope?: string;
  category?: string;
} = {}): Promise<ContextSummary[]> {
  if (!hasTauri()) {
    return [
      {
        key: "desktop-plan",
        title: "Desktop App Plan",
        category: "architecture",
        scope: "grafiki/desktop",
        version: 1,
      },
      {
        key: "retrieval-quality",
        title: "Retrieval Quality Notes",
        category: "reference",
        scope: "grafiki/search",
        version: 2,
      },
    ];
  }

  return invoke<ContextSummary[]>("list_project_context", {
    request: {
      startDir: input.startDir ?? "",
      scope: input.scope ?? "",
      category: input.category ?? "",
    },
  });
}

export async function listProjectState(input: {
  startDir?: string;
  scope?: string;
  status?: string;
} = {}): Promise<StateItem[]> {
  if (!hasTauri()) {
    return [
      {
        key: "desktop-alpha",
        title: "Finish desktop alpha",
        status: "in-progress",
        priority: "high",
        owner: "Grafiki",
        scope: "grafiki/desktop",
      },
      {
        key: "release-packaging",
        title: "Polish app packaging",
        status: "planned",
        priority: "medium",
        owner: null,
        scope: "grafiki/desktop",
      },
    ];
  }

  return invoke<StateItem[]>("list_project_state", {
    request: {
      startDir: input.startDir ?? "",
      scope: input.scope ?? "",
      status: input.status ?? "",
    },
  });
}

export async function listProjectDecisions(input: {
  startDir?: string;
  scope?: string;
  status?: string;
} = {}): Promise<DecisionItem[]> {
  if (!hasTauri()) {
    return [
      {
        id: "mock-decision-1",
        title: "Grafiki stays focused on AI memory",
        status: "active",
        scope: "grafiki/product",
        reasoning: "A generic notes app would dilute the memory-first workflow.",
      },
      {
        id: "mock-decision-2",
        title: "Desktop app uses multi-pane memory workflows",
        status: "active",
        scope: "grafiki/desktop",
        reasoning: "Panes let users compare retrieval, detail, graph, and state without losing context.",
      },
    ];
  }

  return invoke<DecisionItem[]>("list_project_decisions", {
    request: {
      startDir: input.startDir ?? "",
      scope: input.scope ?? "",
      status: input.status ?? "",
    },
  });
}

export async function listProjectRelations(input: {
  startDir?: string;
  scope?: string;
  relation?: string;
} = {}): Promise<GraphRelation[]> {
  if (!hasTauri()) {
    return [
      {
        id: "mock-relation-1",
        from_entity: "grafiki",
        to_entity: "desktop",
        relation: "uses",
        weight: 1.25,
        confidence: 0.92,
        source_type: "INFERRED",
        source: "preview",
      },
      {
        id: "mock-relation-2",
        from_entity: "desktop",
        to_entity: "retrieval",
        relation: "depends_on",
        weight: 1,
        confidence: 0.88,
        source_type: "EXTRACTED",
        source: null,
      },
    ].filter((relation) => !input.relation || relation.relation === input.relation);
  }

  return invoke<GraphRelation[]>("list_project_relations", {
    request: {
      startDir: input.startDir ?? "",
      scope: input.scope ?? "",
      relation: input.relation ?? "",
    },
  });
}

export async function listProjectSessions(input: {
  startDir?: string;
  scope?: string;
} = {}): Promise<SessionLogItem[]> {
  if (!hasTauri()) {
    return [
      {
        id: "mock-session-1",
        session_type: "codex",
        status: "active",
        scope: "grafiki/desktop",
        goal: "Finish desktop memory workflows",
        summary: null,
        accomplishments: [],
        remaining: ["Review handoff quality"],
        files_changed: [],
        decisions_made: [],
        entities_touched: [],
        handoff_context: null,
        parent_session: null,
        child_session: null,
        started_at: new Date().toISOString(),
        ended_at: null,
      },
    ];
  }

  return invoke<SessionLogItem[]>("list_project_sessions", {
    request: {
      startDir: input.startDir ?? "",
      scope: input.scope ?? "",
    },
  });
}

export async function listCandidates(input: {
  startDir?: string;
  scope?: string;
  status?: string;
  limit?: number;
} = {}): Promise<ExtractionCandidate[]> {
  if (!hasTauri()) {
    const status = input.status ?? "pending";
    return mockCandidates
      .filter((candidate) => status === "all" || candidate.status === status)
      .filter((candidate) => !input.scope || candidate.scope.includes(input.scope))
      .slice(0, input.limit ?? 50);
  }

  return invoke<ExtractionCandidate[]>("list_memory_candidates", {
    request: {
      startDir: input.startDir ?? "",
      scope: input.scope ?? "",
      status: input.status ?? "pending",
      limit: input.limit ?? 50,
    },
  });
}

export async function approveCandidate(input: {
  startDir?: string;
  id: string;
}): Promise<CandidateMutationResult> {
  if (!hasTauri()) {
    const candidate = mockCandidates.find((item) => item.id === input.id) ?? mockCandidates[0];
    return {
      candidate: {
        ...candidate,
        status: "approved",
        trusted_record_type: candidate.record_type,
        trusted_record_id: String(candidate.payload.key ?? candidate.payload.id ?? candidate.id),
        reviewed_at: new Date().toISOString(),
      },
      message: "Candidate approved into trusted memory.",
    };
  }

  return invoke<CandidateMutationResult>("approve_memory_candidate", {
    request: {
      startDir: input.startDir ?? "",
      id: input.id,
    },
  });
}

export async function editCandidate(input: {
  startDir?: string;
  id: string;
  recordType?: ExtractionCandidate["record_type"];
  payload?: Record<string, unknown>;
  scope?: string;
  confidence?: number;
  rationale?: string;
}): Promise<CandidateMutationResult> {
  if (!hasTauri()) {
    const candidate = mockCandidates.find((item) => item.id === input.id) ?? mockCandidates[0];
    return {
      candidate: {
        ...candidate,
        record_type: input.recordType ?? candidate.record_type,
        payload: input.payload ?? candidate.payload,
        scope: input.scope ?? candidate.scope,
        confidence: input.confidence ?? candidate.confidence,
        rationale: input.rationale ?? candidate.rationale,
      },
      message: "Candidate updated for review.",
    };
  }

  return invoke<CandidateMutationResult>("edit_memory_candidate", {
    request: {
      startDir: input.startDir ?? "",
      id: input.id,
      recordType: input.recordType,
      payload: input.payload,
      scope: input.scope,
      confidence: input.confidence,
      rationale: input.rationale,
    },
  });
}

export async function bulkReviewCandidates(input: {
  startDir?: string;
  action: "approve" | "reject";
  ids: string[];
  rationale?: string;
}): Promise<BulkCandidateReviewResult> {
  if (!hasTauri()) {
    const results = input.ids.map((id) => {
      const candidate = mockCandidates.find((item) => item.id === id) ?? mockCandidates[0];
      return {
        candidate: {
          ...candidate,
          status: input.action === "approve" ? "approved" as const : "rejected" as const,
          rationale: input.action === "reject" ? input.rationale ?? candidate.rationale : candidate.rationale,
          reviewed_at: new Date().toISOString(),
        },
        message:
          input.action === "approve"
            ? "Candidate approved into trusted memory."
            : "Candidate rejected.",
      };
    });
    return {
      action: input.action,
      requested: input.ids.length,
      succeeded: results.length,
      failed: 0,
      results,
      errors: [],
    };
  }

  return invoke<BulkCandidateReviewResult>("bulk_review_memory_candidates", {
    request: {
      startDir: input.startDir ?? "",
      action: input.action,
      ids: input.ids,
      rationale: input.rationale ?? "",
    },
  });
}

export async function rejectCandidate(input: {
  startDir?: string;
  id: string;
  rationale?: string;
}): Promise<CandidateMutationResult> {
  if (!hasTauri()) {
    const candidate = mockCandidates.find((item) => item.id === input.id) ?? mockCandidates[0];
    return {
      candidate: {
        ...candidate,
        status: "rejected",
        rationale: input.rationale || candidate.rationale,
        reviewed_at: new Date().toISOString(),
      },
      message: "Candidate rejected.",
    };
  }

  return invoke<CandidateMutationResult>("reject_memory_candidate", {
    request: {
      startDir: input.startDir ?? "",
      id: input.id,
      rationale: input.rationale ?? "",
    },
  });
}

export async function autoCaptureMemory(input: AutoCaptureInput = {}): Promise<AutoCaptureResult> {
  if (!hasTauri()) {
    const candidate: ExtractionCandidate = {
      id: `auto-capture-${Date.now()}`,
      source_type: "desktop:auto-capture",
      source: input.source ?? "preview",
      record_type: "context",
      payload: {
        key: "auto-capture-preview",
        title: "Auto-captured coding session snapshot",
        content: "Preview auto-capture candidate.",
      },
      scope: input.scope ?? "",
      confidence: 0.72,
      status: "pending",
      rationale: "Preview mode.",
      trusted_record_type: null,
      trusted_record_id: null,
      created_at: new Date().toISOString(),
      reviewed_at: null,
    };
    return {
      scope: input.scope ?? "",
      source: input.source ?? "preview",
      path: input.startDir ?? "",
      git_root: null,
      changed_files: ["preview.ts"],
      staged_files: [],
      unstaged_files: ["preview.ts"],
      untracked_files: [],
      diff_stat: "Preview diff stat.",
      last_commit: null,
      candidates: [{ candidate, message: "Candidate proposed for review." }],
      message: "Preview auto-capture created one pending candidate.",
    };
  }

  return invoke<AutoCaptureResult>("auto_capture_memory", {
    request: {
      startDir: input.startDir ?? "",
      scope: input.scope ?? "",
      source: input.source ?? "desktop",
      limit: input.limit ?? 25,
    },
  });
}

export async function startAutomaticCapture(input: {
  startDir?: string;
  scope?: string;
  sourceApp?: string;
} = {}): Promise<RawCaptureSessionResult> {
  if (!hasTauri()) {
    return {
      capture: {
        id: `capture-${Date.now()}`,
        project: "Grafiki",
        scope: input.scope ?? "",
        status: "active",
        source_app: input.sourceApp ?? "preview",
        consent_profile: "local-explicit",
        redaction_profile: "default",
        started_at: new Date().toISOString(),
        ended_at: null,
      },
      message: "Preview capture session started.",
    };
  }
  return invoke<RawCaptureSessionResult>("start_automatic_capture", {
    request: {
      startDir: input.startDir ?? "",
      scope: input.scope ?? "",
      sourceApp: input.sourceApp ?? "grafiki-desktop",
    },
  });
}

export async function stopAutomaticCapture(input: {
  startDir?: string;
  captureId: string;
}): Promise<RawCaptureSessionResult> {
  if (!hasTauri()) {
    return {
      capture: {
        id: input.captureId,
        project: "Grafiki",
        scope: "",
        status: "stopped",
        source_app: "preview",
        consent_profile: "local-explicit",
        redaction_profile: "default",
        started_at: new Date().toISOString(),
        ended_at: new Date().toISOString(),
      },
      message: "Preview capture session stopped.",
    };
  }
  return invoke<RawCaptureSessionResult>("stop_automatic_capture", {
    request: {
      startDir: input.startDir ?? "",
      captureId: input.captureId,
    },
  });
}

export async function getAutomaticCaptureStatus(input: {
  startDir?: string;
  scope?: string;
} = {}): Promise<RawCaptureStatus> {
  if (!hasTauri()) {
    return {
      project: "Grafiki",
      scope: input.scope ?? "",
      active_sessions: [],
      recent_events: [],
      event_count: 0,
    };
  }
  return invoke<RawCaptureStatus>("get_automatic_capture_status", {
    request: {
      startDir: input.startDir ?? "",
      scope: input.scope ?? "",
      limit: 20,
    },
  });
}

export async function listRawCaptureEvents(input: {
  startDir?: string;
  scope?: string;
  captureId?: string;
  sourceType?: string;
  limit?: number;
} = {}): Promise<RawCaptureEvent[]> {
  if (!hasTauri()) return [];
  return invoke<RawCaptureEvent[]>("list_raw_capture_events", {
    request: {
      startDir: input.startDir ?? "",
      scope: input.scope ?? "",
      captureId: input.captureId ?? "",
      sourceType: input.sourceType ?? "",
      limit: input.limit ?? 50,
    },
  });
}

export async function getCaptureConfig(input: { startDir?: string } = {}): Promise<CaptureConfigReport> {
  if (!hasTauri()) {
    return {
      project: "Grafiki",
      project_dir: input.startDir ?? "/preview/grafiki",
      config_path: `${input.startDir ?? "/preview/grafiki"}/.grafiki.capture.json`,
      created: false,
      config: {
        version: 1,
        sources: {
          git: true,
          transcripts: true,
          terminal: true,
          files: true,
          ide: true,
          screen: false,
          browser: false,
          audio: false,
          system: true,
        },
        blocked_paths: [".git", ".grafiki", ".grafiki.capture.json", ".env", "node_modules", "target"],
        blocked_apps: [],
        redaction_profile: "default",
        terminal_output: "off",
        screen_policy: "manual",
        browser_policy: "off",
      },
    };
  }

  return invoke<CaptureConfigReport>("get_capture_config", {
    request: { startDir: input.startDir ?? "" },
  });
}

export async function updateCaptureConfig(input: CaptureConfigUpdateInput): Promise<CaptureConfigReport> {
  if (!hasTauri()) {
    const current = await getCaptureConfig({ startDir: input.startDir });
    return {
      ...current,
      config: {
        ...current.config,
        sources: {
          ...current.config.sources,
          ...(input.git !== undefined ? { git: input.git } : {}),
          ...(input.transcripts !== undefined ? { transcripts: input.transcripts } : {}),
          ...(input.terminal !== undefined ? { terminal: input.terminal } : {}),
          ...(input.files !== undefined ? { files: input.files } : {}),
          ...(input.ide !== undefined ? { ide: input.ide } : {}),
          ...(input.screen !== undefined ? { screen: input.screen } : {}),
          ...(input.browser !== undefined ? { browser: input.browser } : {}),
          ...(input.audio !== undefined ? { audio: input.audio } : {}),
          ...(input.system !== undefined ? { system: input.system } : {}),
        },
        blocked_paths: [
          ...current.config.blocked_paths.filter((path) => !(input.removeBlockedPaths ?? []).includes(path)),
          ...(input.addBlockedPaths ?? []),
        ],
        blocked_apps: [
          ...current.config.blocked_apps.filter((app) => !(input.removeBlockedApps ?? []).includes(app)),
          ...(input.addBlockedApps ?? []),
        ],
        terminal_output: input.terminalOutput ?? current.config.terminal_output,
        screen_policy: input.screenPolicy ?? current.config.screen_policy,
        browser_policy: input.browserPolicy ?? current.config.browser_policy,
        redaction_profile: input.redactionProfile ?? current.config.redaction_profile,
      },
    };
  }

  return invoke<CaptureConfigReport>("update_capture_config_settings", { request: input });
}

export async function ingestRawCaptureEvent(input: {
  startDir?: string;
  captureId?: string;
  scope?: string;
  sourceType: string;
  source?: string;
  title?: string;
  text?: string;
  payload?: Record<string, unknown>;
  metadata?: Record<string, unknown>;
  privacyLevel?: string;
  redacted?: boolean;
}): Promise<RawCaptureEventResult> {
  if (!hasTauri()) {
    return {
      event: {
        id: `event-${Date.now()}`,
        capture_session: input.captureId ?? "preview",
        source_type: input.sourceType,
        source: input.source ?? null,
        title: input.title ?? null,
        text: input.text ?? null,
        payload: input.payload ?? null,
        metadata: input.metadata ?? null,
        privacy_level: input.privacyLevel ?? "internal",
        redacted: input.redacted ?? false,
        scope: input.scope ?? "",
        captured_at: new Date().toISOString(),
        created_at: new Date().toISOString(),
      },
      message: "Preview capture event ingested.",
    };
  }
  return invoke<RawCaptureEventResult>("ingest_raw_capture_event", {
    request: {
      startDir: input.startDir ?? "",
      captureId: input.captureId ?? "",
      scope: input.scope ?? "",
      sourceType: input.sourceType,
      source: input.source ?? "",
      title: input.title ?? "",
      text: input.text ?? "",
      payload: input.payload ?? null,
      metadata: input.metadata ?? null,
      privacyLevel: input.privacyLevel ?? "internal",
      redacted: input.redacted ?? false,
    },
  });
}

export async function captureScreenSnapshot(input: {
  startDir?: string;
  captureId?: string;
  scope?: string;
} = {}): Promise<RawCaptureEventResult> {
  if (!hasTauri()) {
    return ingestRawCaptureEvent({
      ...input,
      sourceType: "screen",
      title: "Preview screen snapshot",
      text: "Preview screen snapshot.",
      privacyLevel: "sensitive",
    });
  }
  return invoke<RawCaptureEventResult>("capture_screen_snapshot", {
    request: {
      startDir: input.startDir ?? "",
      captureId: input.captureId ?? "",
      scope: input.scope ?? "",
      source: "grafiki-desktop",
    },
  });
}

export async function summarizeAutomaticCapture(input: {
  startDir?: string;
  scope?: string;
  captureId?: string;
  limit?: number;
} = {}): Promise<RawCaptureCandidateResult> {
  if (!hasTauri()) {
    return {
      capture_id: input.captureId ?? null,
      events_summarized: 0,
      candidates: [],
      message: "Preview capture summary.",
    };
  }
  return invoke<RawCaptureCandidateResult>("summarize_automatic_capture", {
    request: {
      startDir: input.startDir ?? "",
      scope: input.scope ?? "",
      captureId: input.captureId ?? "",
      limit: input.limit ?? 80,
    },
  });
}

export async function importAgentTranscripts(
  input: AgentTranscriptImportInput,
): Promise<AgentTranscriptImportResult> {
  if (!hasTauri()) {
    return {
      agent: input.agent,
      scope: input.scope ?? "",
      capture_id: `capture-${Date.now()}`,
      files_scanned: input.input ? 1 : 0,
      files_imported: input.input ? 1 : 0,
      events_imported: input.input ? 2 : 0,
      sources: input.input
        ? [{ path: input.input, events: 2, skipped: null }]
        : [],
      candidates: input.summarize
        ? {
            capture_id: null,
            events_summarized: input.input ? 2 : 0,
            candidates: [],
            message: "Preview transcript import summary.",
          }
        : null,
      message: input.input
        ? `Preview imported ${input.agent} transcript events.`
        : `Preview did not find ${input.agent} transcript events.`,
    };
  }

  return invoke<AgentTranscriptImportResult>("import_agent_transcripts_from_disk", {
    request: {
      startDir: input.startDir ?? "",
      scope: input.scope ?? "",
      agent: input.agent,
      input: input.input ?? "",
      limit: input.limit ?? 200,
      summarize: input.summarize ?? false,
    },
  });
}

export async function deleteMemoryRecord(input: {
  startDir?: string;
  recordType: string;
  id: string;
}): Promise<DeleteMemoryResult> {
  if (!hasTauri()) {
    return {
      record_type: input.recordType,
      id: input.id,
      title: input.id,
      scope: "",
      message: `Deleted ${input.recordType} in preview mode.`,
    };
  }

  return invoke<DeleteMemoryResult>("delete_memory_record", {
    request: {
      startDir: input.startDir ?? "",
      recordType: input.recordType,
      id: input.id,
    },
  });
}

export async function updateMemoryRecord(input: UpdateMemoryInput): Promise<UpdateMemoryResult> {
  if (!hasTauri()) {
    return {
      record_type: input.recordType,
      id: input.id,
      title: input.title ?? input.id,
      scope: input.scope ?? "",
      message: `Updated ${input.recordType} in preview mode.`,
    };
  }

  return invoke<UpdateMemoryResult>("update_memory_record", {
    request: input,
  });
}

export async function exportMemoryToFile(input: {
  startDir?: string;
  scope?: string;
} = {}): Promise<ExportFileResult | null> {
  if (!hasTauri()) {
    return {
      output_path: "preview-grafiki-export.json",
      records: mockSearchResults.length,
      message: "Exported preview memory.",
    };
  }

  const outputPath = await save({
    title: "Export Grafiki memory",
    defaultPath: "grafiki-export.json",
    filters: [{ name: "Grafiki JSON", extensions: ["json"] }],
  });
  if (!outputPath) return null;

  return invoke<ExportFileResult>("export_memory_file", {
    request: {
      startDir: input.startDir ?? "",
      scope: input.scope ?? "",
      outputPath,
    },
  });
}

export async function importMemoryFromFile(input: {
  startDir?: string;
} = {}): Promise<ImportMemoryResult | null> {
  if (!hasTauri()) {
    return {
      project: "Grafiki",
      source_project: "Preview",
      entities: 1,
      relations: 1,
      skipped_relations: 0,
      observations: 1,
      decisions: 1,
      state: 1,
      context_skipped: 0,
      sessions_skipped: 0,
    };
  }

  const inputPath = await open({
    title: "Import Grafiki memory",
    multiple: false,
    filters: [{ name: "Grafiki JSON", extensions: ["json"] }],
  });
  if (typeof inputPath !== "string") return null;

  return invoke<ImportMemoryResult>("import_memory_file", {
    request: {
      startDir: input.startDir ?? "",
      inputPath,
    },
  });
}

export async function processProjectEmbeddings(input: {
  startDir?: string;
  scope?: string;
  rebuild?: boolean;
  limit?: number;
} = {}): Promise<ProcessEmbeddingsResult> {
  if (!hasTauri()) {
    return {
      project: "Grafiki",
      scope: input.scope ?? "*",
      provider: "deterministic",
      model: "preview",
      dimension: 64,
      enqueued: input.rebuild ? 3 : 0,
      processed: 3,
      skipped: 0,
      failed: 0,
      pending_remaining: 0,
    };
  }

  return invoke<ProcessEmbeddingsResult>("process_project_embeddings", {
    request: {
      startDir: input.startDir ?? "",
      scope: input.scope ?? "*",
      rebuild: input.rebuild ?? false,
      limit: input.limit ?? 100,
    },
  });
}

export async function getDaemonStatus(input: {
  startDir?: string;
} = {}): Promise<DaemonStatus> {
  if (!hasTauri()) {
    return {
      project: "Grafiki",
      running: false,
      pid: null,
      host: "127.0.0.1",
      port: 9700,
      url: "http://127.0.0.1:9700",
      pid_path: "~/.grafiki/daemons/Grafiki.pid.json",
      log_path: "~/.grafiki/daemons/Grafiki.log",
      cli_path: "preview",
      cli_available: true,
      message: "Preview daemon is stopped.",
    };
  }

  return invoke<DaemonStatus>("get_daemon_status", {
    request: {
      startDir: input.startDir ?? "",
    },
  });
}

export async function startDaemon(input: {
  startDir?: string;
  host?: string;
  port?: number;
  token?: string;
} = {}): Promise<DaemonStartResult> {
  if (!hasTauri()) {
    return {
      project: "Grafiki",
      running: true,
      already_running: false,
      pid: 9700,
      host: input.host ?? "127.0.0.1",
      port: input.port ?? 9700,
      url: `http://${input.host ?? "127.0.0.1"}:${input.port ?? 9700}`,
      pid_path: "~/.grafiki/daemons/Grafiki.pid.json",
      log_path: "~/.grafiki/daemons/Grafiki.log",
      cli_path: "preview",
      message: "Preview daemon started.",
    };
  }

  return invoke<DaemonStartResult>("start_daemon", {
    request: {
      startDir: input.startDir ?? "",
      host: input.host ?? "127.0.0.1",
      port: input.port ?? 9700,
      token: input.token ?? "",
    },
  });
}

export async function stopDaemon(input: {
  startDir?: string;
} = {}): Promise<DaemonStopResult> {
  if (!hasTauri()) {
    return {
      project: "Grafiki",
      stopped: true,
      pid: 9700,
      pid_path: "~/.grafiki/daemons/Grafiki.pid.json",
      cli_path: "preview",
      message: "Preview daemon stopped.",
    };
  }

  return invoke<DaemonStopResult>("stop_daemon", {
    request: {
      startDir: input.startDir ?? "",
    },
  });
}

export async function captureMemory(input: CaptureMemoryInput): Promise<CaptureMemoryResult> {
  if (!hasTauri()) {
    return {
      record_type: input.captureType,
      id: `mock-${Date.now()}`,
      title: input.title,
      scope: input.scope ?? "",
      message: `Captured ${input.captureType} locally in preview mode.`,
    };
  }

  return invoke<CaptureMemoryResult>("capture_memory", { request: input });
}

export async function initializeProject(input: {
  projectDir: string;
  projectName?: string;
}): Promise<InitProjectResult> {
  if (!hasTauri()) {
    return {
      project: input.projectName || "Grafiki",
      project_dir: input.projectDir,
      db_path: "~/.grafiki/Grafiki.db",
      marker_path: `${input.projectDir}/.grafiki`,
      imported_files: [],
      proposed_candidates: 0,
      trusted_records: 0,
      skipped_sources: [],
      decisions_found: 0,
      rules_found: 0,
      next_agent_setup: `grafiki mcp --project ${input.projectName || "Grafiki"} --path ${input.projectDir}`,
    };
  }

  return invoke<InitProjectResult>("initialize_project", { request: input });
}

export async function listAgentActivity(input: {
  startDir?: string;
  scope?: string;
  limit?: number;
} = {}): Promise<AgentQueryLogItem[]> {
  if (!hasTauri()) {
    return [
      {
        id: "mock-agent-query",
        agent: "codex",
        question: "What should I know before changing the desktop UI?",
        scope: input.scope ?? "",
        returned_ids: ["decision:mock-ui-direction", "context:mock-desktop-plan"],
        retrieval_mode: "hybrid",
        latency_ms: 12,
        created_at: new Date().toISOString(),
      },
    ];
  }

  return invoke<AgentQueryLogItem[]>("list_agent_activity", {
    request: {
      startDir: input.startDir ?? "",
      scope: input.scope ?? "",
      limit: input.limit ?? 50,
    },
  });
}

export async function startSession(input: StartSessionInput): Promise<StartSessionResult> {
  if (!hasTauri()) {
    return {
      session_id: `mock-session-${Date.now()}`,
      project: "Grafiki",
      session_type: input.sessionType,
      goal: input.goal,
      scope: input.scope ?? "",
      briefing: "Preview session briefing.",
    };
  }

  return invoke<StartSessionResult>("start_grafiki_session", { request: input });
}

export async function endSession(input: EndSessionInput): Promise<EndSessionResult> {
  if (!hasTauri()) {
    return {
      session_id: input.sessionId || "mock-session",
      project: "Grafiki",
      status: input.status,
      summary: input.summary,
    };
  }

  return invoke<EndSessionResult>("end_grafiki_session", { request: input });
}

export async function handoffSession(input: HandoffSessionInput): Promise<HandoffSessionResult> {
  if (!hasTauri()) {
    return {
      parent_session_id: input.sessionId || "mock-session",
      child_session_id: `mock-child-${Date.now()}`,
      project: "Grafiki",
      scope: "grafiki/desktop",
      handoff_context: "Preview handoff context.",
    };
  }

  return invoke<HandoffSessionResult>("handoff_grafiki_session", { request: input });
}

export async function pickProjectFolder(defaultPath?: string): Promise<string | null> {
  if (!hasTauri()) {
    return window.prompt("Project folder", defaultPath ?? mockSnapshot.start_dir);
  }

  const selected = await open({
    directory: true,
    multiple: false,
    defaultPath: defaultPath || undefined,
    title: "Choose Grafiki project folder",
  });

  return typeof selected === "string" ? selected : null;
}

const mockCandidates: ExtractionCandidate[] = [
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

export const mockSearchResults = [
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

const mockSnapshot: ProjectSnapshot = {
  start_dir: "/Users/vishalsunilkumar/Documents/Project/Grafiki",
  scope: "",
  memory_available: true,
  project: {
    project: "Grafiki",
    project_dir: "/Users/vishalsunilkumar/Documents/Project/Grafiki",
    db_path: "~/.grafiki/Grafiki.db",
    marker_path: "/Users/vishalsunilkumar/Documents/Project/Grafiki/.grafiki",
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
