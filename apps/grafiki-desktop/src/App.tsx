import {
  AnimatePresence,
  LayoutGroup,
  motion,
  useReducedMotion,
} from "framer-motion";
import {
  Activity,
  AlertTriangle,
  Archive,
  BrainCircuit,
  CheckCircle2,
  CircleDot,
  Columns3,
  Command as CommandIcon,
  Database,
  Download,
  FileClock,
  FileText,
  FolderOpen,
  GitBranch,
  History,
  LayoutDashboard,
  Network,
  PanelRight,
  Pencil,
  Plus,
  RefreshCcw,
  Search as SearchIcon,
  Settings,
  ShieldQuestion,
  Sparkles,
  SplitSquareHorizontal,
  Trash2,
  Upload,
  X,
  RadioTower,
} from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import {
  approveCandidate,
  autoCaptureMemory,
  bulkReviewCandidates,
  captureScreenSnapshot,
  captureMemory,
  deleteMemoryRecord,
  editCandidate,
  endSession,
  exportMemoryToFile,
  getAutomaticCaptureStatus,
  getCaptureConfig,
  getDaemonStatus,
  getMemoryGraph,
  getMemoryRecord,
  getProjectSnapshot,
  handoffSession,
  importAgentTranscripts,
  importMemoryFromFile,
  initializeProject,
  listAgentActivity,
  listProjectContext,
  listProjectDecisions,
  listProjectRelations,
  listProjectSessions,
  listProjectState,
  listCandidates,
  pickProjectFolder,
  processProjectEmbeddings,
  searchProjectMemory,
  startDaemon,
  startSession,
  stopDaemon,
  rejectCandidate,
  startAutomaticCapture,
  updateMemoryRecord,
  updateCaptureConfig,
  stopAutomaticCapture,
  summarizeAutomaticCapture,
} from "./api";
import {
  decodeLayoutFromHash,
  loadInitialLayout,
  newPaneId,
  persistLayout,
  titleForPane,
} from "./layout";
import type {
  AgentQueryLogItem,
  AgentTranscriptImportInput,
  CaptureConfigReport,
  CaptureSourceConfig,
  CaptureMemoryResult,
  CaptureType,
  ContextSummary,
  DaemonStatus,
  DecisionItem,
  EvidenceLink,
  ExtractionCandidate,
  GraphRelation,
  GraphReport,
  HandoffSessionResult,
  MemoryRecordDetail,
  PaneKind,
  PaneState,
  ProjectSnapshot,
  RawCaptureStatus,
  SearchMode,
  SearchResult,
  SessionLogItem,
  StateItem,
  LayoutState,
} from "./types";

const PROJECT_ROOT_KEY = "grafiki.desktop.projectRoot";

const navItems: Array<{ kind: PaneKind; label: string; hotkey: string; icon: typeof LayoutDashboard }> = [
  { kind: "overview", label: "Overview", hotkey: "O", icon: LayoutDashboard },
  { kind: "search", label: "Search", hotkey: "/", icon: SearchIcon },
  { kind: "graph", label: "Graph", hotkey: "G", icon: Network },
  { kind: "candidates", label: "Review", hotkey: "V", icon: ShieldQuestion },
  { kind: "agent", label: "Agent Activity", hotkey: "A", icon: RadioTower },
  { kind: "relations", label: "Relations", hotkey: "R", icon: GitBranch },
  { kind: "sessions", label: "Sessions", hotkey: "S", icon: History },
  { kind: "state", label: "State", hotkey: "T", icon: Activity },
  { kind: "decisions", label: "Decisions", hotkey: "D", icon: GitBranch },
  { kind: "context", label: "Context", hotkey: "C", icon: Archive },
  { kind: "settings", label: "Settings", hotkey: ",", icon: Settings },
];

const captureItems = [
  { captureType: "decision", label: "Decision", icon: GitBranch },
  { captureType: "observation", label: "Observation", icon: BrainCircuit },
  { captureType: "state", label: "State Item", icon: Activity },
  { captureType: "context", label: "Context", icon: FileText },
  { captureType: "handoff", label: "Handoff", icon: FileClock },
  { captureType: "relation", label: "Relation", icon: Network },
] as const;

const entityTypeOptions = ["concept", "module", "service", "file", "api", "tool", "library", "config", "person", "endpoint"];
const observationCategories = [
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
const contextCategories = ["reference", "spec", "architecture", "guide", "runbook", "onboarding", "audit", "postmortem"];
const stateStatuses = ["planned", "in-progress", "blocked", "needs-review", "done", "abandoned"];
const statePriorities = ["medium", "high", "critical", "low"];
const decisionStatuses = ["active", "revisit", "superseded", "revoked"];
const candidateStatuses = ["pending", "approved", "rejected", "all"];
const relationTypes = [
  "works_with",
  "depends_on",
  "blocks",
  "unblocks",
  "part_of",
  "uses",
  "produces",
  "consumes",
  "calls",
  "extends",
  "replaces",
  "tests",
  "deploys_to",
  "owns",
  "related_to",
];
const relationSourceTypes = ["EXTRACTED", "INFERRED", "AMBIGUOUS"];
const sessionTypes = [
  "codex",
  "claude-code",
  "claude-ai",
  "cursor",
  "copilot",
  "windsurf",
  "cline",
  "aider",
  "co-work",
  "other",
];
const sessionStatuses = ["active", "completed", "handed-off", "abandoned"];
const endStatuses = ["completed", "handed-off", "abandoned"];
const searchRecordTypes = ["all", "entity", "observation", "decision", "context", "state", "session"];

const captureCopy: Record<CaptureType, { title: string; titlePlaceholder: string; body: string; bodyPlaceholder: string }> = {
  decision: {
    title: "Decision",
    titlePlaceholder: "Decision title",
    body: "Reasoning",
    bodyPlaceholder: "Why this decision matters, tradeoffs, alternatives.",
  },
  observation: {
    title: "Entity",
    titlePlaceholder: "Thing this memory belongs to",
    body: "Observation",
    bodyPlaceholder: "A durable fact, behavior, constraint, or lesson.",
  },
  state: {
    title: "Work item",
    titlePlaceholder: "What needs to remain active",
    body: "Details",
    bodyPlaceholder: "Current state, blockers, or next useful action.",
  },
  context: {
    title: "Context title",
    titlePlaceholder: "Name for this trusted context",
    body: "Context",
    bodyPlaceholder: "Reference material that future AI sessions should trust.",
  },
  handoff: {
    title: "Handoff",
    titlePlaceholder: "Handoff label",
    body: "Note",
    bodyPlaceholder: "Optional note for your own orientation. Existing memory generates the handoff.",
  },
  relation: {
    title: "Source entity",
    titlePlaceholder: "Entity to link from",
    body: "Optional observation",
    bodyPlaceholder: "Why this relationship exists.",
  },
};

const transition = {
  quick: { duration: 0.14, ease: [0.2, 0, 0.2, 1] },
  pane: { type: "spring", stiffness: 420, damping: 38, mass: 0.8 },
  modal: { type: "spring", stiffness: 520, damping: 42, mass: 0.9 },
} as const;

function pressMotion(reduceMotion: boolean) {
  return reduceMotion
    ? {}
    : {
        whileHover: { y: -1 },
        whileTap: { scale: 0.985 },
        transition: transition.quick,
      };
}

export default function App() {
  const [layout, setLayout] = useState<LayoutState>(() => loadInitialLayout());
  const [projectRoot, setProjectRoot] = useState(() => localStorage.getItem(PROJECT_ROOT_KEY) ?? "");
  const [snapshot, setSnapshot] = useState<ProjectSnapshot | null>(null);
  const [selectedResult, setSelectedResult] = useState<SearchResult | null>(null);
  const [recordDetail, setRecordDetail] = useState<MemoryRecordDetail | null>(null);
  const [recordDetailError, setRecordDetailError] = useState<string | null>(null);
  const [recordDetailLoading, setRecordDetailLoading] = useState(false);
  const [commandOpen, setCommandOpen] = useState(false);
  const [launcherOpen, setLauncherOpen] = useState(false);
  const [inspectorOpen, setInspectorOpen] = useState(false);
  const reduceMotion = useReducedMotion() ?? false;

  useEffect(() => {
    refreshSnapshot();
  }, [projectRoot]);

  useEffect(() => {
    persistLayout(layout);
  }, [layout]);

  useEffect(() => {
    if (projectRoot.trim()) localStorage.setItem(PROJECT_ROOT_KEY, projectRoot);
    else localStorage.removeItem(PROJECT_ROOT_KEY);
    setSelectedResult(null);
    setRecordDetail(null);
  }, [projectRoot]);

  useEffect(() => {
    const onHashChange = () => {
      const next = decodeLayoutFromHash(window.location.hash);
      if (next) setLayout(next);
    };
    window.addEventListener("hashchange", onHashChange);
    return () => window.removeEventListener("hashchange", onHashChange);
  }, []);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      const key = event.key.toLowerCase();
      if ((event.metaKey || event.ctrlKey) && key === "k") {
        event.preventDefault();
        setCommandOpen(true);
      }
      if ((event.metaKey || event.ctrlKey) && key === "n") {
        event.preventDefault();
        setLauncherOpen(true);
      }
      if ((event.metaKey || event.ctrlKey) && event.shiftKey && key === "d") {
        event.preventDefault();
        openCapture("decision");
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);

  const activePane = useMemo(
    () => layout.panes.find((pane) => pane.id === layout.activePaneId) ?? layout.panes[0],
    [layout],
  );
  const detailTarget = useMemo(() => {
    if (selectedResult) {
      return {
        recordType: selectedResult.record_type,
        id: selectedResult.id,
        scope: selectedResult.scope,
        startDir: projectRoot,
      };
    }
    if (activePane?.kind === "detail" && activePane.recordId && activePane.recordType) {
      return {
        recordType: activePane.recordType,
        id: activePane.recordId,
        scope: "",
        startDir: projectRoot,
      };
    }
    return null;
  }, [selectedResult, activePane, projectRoot]);

  useEffect(() => {
    if (!detailTarget) {
      setRecordDetail(null);
      setRecordDetailError(null);
      setRecordDetailLoading(false);
      return;
    }

    let cancelled = false;
    setRecordDetailLoading(true);
    setRecordDetailError(null);
    getMemoryRecord(detailTarget)
      .then((detail) => {
        if (!cancelled) setRecordDetail(detail);
      })
      .catch((error) => {
        if (!cancelled) {
          setRecordDetail(null);
          setRecordDetailError(String(error));
        }
      })
      .finally(() => {
        if (!cancelled) setRecordDetailLoading(false);
      });

    return () => {
      cancelled = true;
    };
  }, [detailTarget?.recordType, detailTarget?.id, detailTarget?.scope, detailTarget?.startDir]);

  function activatePane(id: string) {
    setLayout((current) => ({ ...current, activePaneId: id }));
  }

  function openPane(kind: PaneKind, patch: Partial<PaneState> = {}, preferExisting = false) {
    if (preferExisting) {
      const existing = layout.panes.find((pane) => pane.kind === kind);
      if (existing) {
        activatePane(existing.id);
        return;
      }
    }

    const pane: PaneState = {
      id: newPaneId(kind),
      kind,
      title: titleForPane({ kind, ...patch }),
      ...patch,
    };
    setLayout((current) => ({
      activePaneId: pane.id,
      panes: [...current.panes, pane],
    }));
  }

  function switchPrimaryPane(kind: PaneKind, patch: Partial<PaneState> = {}) {
    const pane: PaneState = {
      id: newPaneId(kind),
      kind,
      title: titleForPane({ kind, ...patch }),
      ...patch,
    };
    setLayout({
      activePaneId: pane.id,
      panes: [pane],
    });
  }

  function openCapture(captureType: CaptureType) {
    openPane("capture", {
      captureType,
      title: `New ${captureType}`,
    });
    setLauncherOpen(false);
  }

  async function refreshSnapshot() {
    const next = await getProjectSnapshot({ startDir: projectRoot });
    setSnapshot(next);
    return next;
  }

  function handleCaptured(result: CaptureMemoryResult) {
    setSelectedResult({
      record_type: result.record_type,
      id: result.id,
      title: result.title,
      snippet: result.message,
      scope: result.scope,
      score: null,
    });
    setInspectorOpen(true);
    refreshSnapshot();
  }

  async function initializeCurrentProject(path?: string) {
    const projectDir = path?.trim() || projectRoot.trim() || snapshot?.start_dir || "";
    if (!projectDir) return;
    await initializeProject({ projectDir });
    setProjectRoot(projectDir);
    const next = await getProjectSnapshot({ startDir: projectDir });
    setSnapshot(next);
  }

  function handleSessionChanged(result: { record_type: string; id: string; title: string; scope: string; message: string }) {
    setSelectedResult({
      record_type: result.record_type,
      id: result.id,
      title: result.title,
      snippet: result.message,
      scope: result.scope,
      score: null,
    });
    setInspectorOpen(true);
    refreshSnapshot();
  }

  function updatePane(id: string, patch: Partial<PaneState>) {
    setLayout((current) => ({
      ...current,
      panes: current.panes.map((pane) => (pane.id === id ? { ...pane, ...patch } : pane)),
    }));
  }

  function closePane(id: string) {
    setLayout((current) => {
      if (current.panes.length === 1) return current;
      const index = current.panes.findIndex((pane) => pane.id === id);
      const panes = current.panes.filter((pane) => pane.id !== id);
      const activePaneId =
        current.activePaneId === id ? panes[Math.max(0, index - 1)].id : current.activePaneId;
      return { activePaneId, panes };
    });
  }

  function duplicateActivePane() {
    if (!activePane) return;
    const pane = {
      ...activePane,
      id: newPaneId(activePane.kind),
      title: `${activePane.title} Copy`,
    };
    setLayout((current) => ({
      activePaneId: pane.id,
      panes: [...current.panes, pane],
    }));
  }

  function openResultInPane(result: SearchResult) {
    setSelectedResult(result);
    setInspectorOpen(true);
    openPane("detail", {
      recordId: result.id,
      recordType: result.record_type,
      title: `${result.record_type}: ${result.title}`,
    });
  }

  function openSelectedDetail() {
    const detail = recordDetail;
    const result = selectedResult;
    if (detail) {
      openPane("detail", {
        recordId: detail.id,
        recordType: detail.record_type,
        title: `${detail.record_type}: ${detail.title}`,
      });
      return;
    }
    if (result) openResultInPane(result);
  }

  function openGraphForEntity(entityId: string) {
    openPane("graph", {
      entityId,
      title: `Graph: ${entityId}`,
    });
  }

  return (
    <LayoutGroup>
      <motion.div
        className={`app-shell ${inspectorOpen ? "inspector-open" : ""}`}
        initial={reduceMotion ? false : { opacity: 0 }}
        animate={{ opacity: 1 }}
        transition={transition.quick}
      >
      <Rail
        activeKind={activePane?.kind ?? "overview"}
        onOpen={(kind) => switchPrimaryPane(kind)}
        onLauncher={() => setLauncherOpen(true)}
        onCommand={() => setCommandOpen(true)}
        reduceMotion={reduceMotion}
      />

      <main className="workspace">
        <TopStatus
          snapshot={snapshot}
          paneCount={layout.panes.length}
          inspectorOpen={inspectorOpen}
          onToggleInspector={() => setInspectorOpen((current) => !current)}
          onCapture={() => setLauncherOpen(true)}
        />

        <section className="pane-strip" aria-label="Workspace panes">
          <AnimatePresence initial={false}>
            {layout.panes.map((pane) => (
              <MemoryPane
                key={pane.id}
                pane={pane}
                active={pane.id === layout.activePaneId}
                snapshot={snapshot}
                projectRoot={projectRoot}
                selectedResult={selectedResult}
                recordDetail={recordDetail}
                recordDetailLoading={recordDetailLoading}
                recordDetailError={recordDetailError}
                reduceMotion={reduceMotion}
                onActivate={() => activatePane(pane.id)}
                onClose={() => closePane(pane.id)}
                onSplit={duplicateActivePane}
                onUpdate={(patch) => updatePane(pane.id, patch)}
                onSelectResult={(result) => {
                  setSelectedResult(result);
                  setInspectorOpen(true);
                }}
                onOpenResult={openResultInPane}
                onCaptured={handleCaptured}
                onSessionChanged={handleSessionChanged}
                onProjectRootChange={setProjectRoot}
                onInitializeProject={initializeCurrentProject}
                onMemoryChanged={refreshSnapshot}
              />
            ))}
          </AnimatePresence>
        </section>
      </main>

      <AnimatePresence initial={false}>
        {inspectorOpen ? (
          <Inspector
            snapshot={snapshot}
            activePane={activePane}
            selectedResult={selectedResult}
            recordDetail={recordDetail}
            recordDetailLoading={recordDetailLoading}
            recordDetailError={recordDetailError}
            onOpenDetail={openSelectedDetail}
            onOpenGraph={openGraphForEntity}
            onClose={() => setInspectorOpen(false)}
            reduceMotion={reduceMotion}
          />
        ) : null}
      </AnimatePresence>

      <AnimatePresence>
        {commandOpen ? (
          <CommandPalette
            snapshot={snapshot}
            reduceMotion={reduceMotion}
            onClose={() => setCommandOpen(false)}
            onOpenPane={(kind) => switchPrimaryPane(kind)}
            onNewSearch={(query) =>
              switchPrimaryPane("search", {
                query,
                mode: "hybrid",
                title: titleForPane({ kind: "search", query }),
              })
            }
            onCapture={openCapture}
            onSplit={duplicateActivePane}
          />
        ) : null}
      </AnimatePresence>

      <AnimatePresence>
        {launcherOpen ? (
          <Launcher
            reduceMotion={reduceMotion}
            onClose={() => setLauncherOpen(false)}
            onCapture={openCapture}
          />
        ) : null}
      </AnimatePresence>
      </motion.div>
    </LayoutGroup>
  );
}

function Rail(props: {
  activeKind: PaneKind;
  onOpen: (kind: PaneKind) => void;
  onLauncher: () => void;
  onCommand: () => void;
  reduceMotion: boolean;
}) {
  return (
    <aside className="rail">
      <motion.button
        className="brand"
        aria-label="Grafiki overview"
        onClick={() => props.onOpen("overview")}
        {...pressMotion(props.reduceMotion)}
      >
        <span className="brand-mark">G</span>
        <span className="brand-text">Grafiki</span>
      </motion.button>

      <nav className="rail-nav" aria-label="Primary">
        {navItems.map((item) => {
          const Icon = item.icon;
          return (
            <motion.button
              key={item.kind}
              layout
              className={`rail-item ${props.activeKind === item.kind ? "active" : ""}`}
              onClick={() => props.onOpen(item.kind)}
              title={`${item.label} (${item.hotkey})`}
              {...pressMotion(props.reduceMotion)}
            >
              <Icon size={18} />
              <span>{item.label}</span>
              <kbd>{item.hotkey}</kbd>
            </motion.button>
          );
        })}
      </nav>

      <div className="rail-actions">
        <motion.button
          className="rail-action"
          onClick={props.onCommand}
          {...pressMotion(props.reduceMotion)}
        >
          <CommandIcon size={17} />
          <span>Command</span>
          <kbd>⌘K</kbd>
        </motion.button>
        <motion.button
          className="rail-action primary"
          onClick={props.onLauncher}
          {...pressMotion(props.reduceMotion)}
        >
          <Plus size={18} />
          <span>Capture</span>
          <kbd>⌘N</kbd>
        </motion.button>
      </div>
    </aside>
  );
}

function TopStatus(props: {
  snapshot: ProjectSnapshot | null;
  paneCount: number;
  inspectorOpen: boolean;
  onToggleInspector: () => void;
  onCapture: () => void;
}) {
  const snapshot = props.snapshot;
  const project = snapshot?.project?.project ?? "No project";
  const embedding = snapshot?.embedding?.runtime;
  const memoryAvailable = snapshot?.memory_available ?? false;

  return (
    <motion.header
      className="top-status"
      initial={{ opacity: 0, y: -6 }}
      animate={{ opacity: 1, y: 0 }}
      transition={transition.quick}
    >
      <div className="project-lockup">
        <Database size={16} />
        <div>
          <strong>{project}</strong>
          <span>{snapshot?.project?.db_path ?? snapshot?.start_dir ?? "Waiting for memory"}</span>
        </div>
      </div>

      <div className="status-cluster">
        <button className="button primary top-action" type="button" onClick={props.onCapture}>
          <Plus size={15} />
          Capture
        </button>
        <StatusPill tone={memoryAvailable ? "good" : "warn"} icon={memoryAvailable ? CheckCircle2 : AlertTriangle}>
          {memoryAvailable ? "Memory online" : "Initialize needed"}
        </StatusPill>
        <StatusPill tone="neutral" icon={Columns3}>
          {props.paneCount} {props.paneCount === 1 ? "pane" : "panes"}
        </StatusPill>
        <StatusPill tone="accent" icon={Sparkles}>
          {embedding ? `${embedding.fresh_records}/${embedding.embeddable_records} fresh` : "Embeddings"}
        </StatusPill>
        <button
          className={`icon-button inspector-toggle ${props.inspectorOpen ? "active" : ""}`}
          type="button"
          title={props.inspectorOpen ? "Hide inspector" : "Show inspector"}
          onClick={props.onToggleInspector}
        >
          <PanelRight size={16} />
        </button>
      </div>
    </motion.header>
  );
}

function StatusPill(props: {
  tone: "good" | "warn" | "neutral" | "accent";
  icon: typeof CheckCircle2;
  children: React.ReactNode;
}) {
  const Icon = props.icon;
  return (
    <motion.span className={`status-pill ${props.tone}`} layout transition={transition.quick}>
      <Icon size={14} />
      {props.children}
    </motion.span>
  );
}

function MemoryPane(props: {
  pane: PaneState;
  active: boolean;
  snapshot: ProjectSnapshot | null;
  projectRoot: string;
  selectedResult: SearchResult | null;
  recordDetail: MemoryRecordDetail | null;
  recordDetailLoading: boolean;
  recordDetailError: string | null;
  reduceMotion: boolean;
  onActivate: () => void;
  onClose: () => void;
  onSplit: () => void;
  onUpdate: (patch: Partial<PaneState>) => void;
  onSelectResult: (result: SearchResult) => void;
  onOpenResult: (result: SearchResult) => void;
  onCaptured: (result: CaptureMemoryResult) => void;
  onSessionChanged: (result: { record_type: string; id: string; title: string; scope: string; message: string }) => void;
  onProjectRootChange: (path: string) => void;
  onInitializeProject: (path?: string) => Promise<void>;
  onMemoryChanged: () => Promise<ProjectSnapshot>;
}) {
  const pane = props.pane;

  return (
    <motion.article
      layout
      className={`memory-pane ${props.active ? "active" : ""}`}
      onPointerDown={props.onActivate}
      initial={props.reduceMotion ? false : { opacity: 0, y: 10, scale: 0.992 }}
      animate={{ opacity: 1, y: 0, scale: 1 }}
      exit={props.reduceMotion ? undefined : { opacity: 0, y: 8, scale: 0.992 }}
      transition={transition.pane}
    >
      <header className="pane-header">
        <div>
          <span className="pane-kind">{pane.kind}</span>
          <h2>{pane.title}</h2>
        </div>
        <div className="pane-actions">
          <button onClick={props.onSplit} title="Duplicate pane">
            <SplitSquareHorizontal size={15} />
          </button>
          <button onClick={props.onClose} title="Close pane">
            <X size={15} />
          </button>
        </div>
      </header>

      <div className="pane-body">
        {pane.kind === "overview" ? <OverviewPane snapshot={props.snapshot} /> : null}
        {pane.kind === "search" ? (
          <SearchPane
            pane={pane}
            snapshot={props.snapshot}
            projectRoot={props.projectRoot}
            reduceMotion={props.reduceMotion}
            onUpdate={props.onUpdate}
            onSelectResult={props.onSelectResult}
            onOpenResult={props.onOpenResult}
            onMemoryChanged={props.onMemoryChanged}
          />
        ) : null}
        {pane.kind === "graph" ? (
          <GraphPane pane={pane} snapshot={props.snapshot} startDir={props.projectRoot} onUpdate={props.onUpdate} />
        ) : null}
        {pane.kind === "candidates" ? (
          <CandidatesPane
            snapshot={props.snapshot}
            startDir={props.projectRoot}
            reduceMotion={props.reduceMotion}
            active={props.active}
            onSelectResult={props.onSelectResult}
            onOpenResult={props.onOpenResult}
            onMemoryChanged={props.onMemoryChanged}
          />
        ) : null}
        {pane.kind === "agent" ? (
          <AgentActivityPane
            snapshot={props.snapshot}
            startDir={props.projectRoot}
            reduceMotion={props.reduceMotion}
          />
        ) : null}
        {pane.kind === "relations" ? (
          <RelationsPane
            snapshot={props.snapshot}
            startDir={props.projectRoot}
            reduceMotion={props.reduceMotion}
            onSelectResult={props.onSelectResult}
            onOpenResult={props.onOpenResult}
            onMemoryChanged={props.onMemoryChanged}
          />
        ) : null}
        {pane.kind === "sessions" ? (
          <SessionsPane
            snapshot={props.snapshot}
            startDir={props.projectRoot}
            reduceMotion={props.reduceMotion}
            onSelectResult={props.onSelectResult}
            onOpenResult={props.onOpenResult}
            onSessionChanged={props.onSessionChanged}
          />
        ) : null}
        {pane.kind === "state" ? (
          <StatePane
            snapshot={props.snapshot}
            startDir={props.projectRoot}
            reduceMotion={props.reduceMotion}
            onSelectResult={props.onSelectResult}
            onOpenResult={props.onOpenResult}
            onMemoryChanged={props.onMemoryChanged}
          />
        ) : null}
        {pane.kind === "decisions" ? (
          <DecisionsPane
            snapshot={props.snapshot}
            startDir={props.projectRoot}
            reduceMotion={props.reduceMotion}
            onSelectResult={props.onSelectResult}
            onOpenResult={props.onOpenResult}
            onMemoryChanged={props.onMemoryChanged}
          />
        ) : null}
        {pane.kind === "context" ? (
          <ContextPane
            snapshot={props.snapshot}
            startDir={props.projectRoot}
            reduceMotion={props.reduceMotion}
            onSelectResult={props.onSelectResult}
            onOpenResult={props.onOpenResult}
            onMemoryChanged={props.onMemoryChanged}
          />
        ) : null}
        {pane.kind === "settings" ? (
          <SettingsPane
            snapshot={props.snapshot}
            projectRoot={props.projectRoot}
            onProjectRootChange={props.onProjectRootChange}
            onInitializeProject={props.onInitializeProject}
          />
        ) : null}
        {pane.kind === "detail" ? (
          <DetailPane
            pane={pane}
            selectedResult={props.selectedResult}
            snapshot={props.snapshot}
            detail={props.recordDetail}
            loading={props.recordDetailLoading}
            error={props.recordDetailError}
            startDir={props.projectRoot}
            onMemoryChanged={props.onMemoryChanged}
          />
        ) : null}
        {pane.kind === "capture" ? (
          <CapturePane
            pane={pane}
            startDir={props.projectRoot}
            reduceMotion={props.reduceMotion}
            onCaptured={props.onCaptured}
          />
        ) : null}
      </div>
    </motion.article>
  );
}

function OverviewPane({ snapshot }: { snapshot: ProjectSnapshot | null }) {
  const report = snapshot?.report;
  const status = snapshot?.status;

  return (
    <div className="view-stack">
      <motion.section
        className="briefing-panel"
        initial={{ opacity: 0, y: 8 }}
        animate={{ opacity: 1, y: 0 }}
        transition={transition.quick}
      >
        <div className="briefing-copy">
          <span className="record-type">Next session brief</span>
          <h3>{snapshot?.project?.project ?? "Project memory"}</h3>
          <p>
            Review active state, durable decisions, and retrieval freshness before another AI
            session starts work.
          </p>
        </div>
        <div className="briefing-stats">
          <Metric label="Entities" value={report?.entity_count ?? 0} />
          <Metric label="Relations" value={report?.relation_count ?? 0} />
          <Metric label="Observations" value={report?.observation_count ?? 0} />
          <Metric label="Decisions" value={report?.decision_count ?? 0} />
        </div>
      </motion.section>

      {snapshot?.error ? (
        <section className="notice warn">
          <AlertTriangle size={18} />
          <span>{snapshot.error}</span>
        </section>
      ) : null}

      <motion.section className="dense-list" layout>
        <ListHeading title="Active Memory" icon={Activity} />
        {(status?.active_state.length ? status.active_state : ["No active state recorded"]).map((item) => (
          <Row key={item} title={item} meta="state" />
        ))}
        {(status?.active_sessions.length ? status.active_sessions : ["No active session"]).map((item) => (
          <Row key={item} title={item} meta="session" />
        ))}
      </motion.section>

      <motion.section className="dense-list" layout>
        <ListHeading title="Suggested Queries" icon={SearchIcon} />
        {(report?.suggested_queries.length ? report.suggested_queries : ["What should the next session know?"]).map(
          (query) => (
            <Row key={query} title={query} meta="query" />
          ),
        )}
      </motion.section>
    </div>
  );
}

function SearchPane(props: {
  pane: PaneState;
  snapshot: ProjectSnapshot | null;
  projectRoot: string;
  reduceMotion: boolean;
  onUpdate: (patch: Partial<PaneState>) => void;
  onSelectResult: (result: SearchResult) => void;
  onOpenResult: (result: SearchResult) => void;
  onMemoryChanged: () => Promise<ProjectSnapshot>;
}) {
  const [query, setQuery] = useState(props.pane.query ?? "");
  const [mode, setMode] = useState<SearchMode>(props.pane.mode ?? "hybrid");
  const [recordType, setRecordType] = useState(props.pane.recordType ?? "all");
  const [searchScope, setSearchScope] = useState(props.pane.scope ?? props.snapshot?.scope ?? "");
  const [results, setResults] = useState<SearchResult[]>([]);
  const [semanticAvailable, setSemanticAvailable] = useState(false);
  const [searching, setSearching] = useState(false);
  const [maintenanceBusy, setMaintenanceBusy] = useState(false);
  const [maintenanceMessage, setMaintenanceMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const embedding = props.snapshot?.embedding;
  const runtime = embedding?.runtime;

  useEffect(() => {
    const timer = window.setTimeout(() => {
      props.onUpdate({
        query,
        mode,
        recordType,
        scope: searchScope,
        title: titleForPane({ kind: "search", query, recordType }),
      });

      if (!query.trim()) {
        setResults([]);
        setSemanticAvailable(Boolean(runtime && runtime.indexed_records > 0));
        return;
      }

      setSearching(true);
      searchProjectMemory({
        startDir: props.projectRoot,
        query,
        mode,
        scope: searchScope,
        recordType,
        limit: 20,
      })
        .then((report) => {
          setResults(report.results);
          setSemanticAvailable(report.semantic_available);
          setError(report.fallback ?? null);
        })
        .catch((searchError) => {
          setResults([]);
          setSemanticAvailable(false);
          setError(String(searchError));
        })
        .finally(() => {
          setSearching(false);
        });
    }, 180);

    return () => window.clearTimeout(timer);
  }, [query, mode, recordType, searchScope, props.projectRoot, runtime?.indexed_records]);

  async function processSearchEmbeddings(rebuild: boolean) {
    setMaintenanceBusy(true);
    setMaintenanceMessage(null);
    setError(null);
    try {
      const result = await processProjectEmbeddings({
        startDir: props.projectRoot,
        scope: rebuild ? searchScope || "*" : searchScope || props.snapshot?.scope || "*",
        rebuild,
        limit: 100,
      });
      setMaintenanceMessage(
        `${rebuild ? "Rebuilt" : "Processed"} embeddings: ${result.processed} processed, ${result.pending_remaining} pending.`,
      );
      await props.onMemoryChanged();
    } catch (embeddingError) {
      setError(String(embeddingError));
    } finally {
      setMaintenanceBusy(false);
    }
  }

  return (
    <div className="view-stack search-view">
      <div className="search-box">
        <SearchIcon size={17} />
        <input
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          placeholder="Search project memory"
          autoComplete="off"
        />
      </div>

      <div className="toolbar-row">
        <Segmented
          value={mode}
          options={["hybrid", "keyword", "semantic"]}
          onChange={(next) => setMode(next as SearchMode)}
        />
        <label className="compact-select">
          <span>Type</span>
          <select value={recordType} onChange={(event) => setRecordType(event.target.value)}>
            {searchRecordTypes.map((option) => (
              <option key={option} value={option}>
                {option}
              </option>
            ))}
          </select>
        </label>
      </div>

      <div className="search-filter-row">
        <label>
          <span>Scope</span>
          <input value={searchScope} onChange={(event) => setSearchScope(event.target.value)} placeholder="global or project/module" />
        </label>
        <div className="retrieval-health">
          <strong>{results.length}</strong>
          <span>{searching ? "searching" : semanticAvailable ? "semantic ready" : "keyword path"}</span>
        </div>
      </div>

      <section className="retrieval-panel">
        <div>
          <span className="record-type">Retrieval Index</span>
          <strong>{runtime ? `${runtime.fresh_records}/${runtime.embeddable_records} fresh` : "No index status"}</strong>
          <p>
            {runtime
              ? `${runtime.provider} / ${runtime.vector_backend} / ${runtime.missing_or_stale_records} stale`
              : "Initialize a Grafiki project to inspect embedding freshness."}
          </p>
        </div>
        <div className="retrieval-actions">
          <button
            className="button secondary"
            onClick={() => processSearchEmbeddings(false)}
            disabled={maintenanceBusy || !props.projectRoot.trim()}
          >
            <Sparkles size={15} />
            Process
          </button>
          <button
            className="button secondary"
            onClick={() => processSearchEmbeddings(true)}
            disabled={maintenanceBusy || !props.projectRoot.trim()}
          >
            <RefreshCcw size={15} />
            Rebuild
          </button>
        </div>
      </section>

      {error ? (
        <section className="notice compact">
          <AlertTriangle size={16} />
          <span>{error}</span>
        </section>
      ) : null}
      {maintenanceMessage ? <section className="notice compact good">{maintenanceMessage}</section> : null}

      <motion.section className="result-list" layout>
        <AnimatePresence mode="popLayout">
          {results.map((result, index) => (
          <motion.button
            key={result.id}
            layout
            className="result-row"
            onClick={() => props.onSelectResult(result)}
            onDoubleClick={() => props.onOpenResult(result)}
            initial={props.reduceMotion ? false : { opacity: 0, y: 8 }}
            animate={{ opacity: 1, y: 0 }}
            exit={props.reduceMotion ? undefined : { opacity: 0, y: -4 }}
            transition={{ ...transition.quick, delay: props.reduceMotion ? 0 : index * 0.025 }}
            whileHover={props.reduceMotion ? undefined : { x: 2 }}
            whileTap={props.reduceMotion ? undefined : { scale: 0.995 }}
          >
            <span className="record-type">{result.record_type}</span>
            <strong>{result.title}</strong>
            <span>{result.snippet}</span>
            <footer>
              <code>{result.scope || "global"}</code>
              <span>{typeof result.score === "number" ? result.score.toFixed(2) : "keyword"}</span>
            </footer>
          </motion.button>
          ))}
        </AnimatePresence>
      </motion.section>
    </div>
  );
}

function GraphPane(props: {
  pane: PaneState;
  snapshot: ProjectSnapshot | null;
  startDir: string;
  onUpdate: (patch: Partial<PaneState>) => void;
}) {
  const report = props.snapshot?.report;
  const candidates = useMemo(() => {
    const nodes = [...(report?.god_nodes ?? []), ...(report?.orphan_entities ?? [])];
    return nodes.filter((node, index) => nodes.findIndex((candidate) => candidate.id === node.id) === index);
  }, [report]);
  const root = props.pane.entityId ?? candidates[0]?.id ?? "";
  const [graph, setGraph] = useState<GraphReport | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!root) {
      setGraph(null);
      return;
    }

    let cancelled = false;
    setError(null);
    getMemoryGraph({ startDir: props.startDir, entityId: root, depth: 2 })
      .then((next) => {
        if (!cancelled) setGraph(next);
      })
      .catch((graphError) => {
        if (!cancelled) {
          setGraph(null);
          setError(String(graphError));
        }
      });

    return () => {
      cancelled = true;
    };
  }, [root, props.startDir]);

  const entityNames = new Map(graph?.entities.map((entity) => [entity.id, entity.name]) ?? []);
  const rows =
    graph?.relations.map((relation) => ({
      from: entityNames.get(relation.from_entity) ?? relation.from_entity,
      relation: relation.relation,
      to: entityNames.get(relation.to_entity) ?? relation.to_entity,
    })) ?? [];

  return (
    <motion.div
      className="relationship-ledger"
      initial={{ opacity: 0, y: 8 }}
      animate={{ opacity: 1, y: 0 }}
      transition={transition.quick}
    >
      <header>
        <span className="record-type">Relationship ledger</span>
        <strong>{root ? entityNames.get(root) ?? root : "No entity selected"}</strong>
      </header>
      {candidates.length ? (
        <div className="graph-toolbar">
          <select value={root} onChange={(event) => props.onUpdate({ entityId: event.target.value })}>
            {candidates.map((node) => (
              <option key={node.id} value={node.id}>
                {node.name} - {node.entity_type}
              </option>
            ))}
          </select>
          <span>{graph ? `${graph.entities.length} entities / ${graph.relations.length} relations` : "Loading"}</span>
        </div>
      ) : null}
      {error ? (
        <section className="notice compact">
          <AlertTriangle size={16} />
          <span>{error}</span>
        </section>
      ) : null}
      <div className="relation-table">
        {(rows.length
          ? rows
          : [
              { from: "Project", relation: "contains", to: `${report?.entity_count ?? 0} entities` },
              { from: "Entities", relation: "linked by", to: `${report?.relation_count ?? 0} relations` },
              { from: "Sessions", relation: "recall", to: `${report?.observation_count ?? 0} observations` },
              { from: "Decisions", relation: "shape", to: `${report?.decision_count ?? 0} active records` },
            ]
        ).map((row) => (
          <motion.div
            className="relation-row"
            key={`${row.from}-${row.relation}-${row.to}`}
            layout
            whileHover={{ x: 2 }}
            transition={transition.quick}
          >
            <code>{row.from}</code>
            <span>{row.relation}</span>
            <code>{row.to}</code>
          </motion.div>
        ))}
      </div>
    </motion.div>
  );
}

function SessionsPane(props: {
  snapshot: ProjectSnapshot | null;
  startDir: string;
  reduceMotion: boolean;
  onSelectResult: (result: SearchResult) => void;
  onOpenResult: (result: SearchResult) => void;
  onSessionChanged: (result: { record_type: string; id: string; title: string; scope: string; message: string }) => void;
}) {
  const sessions = props.snapshot?.status?.active_sessions ?? [];
  const [history, setHistory] = useState<SessionLogItem[]>([]);
  const [sessionType, setSessionType] = useState("codex");
  const [goal, setGoal] = useState("");
  const [scope, setScope] = useState("");
  const [endStatus, setEndStatus] = useState("completed");
  const [summary, setSummary] = useState("");
  const [accomplishments, setAccomplishments] = useState("");
  const [remaining, setRemaining] = useState("");
  const [filesChanged, setFilesChanged] = useState("");
  const [loadingHistory, setLoadingHistory] = useState(false);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [handoffReview, setHandoffReview] = useState<{ report: HandoffSessionResult; title: string } | null>(null);

  async function loadHistory() {
    setLoadingHistory(true);
    setError(null);
    try {
      setHistory(await listProjectSessions({ startDir: props.startDir }));
    } catch (listError) {
      setError(String(listError));
    } finally {
      setLoadingHistory(false);
    }
  }

  useEffect(() => {
    loadHistory();
  }, [props.startDir, props.snapshot]);

  async function start(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!goal.trim() || busy) return;
    setBusy(true);
    setMessage(null);
    setError(null);
    try {
      const report = await startSession({
        startDir: props.startDir,
        sessionType,
        goal,
        scope,
      });
      setHandoffReview(null);
      setMessage(`Started ${report.session_type} session.`);
      props.onSessionChanged({
        record_type: "session",
        id: report.session_id,
        title: report.goal,
        scope: report.scope,
        message: report.briefing,
      });
      await loadHistory();
      setGoal("");
    } catch (sessionError) {
      setError(String(sessionError));
    } finally {
      setBusy(false);
    }
  }

  async function end(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (busy) return;
    setBusy(true);
    setMessage(null);
    setError(null);
    try {
      const report = await endSession({
        startDir: props.startDir,
        status: endStatus,
        summary,
        accomplishments,
        remaining,
        filesChanged,
      });
      setHandoffReview(null);
      setMessage(`Ended session ${report.session_id}.`);
      props.onSessionChanged({
        record_type: "session",
        id: report.session_id,
        title: report.session_id,
        scope: "",
        message: report.summary || `Session ended with status ${report.status}.`,
      });
      await loadHistory();
      setSummary("");
      setAccomplishments("");
      setRemaining("");
      setFilesChanged("");
    } catch (sessionError) {
      setError(String(sessionError));
    } finally {
      setBusy(false);
    }
  }

  async function endSelected(session: SessionLogItem) {
    if (busy) return;
    if (!window.confirm(`End session "${session.goal || session.id}" as completed?`)) return;
    setBusy(true);
    setMessage(null);
    setError(null);
    try {
      const report = await endSession({
        startDir: props.startDir,
        sessionId: session.id,
        status: "completed",
        summary: session.summary ?? "",
        accomplishments: session.accomplishments.join(", "),
        remaining: session.remaining.join(", "),
        filesChanged: session.files_changed.join(", "),
      });
      setHandoffReview(null);
      setMessage(`Ended session ${report.session_id}.`);
      props.onSessionChanged({
        record_type: "session",
        id: report.session_id,
        title: session.goal || report.session_id,
        scope: session.scope,
        message: report.summary || `Session ended with status ${report.status}.`,
      });
      await loadHistory();
    } catch (sessionError) {
      setError(String(sessionError));
    } finally {
      setBusy(false);
    }
  }

  async function handoffSelected(session: SessionLogItem) {
    if (busy) return;
    if (!window.confirm(`Create a handoff from "${session.goal || session.id}"?`)) return;
    setBusy(true);
    setMessage(null);
    setError(null);
    try {
      const report = await handoffSession({
        startDir: props.startDir,
        sessionId: session.id,
      });
      setHandoffReview({ report, title: session.goal || report.child_session_id });
      setMessage(`Created handoff session ${report.child_session_id}.`);
      props.onSessionChanged({
        record_type: "session",
        id: report.child_session_id,
        title: session.goal || report.child_session_id,
        scope: report.scope,
        message: report.handoff_context,
      });
      await loadHistory();
    } catch (sessionError) {
      setError(String(sessionError));
    } finally {
      setBusy(false);
    }
  }

  async function copyHandoffContext() {
    if (!handoffReview) return;
    setMessage(null);
    setError(null);
    try {
      await navigator.clipboard.writeText(handoffReview.report.handoff_context);
      setMessage("Copied handoff context.");
    } catch (clipboardError) {
      setError(String(clipboardError));
    }
  }

  function openHandoffSession(id: string, label: string) {
    if (!handoffReview) return;
    props.onOpenResult({
      record_type: "session",
      id,
      title: label,
      scope: handoffReview.report.scope,
      snippet: handoffReview.report.handoff_context,
    });
  }

  return (
    <div className="view-stack">
      <EntityList
        title="Active Sessions"
        icon={History}
        rows={sessions.length ? sessions : ["No active session"]}
        meta="handoff chain"
      />

      <MemoryListHeader
        title="Session History"
        icon={History}
        loading={loadingHistory}
        onRefresh={loadHistory}
      />
      <section className="record-list">
        {history.length ? (
          history.map((session, index) => {
            const result = sessionToSearchResult(session);
            return (
              <MemoryRecordRow
                key={session.id}
                type="session"
                title={session.goal || session.id}
                body={sessionBody(session)}
                scope={session.scope}
                meta={`${session.status} / ${session.session_type}`}
                index={index}
                reduceMotion={props.reduceMotion}
                onSelect={() => props.onSelectResult(result)}
                onOpen={() => props.onOpenResult(result)}
                onEdit={() => props.onOpenResult(result)}
                onHandoff={session.status === "active" ? () => handoffSelected(session) : undefined}
                onComplete={session.status === "active" ? () => endSelected(session) : undefined}
              />
            );
          })
        ) : (
          <EmptyRecordList text="No sessions recorded." />
        )}
      </section>

      {message ? <section className="notice compact good">{message}</section> : null}
      {error ? (
        <section className="notice compact">
          <AlertTriangle size={16} />
          <span>{error}</span>
        </section>
      ) : null}

      {handoffReview ? (
        <motion.section
          className="handoff-review"
          layout
          initial={props.reduceMotion ? false : { opacity: 0, y: 8 }}
          animate={{ opacity: 1, y: 0 }}
          transition={transition.quick}
        >
          <header>
            <div>
              <span className="record-type">Handoff Review</span>
              <strong>{handoffReview.title}</strong>
            </div>
            <code>{handoffReview.report.scope || "global"}</code>
          </header>
          <div className="handoff-review-facts">
            <div>
              <span>Parent</span>
              <code>{handoffReview.report.parent_session_id}</code>
            </div>
            <div>
              <span>Child</span>
              <code>{handoffReview.report.child_session_id}</code>
            </div>
            <div>
              <span>Project</span>
              <code>{handoffReview.report.project}</code>
            </div>
          </div>
          <pre>{handoffReview.report.handoff_context}</pre>
          <div className="handoff-review-actions">
            <button className="button secondary" type="button" onClick={copyHandoffContext}>
              <FileText size={15} />
              Copy Context
            </button>
            <button
              className="button secondary"
              type="button"
              onClick={() => openHandoffSession(handoffReview.report.parent_session_id, "Parent Session")}
            >
              <History size={15} />
              Open Parent
            </button>
            <button
              className="button primary"
              type="button"
              onClick={() => openHandoffSession(handoffReview.report.child_session_id, "Child Session")}
            >
              <SplitSquareHorizontal size={15} />
              Open Child
            </button>
          </div>
        </motion.section>
      ) : null}

      <form className="capture-form" onSubmit={start}>
        <ListHeading title="Start Session" icon={Plus} />
        <div className="metadata-grid">
          <label>
            <span>Type</span>
            <select value={sessionType} onChange={(event) => setSessionType(event.target.value)}>
              {sessionTypes.map((option) => (
                <option key={option} value={option}>
                  {option}
                </option>
              ))}
            </select>
          </label>
          <label>
            <span>Scope</span>
            <input value={scope} onChange={(event) => setScope(event.target.value)} placeholder="project/module" />
          </label>
        </div>
        <label>
          <span>Goal</span>
          <textarea value={goal} onChange={(event) => setGoal(event.target.value)} placeholder="What should this session accomplish?" />
        </label>
        <div className="form-actions">
          <button className="button primary" disabled={!goal.trim() || busy}>
            Start
          </button>
        </div>
      </form>

      <form className="capture-form" onSubmit={end}>
        <ListHeading title="End Latest Active Session" icon={CheckCircle2} />
        <div className="metadata-grid">
          <label>
            <span>Status</span>
            <select value={endStatus} onChange={(event) => setEndStatus(event.target.value)}>
              {endStatuses.map((option) => (
                <option key={option} value={option}>
                  {option}
                </option>
              ))}
            </select>
          </label>
        </div>
        <label>
          <span>Summary</span>
          <textarea value={summary} onChange={(event) => setSummary(event.target.value)} placeholder="What changed, what remains, and what should the next session know?" />
        </label>
        <div className="metadata-grid">
          <label>
            <span>Accomplishments</span>
            <input
              value={accomplishments}
              onChange={(event) => setAccomplishments(event.target.value)}
              placeholder="comma separated"
            />
          </label>
          <label>
            <span>Remaining</span>
            <input value={remaining} onChange={(event) => setRemaining(event.target.value)} placeholder="comma separated" />
          </label>
          <label>
            <span>Files Changed</span>
            <input
              value={filesChanged}
              onChange={(event) => setFilesChanged(event.target.value)}
              placeholder="comma separated paths"
            />
          </label>
        </div>
        <div className="form-actions">
          <button className="button secondary" disabled={busy}>
            End Session
          </button>
        </div>
      </form>
    </div>
  );
}

function DecisionsPane(props: {
  snapshot: ProjectSnapshot | null;
  startDir: string;
  reduceMotion: boolean;
  onSelectResult: (result: SearchResult) => void;
  onOpenResult: (result: SearchResult) => void;
  onMemoryChanged: () => Promise<ProjectSnapshot>;
}) {
  const [decisions, setDecisions] = useState<DecisionItem[]>([]);
  const [loading, setLoading] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  async function load() {
    setLoading(true);
    setError(null);
    try {
      setDecisions(await listProjectDecisions({ startDir: props.startDir }));
    } catch (listError) {
      setError(String(listError));
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    load();
  }, [props.startDir, props.snapshot]);

  async function remove(decision: DecisionItem) {
    if (!window.confirm(`Delete decision "${decision.title}"?`)) return;
    setMessage(null);
    setError(null);
    try {
      const result = await deleteMemoryRecord({
        startDir: props.startDir,
        recordType: "decision",
        id: decision.id,
      });
      setDecisions((current) => current.filter((candidate) => candidate.id !== decision.id));
      setMessage(result.message);
      await props.onMemoryChanged();
    } catch (deleteError) {
      setError(String(deleteError));
    }
  }

  return (
    <div className="view-stack">
      <MemoryListHeader
        title="Decisions"
        icon={GitBranch}
        loading={loading}
        onRefresh={load}
      />
      {message ? <section className="notice compact good">{message}</section> : null}
      {error ? (
        <section className="notice compact">
          <AlertTriangle size={16} />
          <span>{error}</span>
        </section>
      ) : null}
      <section className="record-list">
        {decisions.length ? (
          decisions.map((decision, index) => {
            const result = decisionToSearchResult(decision);
            return (
              <MemoryRecordRow
                key={decision.id}
                type="decision"
                title={decision.title}
                body={decision.reasoning || "No reasoning recorded yet."}
                scope={decision.scope}
                meta={decision.status}
                index={index}
                reduceMotion={props.reduceMotion}
                onSelect={() => props.onSelectResult(result)}
                onOpen={() => props.onOpenResult(result)}
                onEdit={() => props.onOpenResult(result)}
                onDelete={() => remove(decision)}
              />
            );
          })
        ) : (
          <EmptyRecordList text="No decisions recorded." />
        )}
      </section>
    </div>
  );
}

function CandidatesPane(props: {
  snapshot: ProjectSnapshot | null;
  startDir: string;
  reduceMotion: boolean;
  active: boolean;
  onSelectResult: (result: SearchResult) => void;
  onOpenResult: (result: SearchResult) => void;
  onMemoryChanged: () => Promise<ProjectSnapshot>;
}) {
  const [candidates, setCandidates] = useState<ExtractionCandidate[]>([]);
  const [captureStatus, setCaptureStatus] = useState<RawCaptureStatus | null>(null);
  const [status, setStatus] = useState("pending");
  const [scope, setScope] = useState(props.snapshot?.scope ?? "");
  const [loading, setLoading] = useState(false);
  const [capturing, setCapturing] = useState(false);
  const [busyId, setBusyId] = useState<string | null>(null);
  const [selectedIds, setSelectedIds] = useState<string[]>([]);
  const [focusedCandidateId, setFocusedCandidateId] = useState<string | null>(null);
  const [minConfidence, setMinConfidence] = useState("0");
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editPayload, setEditPayload] = useState("");
  const [editScope, setEditScope] = useState("");
  const [editConfidence, setEditConfidence] = useState("0.5");
  const [editRationale, setEditRationale] = useState("");
  const [evidencePreview, setEvidencePreview] = useState<EvidenceLink | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const minConfidenceValue = Number(minConfidence);
  const visibleCandidates = useMemo(
    () =>
      candidates.filter((candidate) => {
        if (!Number.isFinite(minConfidenceValue) || minConfidenceValue <= 0) return true;
        return candidate.confidence >= minConfidenceValue || selectedIds.includes(candidate.id) || editingId === candidate.id;
      }),
    [candidates, editingId, minConfidenceValue, selectedIds],
  );
  const candidateGroups = useMemo(() => groupCandidates(visibleCandidates), [visibleCandidates]);

  async function load() {
    setLoading(true);
    setError(null);
    try {
      const [nextCandidates, nextCaptureStatus] = await Promise.all([
        listCandidates({
          startDir: props.startDir,
          scope,
          status,
          limit: 100,
        }),
        getAutomaticCaptureStatus({
          startDir: props.startDir,
          scope,
        }),
      ]);
      setCandidates(nextCandidates);
      setSelectedIds((ids) =>
        ids.filter((id) => nextCandidates.some((candidate) => candidate.id === id && candidate.status === "pending")),
      );
      setFocusedCandidateId((id) => {
        if (id && nextCandidates.some((candidate) => candidate.id === id)) return id;
        return nextCandidates.find((candidate) => candidate.status === "pending")?.id ?? nextCandidates[0]?.id ?? null;
      });
      setCaptureStatus(nextCaptureStatus);
    } catch (listError) {
      setError(String(listError));
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    load();
  }, [props.startDir, props.snapshot, scope, status]);

  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      // Only the active pane handles global candidate shortcuts; otherwise
      // pressing `a`/`r` while reading another pane would silently mutate
      // trusted memory.
      if (!props.active) {
        return;
      }
      const target = event.target as HTMLElement | null;
      if (
        target?.closest("input, textarea, select, button") ||
        editingId ||
        busyId ||
        !visibleCandidates.length
      ) {
        return;
      }

      const currentIndex = Math.max(
        0,
        visibleCandidates.findIndex((candidate) => candidate.id === focusedCandidateId),
      );
      if (event.key === "j" || event.key === "ArrowDown") {
        event.preventDefault();
        setFocusedCandidateId(visibleCandidates[Math.min(visibleCandidates.length - 1, currentIndex + 1)].id);
      } else if (event.key === "k" || event.key === "ArrowUp") {
        event.preventDefault();
        setFocusedCandidateId(visibleCandidates[Math.max(0, currentIndex - 1)].id);
      } else if (["a", "r", "e", "o", "v", " "].includes(event.key)) {
        const candidate = visibleCandidates[currentIndex];
        if (!candidate) return;
        event.preventDefault();
        if (event.key === "a" && candidate.status === "pending") void approve(candidate);
        if (event.key === "r" && candidate.status === "pending") void reject(candidate);
        if (event.key === "e" && candidate.status === "pending") beginEdit(candidate);
        if (event.key === "o") openTrusted(candidate);
        if (event.key === "v") openEvidencePreview(candidate.evidence?.[0] ?? null);
        if (event.key === " ") toggleSelected(candidate, !selectedIds.includes(candidate.id));
      }
    }

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [props.active, busyId, editingId, focusedCandidateId, selectedIds, visibleCandidates]);

  async function approve(candidate: ExtractionCandidate) {
    setBusyId(candidate.id);
    setMessage(null);
    setError(null);
    try {
      const result = await approveCandidate({ startDir: props.startDir, id: candidate.id });
      setMessage(result.message);
      const trustedResult = candidateToSearchResult(result.candidate);
      if (trustedResult) props.onSelectResult(trustedResult);
      await load();
      await props.onMemoryChanged();
    } catch (approveError) {
      setError(String(approveError));
    } finally {
      setBusyId(null);
    }
  }

  async function reject(candidate: ExtractionCandidate) {
    const rationale = window.prompt("Reject rationale", candidate.rationale ?? "");
    if (rationale === null) return;
    setBusyId(candidate.id);
    setMessage(null);
    setError(null);
    try {
      const result = await rejectCandidate({ startDir: props.startDir, id: candidate.id, rationale });
      setMessage(result.message);
      await load();
      await props.onMemoryChanged();
    } catch (rejectError) {
      setError(String(rejectError));
    } finally {
      setBusyId(null);
    }
  }

  function toggleSelected(candidate: ExtractionCandidate, checked: boolean) {
    if (candidate.status !== "pending") return;
    setSelectedIds((ids) => {
      if (checked) return ids.includes(candidate.id) ? ids : [...ids, candidate.id];
      return ids.filter((id) => id !== candidate.id);
    });
  }

  function selectLowConfidence() {
    setSelectedIds(
      candidates
        .filter((candidate) => candidate.status === "pending" && candidateIsNoisy(candidate))
        .map((candidate) => candidate.id),
    );
  }

  function selectAllPending() {
    setSelectedIds(candidates.filter((candidate) => candidate.status === "pending").map((candidate) => candidate.id));
  }

  function beginEdit(candidate: ExtractionCandidate) {
    setEditingId(candidate.id);
    setEditPayload(JSON.stringify(candidate.payload, null, 2));
    setEditScope(candidate.scope);
    setEditConfidence(String(candidate.confidence));
    setEditRationale(candidate.rationale ?? "");
    setMessage(null);
    setError(null);
  }

  async function saveEdit(candidate: ExtractionCandidate) {
    let payload: Record<string, unknown>;
    try {
      const parsed = JSON.parse(editPayload);
      if (!parsed || Array.isArray(parsed) || typeof parsed !== "object") {
        throw new Error("Candidate payload must be a JSON object.");
      }
      payload = parsed as Record<string, unknown>;
    } catch (parseError) {
      setError(String(parseError));
      return;
    }

    const confidence = Number(editConfidence);
    if (!Number.isFinite(confidence) || confidence < 0 || confidence > 1) {
      setError("Confidence must be a number from 0 to 1.");
      return;
    }

    setBusyId(candidate.id);
    setMessage(null);
    setError(null);
    try {
      const result = await editCandidate({
        startDir: props.startDir,
        id: candidate.id,
        payload,
        scope: editScope,
        confidence,
        rationale: editRationale,
      });
      setMessage(result.message);
      setEditingId(null);
      await load();
    } catch (editError) {
      setError(String(editError));
    } finally {
      setBusyId(null);
    }
  }

  async function bulkReview(action: "approve" | "reject", ids = selectedIds) {
    if (!ids.length) {
      setMessage("Select at least one pending candidate.");
      return;
    }
    const rationale =
      action === "reject"
        ? window.prompt("Reject rationale", "Bulk review cleanup") ?? null
        : "";
    if (rationale === null) return;

    setBusyId("bulk");
    setMessage(null);
    setError(null);
    try {
      const result = await bulkReviewCandidates({
        startDir: props.startDir,
        action,
        ids,
        rationale,
      });
      setMessage(`${result.action} complete: ${result.succeeded}/${result.requested} candidates reviewed.`);
      if (result.failed) {
        setError(result.errors.map((item) => `${item.id}: ${item.error}`).join("\n"));
      }
      setSelectedIds([]);
      await load();
      await props.onMemoryChanged();
    } catch (bulkError) {
      setError(String(bulkError));
    } finally {
      setBusyId(null);
    }
  }

  function openTrusted(candidate: ExtractionCandidate) {
    const result = candidateToSearchResult(candidate);
    if (!result) {
      setMessage("Approve this candidate before opening it as trusted memory.");
      return;
    }
    props.onOpenResult(result);
  }

  function openEvidencePreview(evidence: EvidenceLink | null) {
    if (!evidence) {
      setMessage("No evidence attached to this candidate yet.");
      return;
    }
    setEvidencePreview(evidence);
    setMessage(null);
  }

  function groupPendingIds(group: CandidateGroup) {
    return group.candidates.filter((candidate) => candidate.status === "pending").map((candidate) => candidate.id);
  }

  function groupNoisyIds(group: CandidateGroup) {
    return group.candidates
      .filter((candidate) => candidate.status === "pending" && candidateIsNoisy(candidate))
      .map((candidate) => candidate.id);
  }

  async function autoCapture() {
    setCapturing(true);
    setMessage(null);
    setError(null);
    try {
      const result = await autoCaptureMemory({
        startDir: props.startDir,
        scope,
        source: "desktop-review",
        limit: 40,
      });
      setMessage(`${result.message} ${result.changed_files.length} changed files found.`);
      await load();
      await props.onMemoryChanged();
    } catch (captureError) {
      setError(String(captureError));
    } finally {
      setCapturing(false);
    }
  }

  async function startRawCapture() {
    setCapturing(true);
    setMessage(null);
    setError(null);
    try {
      const result = await startAutomaticCapture({
        startDir: props.startDir,
        scope,
        sourceApp: "grafiki-desktop",
      });
      setMessage(result.message);
      await load();
    } catch (captureError) {
      setError(String(captureError));
    } finally {
      setCapturing(false);
    }
  }

  async function stopRawCapture() {
    const activeId = captureStatus?.active_sessions[0]?.id;
    if (!activeId) {
      setMessage("No active capture session to stop.");
      return;
    }
    setCapturing(true);
    setMessage(null);
    setError(null);
    try {
      const result = await stopAutomaticCapture({
        startDir: props.startDir,
        captureId: activeId,
      });
      setMessage(result.message);
      await load();
    } catch (captureError) {
      setError(String(captureError));
    } finally {
      setCapturing(false);
    }
  }

  async function captureScreen() {
    setCapturing(true);
    setMessage(null);
    setError(null);
    try {
      const result = await captureScreenSnapshot({
        startDir: props.startDir,
        scope,
        captureId: captureStatus?.active_sessions[0]?.id,
      });
      setMessage(result.message);
      await load();
    } catch (captureError) {
      setError(String(captureError));
    } finally {
      setCapturing(false);
    }
  }

  async function importTranscript() {
    const agentInput = window.prompt("Agent transcript", "codex")?.trim().toLowerCase();
    if (!agentInput) return;
    if (!["codex", "claude-code", "cursor", "generic"].includes(agentInput)) {
      setError("Agent must be codex, claude-code, cursor, or generic.");
      return;
    }
    const inputPath = window.prompt("Transcript file or folder", "") ?? "";

    setCapturing(true);
    setMessage(null);
    setError(null);
    try {
      const result = await importAgentTranscripts({
        startDir: props.startDir,
        scope,
        agent: agentInput as AgentTranscriptImportInput["agent"],
        input: inputPath.trim(),
        limit: 200,
        summarize: true,
      });
      const proposed = result.candidates?.candidates.length ?? 0;
      setMessage(`${result.message} ${proposed} candidates proposed.`);
      await load();
      await props.onMemoryChanged();
    } catch (captureError) {
      setError(String(captureError));
    } finally {
      setCapturing(false);
    }
  }

  async function summarizeCapture() {
    setCapturing(true);
    setMessage(null);
    setError(null);
    try {
      const result = await summarizeAutomaticCapture({
        startDir: props.startDir,
        scope,
        captureId: captureStatus?.active_sessions[0]?.id,
        limit: 80,
      });
      setMessage(`${result.message} ${result.events_summarized} events summarized.`);
      await load();
      await props.onMemoryChanged();
    } catch (captureError) {
      setError(String(captureError));
    } finally {
      setCapturing(false);
    }
  }

  return (
    <div className="view-stack">
      <MemoryListHeader
        title="Memory Review"
        icon={ShieldQuestion}
        loading={loading}
        onRefresh={load}
      />
      <div className="toolbar-row candidate-toolbar">
        <label className="compact-select">
          <span>Status</span>
          <select value={status} onChange={(event) => setStatus(event.target.value)}>
            {candidateStatuses.map((option) => (
              <option key={option} value={option}>
                {option}
              </option>
            ))}
          </select>
        </label>
        <label className="compact-input">
          <span>Scope</span>
          <input value={scope} onChange={(event) => setScope(event.target.value)} placeholder="global or project/module" />
        </label>
        <label className="compact-input confidence-filter">
          <span>Min Confidence</span>
          <input
            value={minConfidence}
            onChange={(event) => setMinConfidence(event.target.value)}
            inputMode="decimal"
            placeholder="0"
          />
        </label>
        <button className="button primary" type="button" onClick={autoCapture} disabled={capturing}>
          <Sparkles size={15} />
          {capturing ? "Capturing" : "Auto Capture"}
        </button>
        <span className="subtle">{visibleCandidates.length}/{candidates.length} candidates</span>
      </div>
      <div className="toolbar-row candidate-toolbar">
        <span className="subtle">{selectedIds.length} selected</span>
        <button className="button" type="button" onClick={selectAllPending} disabled={!candidates.some((candidate) => candidate.status === "pending")}>
          <CheckCircle2 size={15} />
          Select Pending
        </button>
        <button className="button" type="button" onClick={selectLowConfidence} disabled={!candidates.some((candidate) => candidate.status === "pending" && candidateIsNoisy(candidate))}>
          <ShieldQuestion size={15} />
          Select Noisy
        </button>
        <button className="button primary" type="button" onClick={() => bulkReview("approve")} disabled={!selectedIds.length || Boolean(busyId)}>
          <CheckCircle2 size={15} />
          Approve Selected
        </button>
        <button className="button danger-button" type="button" onClick={() => bulkReview("reject")} disabled={!selectedIds.length || Boolean(busyId)}>
          <Trash2 size={15} />
          Reject Selected
        </button>
      </div>
      <div className="toolbar-row candidate-toolbar">
        <span className="subtle">
          {captureStatus?.active_sessions.length ?? 0} live / {captureStatus?.event_count ?? 0} raw events
        </span>
        <button className="button" type="button" onClick={startRawCapture} disabled={capturing}>
          <CircleDot size={15} />
          Start
        </button>
        <button className="button" type="button" onClick={stopRawCapture} disabled={capturing || !captureStatus?.active_sessions.length}>
          <X size={15} />
          Stop
        </button>
        <button className="button" type="button" onClick={captureScreen} disabled={capturing}>
          <PanelRight size={15} />
          Screen
        </button>
        <button className="button" type="button" onClick={importTranscript} disabled={capturing}>
          <Upload size={15} />
          Transcript
        </button>
        <button className="button" type="button" onClick={summarizeCapture} disabled={capturing || !(captureStatus?.event_count ?? 0)}>
          <Sparkles size={15} />
          Summarize
        </button>
      </div>
      {message ? <section className="notice compact good">{message}</section> : null}
      {evidencePreview ? (
        <section className="notice compact evidence-preview">
          <FileText size={16} />
          <span>
            <strong>{evidencePreview.title ?? evidencePreview.source ?? evidencePreview.source_type}</strong>
            {evidencePreview.excerpt ? ` ${evidencePreview.excerpt}` : ""}
          </span>
          <button className="icon-button" type="button" onClick={() => setEvidencePreview(null)} title="Dismiss evidence">
            <X size={14} />
          </button>
        </section>
      ) : null}
      {error ? (
        <section className="notice compact">
          <AlertTriangle size={16} />
          <span>{error}</span>
        </section>
      ) : null}
      <section className="candidate-group-list">
        {candidateGroups.length ? (
          candidateGroups.map((group, groupIndex) => {
            const pendingIds = groupPendingIds(group);
            const noisyIds = groupNoisyIds(group);
            return (
              <section className="candidate-group" key={group.key}>
                <header className="candidate-group-header">
                  <div>
                    <strong>{group.title}</strong>
                    <span>{group.meta}</span>
                  </div>
                  <div className="candidate-actions">
                    <button className="button" type="button" onClick={() => setSelectedIds((ids) => mergeIds(ids, pendingIds))} disabled={!pendingIds.length || Boolean(busyId)}>
                      Select Group
                    </button>
                    <button className="button primary" type="button" onClick={() => bulkReview("approve", pendingIds)} disabled={!pendingIds.length || Boolean(busyId)}>
                      Approve Group
                    </button>
                    <button className="button danger-button" type="button" onClick={() => bulkReview("reject", noisyIds)} disabled={!noisyIds.length || Boolean(busyId)}>
                      Reject Noisy
                    </button>
                  </div>
                </header>
                <div className="record-list candidate-list">
                  {group.candidates.map((candidate, index) => {
                    const trustedResult = candidateToSearchResult(candidate);
                    const isEditing = editingId === candidate.id;
                    const isBusy = busyId === candidate.id || busyId === "bulk";
                    const isFocused = focusedCandidateId === candidate.id;
                    return (
                      <motion.article
                        key={candidate.id}
                        className={`candidate-card ${selectedIds.includes(candidate.id) ? "selected" : ""} ${isFocused ? "focused" : ""} ${candidateIsNoisy(candidate) ? "noisy" : ""}`}
                        initial={props.reduceMotion ? false : { opacity: 0, y: 8 }}
                        animate={{ opacity: 1, y: 0 }}
                        transition={{ delay: props.reduceMotion ? 0 : (groupIndex + index) * 0.015, duration: 0.18 }}
                        onMouseEnter={() => setFocusedCandidateId(candidate.id)}
                        onClick={() => setFocusedCandidateId(candidate.id)}
                      >
                        <header className="candidate-card-header">
                          <label className="candidate-select">
                            <input
                              type="checkbox"
                              checked={selectedIds.includes(candidate.id)}
                              disabled={candidate.status !== "pending" || isBusy}
                              onChange={(event) => toggleSelected(candidate, event.currentTarget.checked)}
                            />
                            <span>
                              <strong>{candidateTitle(candidate)}</strong>
                              <small>{candidate.record_type} / {candidate.status} / {Math.round(candidate.confidence * 100)}% / {candidateCreatedDateLabel(candidate)}</small>
                            </span>
                          </label>
                          <div className="candidate-actions">
                            <button className="icon-button" type="button" onClick={() => beginEdit(candidate)} disabled={candidate.status !== "pending" || isBusy} title="Edit candidate">
                              <Pencil size={15} />
                            </button>
                            <button className="icon-button success" type="button" onClick={() => approve(candidate)} disabled={candidate.status !== "pending" || isBusy} title="Approve candidate">
                              <CheckCircle2 size={15} />
                            </button>
                            <button className="icon-button danger" type="button" onClick={() => reject(candidate)} disabled={candidate.status !== "pending" || isBusy} title="Reject candidate">
                              <Trash2 size={15} />
                            </button>
                            <button className="icon-button" type="button" onClick={() => openTrusted(candidate)} disabled={!trustedResult} title="Open trusted memory">
                              <FileText size={15} />
                            </button>
                          </div>
                        </header>
                        {isEditing ? (
                          <div className="candidate-edit-grid">
                            <label>
                              <span>Scope</span>
                              <input value={editScope} onChange={(event) => setEditScope(event.target.value)} />
                            </label>
                            <label>
                              <span>Confidence</span>
                              <input value={editConfidence} onChange={(event) => setEditConfidence(event.target.value)} inputMode="decimal" />
                            </label>
                            <label className="candidate-edit-wide">
                              <span>Rationale</span>
                              <input value={editRationale} onChange={(event) => setEditRationale(event.target.value)} />
                            </label>
                            <label className="candidate-edit-wide">
                              <span>Payload</span>
                              <textarea value={editPayload} onChange={(event) => setEditPayload(event.target.value)} spellCheck={false} />
                            </label>
                            <div className="candidate-edit-actions candidate-edit-wide">
                              <button className="button primary" type="button" onClick={() => saveEdit(candidate)} disabled={isBusy}>
                                Save
                              </button>
                              <button className="button" type="button" onClick={() => setEditingId(null)} disabled={isBusy}>
                                Cancel
                              </button>
                            </div>
                          </div>
                        ) : (
                          <>
                            <p>{candidateBody(candidate)}</p>
                            <div className="candidate-meta-row">
                              <span>{candidate.scope || "global"}</span>
                              <span>{candidate.source_type}</span>
                              {candidate.source ? <span>{candidate.source}</span> : null}
                              {candidateIsNoisy(candidate) ? <span>low confidence</span> : null}
                            </div>
                            {candidate.evidence?.length ? (
                              <div className="evidence-chip-row">
                                {candidate.evidence.slice(0, 4).map((evidence) => (
                                  <button
                                    className="evidence-chip"
                                    type="button"
                                    key={evidence.id}
                                    title={evidence.excerpt}
                                    onClick={(event) => {
                                      event.stopPropagation();
                                      openEvidencePreview(evidence);
                                    }}
                                  >
                                    {evidence.source_type}: {evidence.title ?? evidence.source ?? "source"}
                                  </button>
                                ))}
                              </div>
                            ) : null}
                          </>
                        )}
                      </motion.article>
                    );
                  })}
                </div>
              </section>
            );
          })
        ) : (
          <EmptyRecordList text="No candidates in this view." />
        )}
      </section>
    </div>
  );
}

function AgentActivityPane(props: {
  snapshot: ProjectSnapshot | null;
  startDir: string;
  reduceMotion: boolean;
}) {
  const [queries, setQueries] = useState<AgentQueryLogItem[]>([]);
  const [scope, setScope] = useState(props.snapshot?.scope ?? "");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function load() {
    setLoading(true);
    setError(null);
    try {
      setQueries(
        await listAgentActivity({
          startDir: props.startDir,
          scope,
          limit: 80,
        }),
      );
    } catch (activityError) {
      setError(String(activityError));
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    load();
  }, [props.startDir, props.snapshot, scope]);

  return (
    <div className="view-stack">
      <MemoryListHeader title="Agent Activity" icon={RadioTower} loading={loading} onRefresh={load} />
      <div className="toolbar-row candidate-toolbar">
        <label className="compact-input">
          <span>Scope</span>
          <input value={scope} onChange={(event) => setScope(event.target.value)} placeholder="global or project/module" />
        </label>
        <span className="subtle">{queries.length} queries</span>
      </div>
      {error ? (
        <section className="notice compact">
          <AlertTriangle size={16} />
          <span>{error}</span>
        </section>
      ) : null}
      <section className="record-list candidate-list">
        {queries.length ? (
          queries.map((query, index) => (
            <MemoryRecordRow
              key={query.id}
              type={query.agent}
              title={query.question}
              body={`${query.returned_ids.length ? query.returned_ids.join(", ") : "No trusted memory returned"}${
                query.fallback ? ` / ${query.fallback}` : ""
              }`}
              scope={query.scope}
              meta={`${query.retrieval_mode} / ${query.latency_ms}ms / ${query.created_at}`}
              index={index}
              reduceMotion={props.reduceMotion}
              onSelect={() => undefined}
              onOpen={() => undefined}
            />
          ))
        ) : (
          <EmptyRecordList text="No agent queries recorded yet." />
        )}
      </section>
    </div>
  );
}

function RelationsPane(props: {
  snapshot: ProjectSnapshot | null;
  startDir: string;
  reduceMotion: boolean;
  onSelectResult: (result: SearchResult) => void;
  onOpenResult: (result: SearchResult) => void;
  onMemoryChanged: () => Promise<ProjectSnapshot>;
}) {
  const [relations, setRelations] = useState<GraphRelation[]>([]);
  const [filter, setFilter] = useState("");
  const [loading, setLoading] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  async function load() {
    setLoading(true);
    setError(null);
    try {
      setRelations(
        await listProjectRelations({
          startDir: props.startDir,
          relation: filter,
        }),
      );
    } catch (listError) {
      setError(String(listError));
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    load();
  }, [props.startDir, props.snapshot, filter]);

  async function remove(relation: GraphRelation) {
    if (!window.confirm(`Remove relation "${relation.from_entity} ${relation.relation} ${relation.to_entity}"?`)) return;
    setMessage(null);
    setError(null);
    try {
      const result = await deleteMemoryRecord({
        startDir: props.startDir,
        recordType: "relation",
        id: relation.id,
      });
      setRelations((current) => current.filter((candidate) => candidate.id !== relation.id));
      setMessage(result.message);
      await props.onMemoryChanged();
    } catch (deleteError) {
      setError(String(deleteError));
    }
  }

  return (
    <div className="view-stack">
      <MemoryListHeader
        title="Relations"
        icon={GitBranch}
        loading={loading}
        onRefresh={load}
      />
      <div className="toolbar-row">
        <label className="compact-select">
          <span>Type</span>
          <select value={filter || "all"} onChange={(event) => setFilter(event.target.value === "all" ? "" : event.target.value)}>
            {["all", ...relationTypes].map((option) => (
              <option key={option} value={option}>
                {option}
              </option>
            ))}
          </select>
        </label>
        <span className="subtle">{props.snapshot?.report?.relation_count ?? 0} graph links</span>
      </div>
      {message ? <section className="notice compact good">{message}</section> : null}
      {error ? (
        <section className="notice compact">
          <AlertTriangle size={16} />
          <span>{error}</span>
        </section>
      ) : null}
      <section className="record-list">
        {relations.length ? (
          relations.map((relation, index) => {
            const result = relationToSearchResult(relation);
            return (
              <MemoryRecordRow
                key={relation.id}
                type="relation"
                title={`${relation.from_entity} ${relation.relation} ${relation.to_entity}`}
                body={`weight ${relation.weight.toFixed(2)} confidence ${relation.confidence.toFixed(2)} source ${relation.source_type}`}
                scope="relationship graph"
                meta={relation.id}
                index={index}
                reduceMotion={props.reduceMotion}
                onSelect={() => props.onSelectResult(result)}
                onOpen={() => props.onOpenResult(result)}
                onEdit={() => props.onOpenResult(result)}
                onDelete={() => remove(relation)}
              />
            );
          })
        ) : (
          <EmptyRecordList text="No relations recorded." />
        )}
      </section>
    </div>
  );
}

function StatePane(props: {
  snapshot: ProjectSnapshot | null;
  startDir: string;
  reduceMotion: boolean;
  onSelectResult: (result: SearchResult) => void;
  onOpenResult: (result: SearchResult) => void;
  onMemoryChanged: () => Promise<ProjectSnapshot>;
}) {
  const [items, setItems] = useState<StateItem[]>([]);
  const [loading, setLoading] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [editing, setEditing] = useState<StateItem | null>(null);
  const [editTitle, setEditTitle] = useState("");
  const [editScope, setEditScope] = useState("");
  const [editStatus, setEditStatus] = useState("in-progress");
  const [editPriority, setEditPriority] = useState("medium");
  const [saving, setSaving] = useState(false);

  async function load() {
    setLoading(true);
    setError(null);
    try {
      setItems(await listProjectState({ startDir: props.startDir }));
    } catch (listError) {
      setError(String(listError));
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    load();
  }, [props.startDir, props.snapshot]);

  async function remove(item: StateItem) {
    if (!window.confirm(`Delete state item "${item.title}"?`)) return;
    setMessage(null);
    setError(null);
    try {
      const result = await deleteMemoryRecord({
        startDir: props.startDir,
        recordType: "state",
        id: item.key,
      });
      setItems((current) => current.filter((candidate) => candidate.key !== item.key));
      setMessage(result.message);
      await props.onMemoryChanged();
    } catch (deleteError) {
      setError(String(deleteError));
    }
  }

  function beginEdit(item: StateItem) {
    setEditing(item);
    setEditTitle(item.title);
    setEditScope(item.scope);
    setEditStatus(item.status);
    setEditPriority(item.priority);
    setMessage(null);
    setError(null);
  }

  async function saveEdit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!editing || !editTitle.trim() || saving) return;
    setSaving(true);
    setMessage(null);
    setError(null);
    try {
      const result = await updateMemoryRecord({
        startDir: props.startDir,
        recordType: "state",
        id: editing.key,
        title: editTitle,
        scope: editScope,
        status: editStatus,
        priority: editPriority,
      });
      setMessage(result.message);
      setEditing(null);
      await load();
      await props.onMemoryChanged();
    } catch (editError) {
      setError(String(editError));
    } finally {
      setSaving(false);
    }
  }

  return (
    <div className="view-stack">
      <MemoryListHeader
        title="State"
        icon={Activity}
        loading={loading}
        onRefresh={load}
      />
      {message ? <section className="notice compact good">{message}</section> : null}
      {error ? (
        <section className="notice compact">
          <AlertTriangle size={16} />
          <span>{error}</span>
        </section>
      ) : null}
      {editing ? (
        <form className="inline-edit-form" onSubmit={saveEdit}>
          <ListHeading title={`Edit ${editing.key}`} icon={Pencil} />
          <label>
            <span>Title</span>
            <input value={editTitle} onChange={(event) => setEditTitle(event.target.value)} />
          </label>
          <div className="metadata-grid">
            <label>
              <span>Status</span>
              <select value={editStatus} onChange={(event) => setEditStatus(event.target.value)}>
                {stateStatuses.map((option) => (
                  <option key={option} value={option}>
                    {option}
                  </option>
                ))}
              </select>
            </label>
            <label>
              <span>Priority</span>
              <select value={editPriority} onChange={(event) => setEditPriority(event.target.value)}>
                {statePriorities.map((option) => (
                  <option key={option} value={option}>
                    {option}
                  </option>
                ))}
              </select>
            </label>
          </div>
          <label>
            <span>Scope</span>
            <input value={editScope} onChange={(event) => setEditScope(event.target.value)} />
          </label>
          <div className="form-actions">
            <button type="button" className="button secondary" onClick={() => setEditing(null)}>
              Cancel
            </button>
            <button className="button primary" disabled={saving || !editTitle.trim()}>
              Save
            </button>
          </div>
        </form>
      ) : null}
      <section className="record-list">
        {items.length ? (
          items.map((item, index) => {
            const result = stateToSearchResult(item);
            return (
              <MemoryRecordRow
                key={item.key}
                type="state"
                title={item.title}
                body={`${item.status} priority ${item.priority}${item.owner ? `, owned by ${item.owner}` : ""}`}
                scope={item.scope}
                meta={item.key}
                index={index}
                reduceMotion={props.reduceMotion}
                onSelect={() => props.onSelectResult(result)}
                onOpen={() => props.onOpenResult(result)}
                onEdit={() => beginEdit(item)}
                onDelete={() => remove(item)}
              />
            );
          })
        ) : (
          <EmptyRecordList text="No state items recorded." />
        )}
      </section>
    </div>
  );
}

function ContextPane(props: {
  snapshot: ProjectSnapshot | null;
  startDir: string;
  reduceMotion: boolean;
  onSelectResult: (result: SearchResult) => void;
  onOpenResult: (result: SearchResult) => void;
  onMemoryChanged: () => Promise<ProjectSnapshot>;
}) {
  const [documents, setDocuments] = useState<ContextSummary[]>([]);
  const [loading, setLoading] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [editing, setEditing] = useState<ContextSummary | null>(null);
  const [editTitle, setEditTitle] = useState("");
  const [editScope, setEditScope] = useState("");
  const [editCategory, setEditCategory] = useState("reference");
  const [editContent, setEditContent] = useState("");
  const [loadingEdit, setLoadingEdit] = useState(false);
  const [saving, setSaving] = useState(false);

  async function load() {
    setLoading(true);
    setError(null);
    try {
      setDocuments(await listProjectContext({ startDir: props.startDir }));
    } catch (listError) {
      setError(String(listError));
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    load();
  }, [props.startDir, props.snapshot]);

  async function remove(document: ContextSummary) {
    if (!window.confirm(`Delete context "${document.title}"?`)) return;
    setMessage(null);
    setError(null);
    try {
      const result = await deleteMemoryRecord({
        startDir: props.startDir,
        recordType: "context",
        id: document.key,
      });
      setDocuments((current) => current.filter((candidate) => candidate.key !== document.key));
      setMessage(result.message);
      await props.onMemoryChanged();
    } catch (deleteError) {
      setError(String(deleteError));
    }
  }

  async function beginEdit(document: ContextSummary) {
    setLoadingEdit(true);
    setEditing(document);
    setEditTitle(document.title);
    setEditScope(document.scope);
    setEditCategory(document.category);
    setEditContent("");
    setMessage(null);
    setError(null);
    try {
      const detail = await getMemoryRecord({
        startDir: props.startDir,
        recordType: "context",
        id: document.key,
        scope: document.scope,
      });
      setEditContent(detail.body);
    } catch (detailError) {
      setError(String(detailError));
    } finally {
      setLoadingEdit(false);
    }
  }

  async function saveEdit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!editing || !editTitle.trim() || !editContent.trim() || saving) return;
    setSaving(true);
    setMessage(null);
    setError(null);
    try {
      const result = await updateMemoryRecord({
        startDir: props.startDir,
        recordType: "context",
        id: editing.key,
        title: editTitle,
        scope: editScope,
        category: editCategory,
        content: editContent,
      });
      setMessage(result.message);
      setEditing(null);
      await load();
      await props.onMemoryChanged();
    } catch (editError) {
      setError(String(editError));
    } finally {
      setSaving(false);
    }
  }

  return (
    <div className="view-stack">
      <MemoryListHeader
        title="Context"
        icon={FileText}
        loading={loading}
        onRefresh={load}
      />
      {message ? <section className="notice compact good">{message}</section> : null}
      {error ? (
        <section className="notice compact">
          <AlertTriangle size={16} />
          <span>{error}</span>
        </section>
      ) : null}
      {editing ? (
        <form className="inline-edit-form" onSubmit={saveEdit}>
          <ListHeading title={`Edit ${editing.key}`} icon={Pencil} />
          <label>
            <span>Title</span>
            <input value={editTitle} onChange={(event) => setEditTitle(event.target.value)} />
          </label>
          <div className="metadata-grid">
            <label>
              <span>Category</span>
              <select value={editCategory} onChange={(event) => setEditCategory(event.target.value)}>
                {contextCategories.map((option) => (
                  <option key={option} value={option}>
                    {option}
                  </option>
                ))}
              </select>
            </label>
            <label>
              <span>Scope</span>
              <input value={editScope} onChange={(event) => setEditScope(event.target.value)} />
            </label>
          </div>
          <label>
            <span>Content</span>
            <textarea
              value={editContent}
              onChange={(event) => setEditContent(event.target.value)}
              placeholder={loadingEdit ? "Loading context" : "Trusted context content"}
            />
          </label>
          <div className="form-actions">
            <button type="button" className="button secondary" onClick={() => setEditing(null)}>
              Cancel
            </button>
            <button className="button primary" disabled={saving || loadingEdit || !editTitle.trim() || !editContent.trim()}>
              Save
            </button>
          </div>
        </form>
      ) : null}
      <section className="record-list">
        {documents.length ? (
          documents.map((document, index) => {
            const result = contextToSearchResult(document);
            return (
              <MemoryRecordRow
                key={document.key}
                type="context"
                title={document.title}
                body={`${document.category} context, version ${document.version}`}
                scope={document.scope}
                meta={document.key}
                index={index}
                reduceMotion={props.reduceMotion}
                onSelect={() => props.onSelectResult(result)}
                onOpen={() => props.onOpenResult(result)}
                onEdit={() => beginEdit(document)}
                onDelete={() => remove(document)}
              />
            );
          })
        ) : (
          <EmptyRecordList text="No context documents recorded." />
        )}
      </section>
    </div>
  );
}

function SettingsPane(props: {
  snapshot: ProjectSnapshot | null;
  projectRoot: string;
  onProjectRootChange: (path: string) => void;
  onInitializeProject: (path?: string) => Promise<void>;
}) {
  const snapshot = props.snapshot;
  const embedding = snapshot?.embedding?.runtime;
  const [draftRoot, setDraftRoot] = useState(props.projectRoot || snapshot?.start_dir || "");
  const [initializing, setInitializing] = useState(false);
  const [maintenanceBusy, setMaintenanceBusy] = useState<string | null>(null);
  const [daemonBusy, setDaemonBusy] = useState<string | null>(null);
  const [daemonStatus, setDaemonStatus] = useState<DaemonStatus | null>(null);
  const [daemonHost, setDaemonHost] = useState("127.0.0.1");
  const [daemonPort, setDaemonPort] = useState(9700);
  const [daemonToken, setDaemonToken] = useState("");
  const [captureConfig, setCaptureConfig] = useState<CaptureConfigReport | null>(null);
  const [captureConfigBusy, setCaptureConfigBusy] = useState(false);
  const [blockedPathDraft, setBlockedPathDraft] = useState("");
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const captureSourceLabels: Array<[keyof CaptureSourceConfig, string]> = [
    ["git", "Git"],
    ["transcripts", "Transcripts"],
    ["terminal", "Terminal"],
    ["files", "Files"],
    ["ide", "IDE"],
    ["system", "System"],
    ["screen", "Screen"],
    ["browser", "Browser"],
    ["audio", "Audio"],
  ];

  useEffect(() => {
    setDraftRoot(props.projectRoot || snapshot?.start_dir || "");
  }, [props.projectRoot, snapshot?.start_dir]);

  useEffect(() => {
    refreshDaemonStatus();
    refreshCaptureConfig();
  }, [props.projectRoot, snapshot?.project?.project]);

  async function refreshDaemonStatus() {
    setDaemonBusy("status");
    try {
      const next = await getDaemonStatus({ startDir: draftRoot || props.projectRoot || snapshot?.start_dir || "" });
      setDaemonStatus(next);
      if (next.host) setDaemonHost(next.host);
      if (next.port) setDaemonPort(next.port);
    } catch (daemonError) {
      setError(String(daemonError));
    } finally {
      setDaemonBusy(null);
    }
  }

  async function refreshCaptureConfig() {
    const startDir = draftRoot || props.projectRoot || snapshot?.start_dir || "";
    if (!startDir.trim()) return;
    setCaptureConfigBusy(true);
    try {
      const next = await getCaptureConfig({ startDir });
      setCaptureConfig(next);
    } catch (configError) {
      setError(String(configError));
    } finally {
      setCaptureConfigBusy(false);
    }
  }

  async function patchCaptureConfig(input: Parameters<typeof updateCaptureConfig>[0]) {
    const startDir = draftRoot || props.projectRoot || snapshot?.start_dir || "";
    if (!startDir.trim()) return;
    setCaptureConfigBusy(true);
    setMessage(null);
    setError(null);
    try {
      const next = await updateCaptureConfig({ startDir, ...input });
      setCaptureConfig(next);
      setMessage("Capture settings saved.");
    } catch (configError) {
      setError(String(configError));
    } finally {
      setCaptureConfigBusy(false);
    }
  }

  async function updateCaptureSource(source: keyof CaptureSourceConfig, enabled: boolean) {
    await patchCaptureConfig({ [source]: enabled } as Parameters<typeof updateCaptureConfig>[0]);
  }

  async function addBlockedPath() {
    const value = blockedPathDraft.trim();
    if (!value) return;
    await patchCaptureConfig({ addBlockedPaths: [value] });
    setBlockedPathDraft("");
  }

  async function removeBlockedPath(path: string) {
    await patchCaptureConfig({ removeBlockedPaths: [path] });
  }

  async function startProjectDaemon() {
    setDaemonBusy("start");
    setMessage(null);
    setError(null);
    try {
      const result = await startDaemon({
        startDir: draftRoot,
        host: daemonHost,
        port: daemonPort,
        token: daemonToken,
      });
      setMessage(`${result.message} ${result.url}`);
      await refreshDaemonStatus();
    } catch (daemonError) {
      setError(String(daemonError));
    } finally {
      setDaemonBusy(null);
    }
  }

  async function stopProjectDaemon() {
    setDaemonBusy("stop");
    setMessage(null);
    setError(null);
    try {
      const result = await stopDaemon({ startDir: draftRoot });
      setMessage(result.message);
      await refreshDaemonStatus();
    } catch (daemonError) {
      setError(String(daemonError));
    } finally {
      setDaemonBusy(null);
    }
  }

  async function initialize() {
    setInitializing(true);
    setMessage(null);
    setError(null);
    try {
      await props.onInitializeProject(draftRoot);
      setMessage("Project initialized or refreshed.");
    } catch (initError) {
      setError(String(initError));
    } finally {
      setInitializing(false);
    }
  }

  async function browseProjectFolder() {
    setMessage(null);
    setError(null);
    try {
      const selected = await pickProjectFolder(draftRoot || snapshot?.start_dir || undefined);
      if (selected) {
        setDraftRoot(selected);
        props.onProjectRootChange(selected);
      }
    } catch (browseError) {
      setError(String(browseError));
    }
  }

  async function exportJson() {
    setMaintenanceBusy("export");
    setMessage(null);
    setError(null);
    try {
      const result = await exportMemoryToFile({
        startDir: draftRoot,
        scope: snapshot?.scope ?? "",
      });
      if (result) {
        setMessage(`${result.message} ${result.output_path}`);
      }
    } catch (exportError) {
      setError(String(exportError));
    } finally {
      setMaintenanceBusy(null);
    }
  }

  async function importJson() {
    setMaintenanceBusy("import");
    setMessage(null);
    setError(null);
    try {
      const result = await importMemoryFromFile({ startDir: draftRoot });
      if (result) {
        setMessage(
          `Imported ${result.entities} entities, ${result.relations} relations, ${result.observations} observations, ${result.decisions} decisions, and ${result.state} state items from ${result.source_project}.`,
        );
        await props.onInitializeProject(draftRoot);
      }
    } catch (importError) {
      setError(String(importError));
    } finally {
      setMaintenanceBusy(null);
    }
  }

  async function runEmbeddings(rebuild: boolean) {
    setMaintenanceBusy(rebuild ? "rebuild-embeddings" : "process-embeddings");
    setMessage(null);
    setError(null);
    try {
      const result = await processProjectEmbeddings({
        startDir: draftRoot,
        scope: rebuild ? "*" : snapshot?.scope || "*",
        rebuild,
        limit: 100,
      });
      setMessage(
        `${rebuild ? "Rebuilt" : "Processed"} embeddings: ${result.processed} processed, ${result.enqueued} enqueued, ${result.pending_remaining} pending.`,
      );
      await props.onInitializeProject(draftRoot);
    } catch (embeddingError) {
      setError(String(embeddingError));
    } finally {
      setMaintenanceBusy(null);
    }
  }

  return (
    <div className="view-stack">
      <section className="settings-grid">
        <div className="settings-editor">
          <label>
            <span>Project Folder</span>
            <input
              value={draftRoot}
              onChange={(event) => setDraftRoot(event.target.value)}
              placeholder="/path/to/project"
            />
          </label>
          <div className="form-actions">
            <button className="button secondary" onClick={browseProjectFolder}>
              <FolderOpen size={15} />
              Browse
            </button>
            <button className="button secondary" onClick={() => props.onProjectRootChange(draftRoot)}>
              Load Project
            </button>
            <button className="button primary" onClick={initialize} disabled={initializing || !draftRoot.trim()}>
              Initialize
            </button>
          </div>
        </div>
      </section>

      <section className="settings-grid">
        <ListHeading title="Capture Consent" icon={ShieldQuestion} />
        <div className="settings-editor">
          <div className="capture-config-summary">
            <span>{captureConfig?.config_path ?? "No capture config loaded"}</span>
            <code>{captureConfig?.config.redaction_profile ?? "default"}</code>
          </div>
          <div className="capture-source-grid">
            {captureSourceLabels.map(([source, label]) => (
              <label className="capture-toggle" key={source}>
                <input
                  type="checkbox"
                  checked={captureConfig?.config.sources[source] ?? false}
                  disabled={captureConfigBusy || !captureConfig}
                  onChange={(event) => updateCaptureSource(source, event.currentTarget.checked)}
                />
                <span>{label}</span>
              </label>
            ))}
          </div>
          <div className="metadata-grid">
            <label>
              <span>Terminal Output</span>
              <select
                value={captureConfig?.config.terminal_output ?? "off"}
                disabled={captureConfigBusy || !captureConfig}
                onChange={(event) =>
                  patchCaptureConfig({ terminalOutput: event.currentTarget.value as "off" | "digest" | "full" })
                }
              >
                <option value="off">Off</option>
                <option value="digest">Digest</option>
                <option value="full">Full</option>
              </select>
            </label>
            <label>
              <span>Screen Policy</span>
              <select
                value={captureConfig?.config.screen_policy ?? "manual"}
                disabled={captureConfigBusy || !captureConfig}
                onChange={(event) =>
                  patchCaptureConfig({ screenPolicy: event.currentTarget.value as "off" | "manual" | "allowlist" })
                }
              >
                <option value="off">Off</option>
                <option value="manual">Manual</option>
                <option value="allowlist">Allowlist</option>
              </select>
            </label>
          </div>
          <label>
            <span>Blocked Path</span>
            <input
              value={blockedPathDraft}
              onChange={(event) => setBlockedPathDraft(event.target.value)}
              placeholder="secrets or .env.local"
            />
          </label>
          <div className="maintenance-actions">
            <button className="button secondary" type="button" onClick={refreshCaptureConfig} disabled={captureConfigBusy || !draftRoot.trim()}>
              <RefreshCcw size={15} />
              Refresh
            </button>
            <button className="button primary" type="button" onClick={addBlockedPath} disabled={captureConfigBusy || !blockedPathDraft.trim()}>
              <Plus size={15} />
              Block Path
            </button>
          </div>
          <div className="capture-blocked-list">
            {(captureConfig?.config.blocked_paths ?? []).slice(0, 12).map((path) => (
              <button
                className="evidence-chip"
                type="button"
                key={path}
                onClick={() => removeBlockedPath(path)}
                disabled={captureConfigBusy}
                title="Remove blocked path"
              >
                {path}
              </button>
            ))}
          </div>
        </div>
      </section>

      <section className="settings-grid">
        <ListHeading title="Local Daemon" icon={Activity} />
        <div className="settings-editor">
          <div className={`daemon-status ${daemonStatus?.running ? "running" : ""}`}>
            <span>{daemonStatus?.running ? "Running" : "Stopped"}</span>
            <strong>{daemonStatus?.url ?? "http://127.0.0.1:9700"}</strong>
            <code>{daemonStatus?.cli_path ?? "CLI not found"}</code>
          </div>
          <div className="metadata-grid">
            <label>
              <span>Host</span>
              <input value={daemonHost} onChange={(event) => setDaemonHost(event.target.value)} />
            </label>
            <label>
              <span>Port</span>
              <input
                type="number"
                min={1024}
                max={65535}
                value={daemonPort}
                onChange={(event) => setDaemonPort(Number(event.target.value) || 9700)}
              />
            </label>
          </div>
          <label>
            <span>Token</span>
            <input
              value={daemonToken}
              onChange={(event) => setDaemonToken(event.target.value)}
              placeholder="optional for local daemon"
            />
          </label>
          <div className="maintenance-actions">
            <button
              className="button secondary"
              onClick={refreshDaemonStatus}
              disabled={daemonBusy !== null || !draftRoot.trim()}
            >
              <RefreshCcw size={15} />
              Refresh
            </button>
            <button
              className="button primary"
              onClick={startProjectDaemon}
              disabled={daemonBusy !== null || !draftRoot.trim() || !daemonStatus?.cli_available}
            >
              <Activity size={15} />
              Start
            </button>
            <button
              className="button secondary danger-button"
              onClick={stopProjectDaemon}
              disabled={daemonBusy !== null || !draftRoot.trim() || !daemonStatus?.cli_available}
            >
              <X size={15} />
              Stop
            </button>
          </div>
        </div>
      </section>

      <section className="settings-grid">
        <ListHeading title="Memory Maintenance" icon={Database} />
        <div className="settings-editor">
          <div className="maintenance-actions">
            <button
              className="button secondary"
              onClick={exportJson}
              disabled={maintenanceBusy !== null || !draftRoot.trim()}
            >
              <Download size={15} />
              Export JSON
            </button>
            <button
              className="button secondary"
              onClick={importJson}
              disabled={maintenanceBusy !== null || !draftRoot.trim()}
            >
              <Upload size={15} />
              Import JSON
            </button>
            <button
              className="button secondary"
              onClick={() => runEmbeddings(false)}
              disabled={maintenanceBusy !== null || !draftRoot.trim()}
            >
              <Sparkles size={15} />
              Process Embeddings
            </button>
            <button
              className="button primary"
              onClick={() => runEmbeddings(true)}
              disabled={maintenanceBusy !== null || !draftRoot.trim()}
            >
              <RefreshCcw size={15} />
              Rebuild Embeddings
            </button>
          </div>
        </div>
      </section>

      {message ? <section className="notice compact good">{message}</section> : null}
      {error ? (
        <section className="notice compact">
          <AlertTriangle size={16} />
          <span>{error}</span>
        </section>
      ) : null}

      <section className="settings-grid">
        <Setting label="Project" value={snapshot?.project?.project ?? "Not initialized"} />
        <Setting label="Database" value={snapshot?.project?.db_path ?? "Unavailable"} mono />
        <Setting label="Embedding provider" value={embedding?.provider ?? "Unknown"} />
        <Setting label="Vector backend" value={embedding?.vector_backend ?? "Unknown"} />
        <Setting label="Indexed records" value={`${embedding?.indexed_records ?? 0}`} />
        <Setting label="Missing or stale" value={`${embedding?.missing_or_stale_records ?? 0}`} />
      </section>
    </div>
  );
}

function DetailPane(props: {
  pane: PaneState;
  selectedResult: SearchResult | null;
  snapshot: ProjectSnapshot | null;
  detail: MemoryRecordDetail | null;
  loading: boolean;
  error: string | null;
  startDir: string;
  onMemoryChanged: () => Promise<ProjectSnapshot>;
}) {
  const result = props.selectedResult;
  const detail = props.detail;
  const title = detail?.title ?? result?.title ?? props.pane.recordId ?? "Selected memory";
  const recordType = detail?.record_type ?? result?.record_type ?? props.pane.recordType ?? "record";
  const body = detail?.body ?? result?.snippet ?? "Select a memory item from search or graph to inspect it here.";
  const editableTypes = ["decision", "entity", "observation", "context", "state", "relation", "session"];
  const canEdit = Boolean(detail && editableTypes.includes(recordType));
  const canDelete = canEdit && recordType !== "session";
  const [editing, setEditing] = useState(false);
  const [editTitle, setEditTitle] = useState("");
  const [editScope, setEditScope] = useState("");
  const [editBody, setEditBody] = useState("");
  const [editSessionType, setEditSessionType] = useState("codex");
  const [editAccomplishments, setEditAccomplishments] = useState("");
  const [editRemaining, setEditRemaining] = useState("");
  const [editFilesChanged, setEditFilesChanged] = useState("");
  const [editStatus, setEditStatus] = useState("active");
  const [editCategory, setEditCategory] = useState("general");
  const [editEntityType, setEditEntityType] = useState("concept");
  const [editPriority, setEditPriority] = useState("medium");
  const [editRelation, setEditRelation] = useState("works_with");
  const [editWeight, setEditWeight] = useState("1");
  const [editConfidence, setEditConfidence] = useState("1");
  const [editSourceType, setEditSourceType] = useState("EXTRACTED");
  const [editSource, setEditSource] = useState("");
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);
  const [deleted, setDeleted] = useState(false);

  useEffect(() => {
    setEditing(false);
    setMessage(null);
    setActionError(null);
    setDeleted(false);
  }, [detail?.id, detail?.record_type]);

  function metadataValue(label: string, fallback = "") {
    return detail?.metadata.find((item) => item.label === label)?.value ?? fallback;
  }

  function beginEdit() {
    if (!detail) return;
    setEditTitle(detail.title);
    setEditScope(detail.scope);
    setEditBody(recordType === "session" && detail.body === "No session summary recorded yet." ? "" : detail.body);
    setEditSessionType(metadataValue("type", "codex"));
    setEditAccomplishments(metadataValue("accomplishments"));
    setEditRemaining(metadataValue("remaining"));
    setEditFilesChanged(metadataValue("files changed"));
    setEditStatus(metadataValue("status", recordType === "state" ? "in-progress" : "active"));
    setEditCategory(metadataValue("category", recordType === "context" ? "reference" : "general"));
    setEditEntityType(metadataValue("entity type", "concept"));
    setEditPriority(metadataValue("priority", "medium"));
    setEditRelation(metadataValue("relation", "works_with"));
    setEditWeight(metadataValue("weight", "1"));
    setEditConfidence(metadataValue("confidence", "1"));
    setEditSourceType(metadataValue("source type", "EXTRACTED"));
    setEditSource(metadataValue("source", ""));
    setMessage(null);
    setActionError(null);
    setEditing(true);
  }

  async function saveEdit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!detail || busy) return;
    const isRelation = recordType === "relation";
    const needsTitle = recordType !== "observation" && !isRelation;
    const needsBody = recordType === "decision" || recordType === "observation" || recordType === "context";
    if ((needsTitle && !editTitle.trim()) || (needsBody && !editBody.trim())) return;
    const weight = Number(editWeight);
    const confidence = Number(editConfidence);
    if (isRelation && (Number.isNaN(weight) || Number.isNaN(confidence))) return;

    setBusy(true);
    setMessage(null);
    setActionError(null);
    try {
      const result = await updateMemoryRecord({
        startDir: props.startDir,
        recordType: recordType as "context" | "state" | "decision" | "entity" | "observation" | "relation" | "session",
        id: detail.id,
        title: editTitle,
        scope: editScope,
        content: editBody,
        goal: recordType === "session" ? editTitle : undefined,
        summary: recordType === "session" ? editBody : undefined,
        sessionType: recordType === "session" ? editSessionType : undefined,
        accomplishments: recordType === "session" ? editAccomplishments : undefined,
        remaining: recordType === "session" ? editRemaining : undefined,
        filesChanged: recordType === "session" ? editFilesChanged : undefined,
        category: editCategory,
        entityType: editEntityType,
        status: editStatus,
        priority: editPriority,
        relation: editRelation,
        weight: isRelation ? weight : undefined,
        confidence: isRelation ? confidence : undefined,
        sourceType: editSourceType,
        source: editSource,
      });
      setMessage(result.message);
      setEditing(false);
      await props.onMemoryChanged();
    } catch (editError) {
      setActionError(String(editError));
    } finally {
      setBusy(false);
    }
  }

  async function removeRecord() {
    if (!detail || busy) return;
    if (!window.confirm(`Delete ${recordType} "${title}"?`)) return;
    setBusy(true);
    setMessage(null);
    setActionError(null);
    try {
      const result = await deleteMemoryRecord({
        startDir: props.startDir,
        recordType,
        id: detail.id,
      });
      setMessage(result.message);
      setDeleted(true);
      await props.onMemoryChanged();
    } catch (deleteError) {
      setActionError(String(deleteError));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="view-stack">
      <section className="detail-block">
        <span className="record-type">{recordType}</span>
        <h3>{title}</h3>
        <p>{body}</p>
        {canEdit && !deleted ? (
          <div className="detail-actions">
            <button className="button secondary" onClick={beginEdit} disabled={busy}>
              <Pencil size={15} />
              Edit
            </button>
            {canDelete ? (
              <button className="button secondary danger-button" onClick={removeRecord} disabled={busy}>
                <Trash2 size={15} />
                Delete
              </button>
            ) : null}
          </div>
        ) : null}
      </section>

      {message ? <section className="notice compact good">{message}</section> : null}
      {actionError ? (
        <section className="notice compact">
          <AlertTriangle size={16} />
          <span>{actionError}</span>
        </section>
      ) : null}

      {editing && detail ? (
        <form className="inline-edit-form" onSubmit={saveEdit}>
          <ListHeading title={`Edit ${recordType}`} icon={Pencil} />
          {recordType !== "observation" && recordType !== "relation" ? (
            <label>
              <span>{recordType === "session" ? "Goal" : "Title"}</span>
              <input value={editTitle} onChange={(event) => setEditTitle(event.target.value)} />
            </label>
          ) : null}
          <div className="metadata-grid">
            {recordType === "session" ? (
              <>
                <label>
                  <span>Type</span>
                  <select value={editSessionType} onChange={(event) => setEditSessionType(event.target.value)}>
                    {sessionTypes.map((option) => (
                      <option key={option} value={option}>
                        {option}
                      </option>
                    ))}
                  </select>
                </label>
                <label>
                  <span>Status</span>
                  <select value={editStatus} onChange={(event) => setEditStatus(event.target.value)}>
                    {sessionStatuses.map((option) => (
                      <option key={option} value={option}>
                        {option}
                      </option>
                    ))}
                  </select>
                </label>
              </>
            ) : null}
            {recordType === "decision" ? (
              <label>
                <span>Status</span>
                <select value={editStatus} onChange={(event) => setEditStatus(event.target.value)}>
                  {decisionStatuses.map((option) => (
                    <option key={option} value={option}>
                      {option}
                    </option>
                  ))}
                </select>
              </label>
            ) : null}
            {recordType === "state" ? (
              <>
                <label>
                  <span>Status</span>
                  <select value={editStatus} onChange={(event) => setEditStatus(event.target.value)}>
                    {stateStatuses.map((option) => (
                      <option key={option} value={option}>
                        {option}
                      </option>
                    ))}
                  </select>
                </label>
                <label>
                  <span>Priority</span>
                  <select value={editPriority} onChange={(event) => setEditPriority(event.target.value)}>
                    {statePriorities.map((option) => (
                      <option key={option} value={option}>
                        {option}
                      </option>
                    ))}
                  </select>
                </label>
              </>
            ) : null}
            {recordType === "entity" ? (
              <label>
                <span>Entity Type</span>
                <select value={editEntityType} onChange={(event) => setEditEntityType(event.target.value)}>
                  {entityTypeOptions.map((option) => (
                    <option key={option} value={option}>
                      {option}
                    </option>
                  ))}
                </select>
              </label>
            ) : null}
            {recordType === "context" || recordType === "observation" ? (
              <label>
                <span>Category</span>
                <select value={editCategory} onChange={(event) => setEditCategory(event.target.value)}>
                  {(recordType === "context" ? contextCategories : observationCategories).map((option) => (
                    <option key={option} value={option}>
                      {option}
                    </option>
                  ))}
                </select>
              </label>
            ) : null}
            {recordType === "relation" ? (
              <>
                <label>
                  <span>Relation</span>
                  <select value={editRelation} onChange={(event) => setEditRelation(event.target.value)}>
                    {relationTypes.map((option) => (
                      <option key={option} value={option}>
                        {option}
                      </option>
                    ))}
                  </select>
                </label>
                <label>
                  <span>Weight</span>
                  <input
                    type="number"
                    step="0.1"
                    value={editWeight}
                    onChange={(event) => setEditWeight(event.target.value)}
                  />
                </label>
                <label>
                  <span>Confidence</span>
                  <input
                    type="number"
                    min="0"
                    max="1"
                    step="0.05"
                    value={editConfidence}
                    onChange={(event) => setEditConfidence(event.target.value)}
                  />
                </label>
                <label>
                  <span>Source Type</span>
                  <select value={editSourceType} onChange={(event) => setEditSourceType(event.target.value)}>
                    {relationSourceTypes.map((option) => (
                      <option key={option} value={option}>
                        {option}
                      </option>
                    ))}
                  </select>
                </label>
                <label>
                  <span>Source</span>
                  <input value={editSource} onChange={(event) => setEditSource(event.target.value)} />
                </label>
              </>
            ) : null}
            {recordType !== "observation" && recordType !== "relation" ? (
              <label>
                <span>Scope</span>
                <input value={editScope} onChange={(event) => setEditScope(event.target.value)} />
              </label>
            ) : null}
          </div>
          {recordType !== "entity" && recordType !== "relation" ? (
            <label>
              <span>{recordType === "state" ? "Details" : recordType === "session" ? "Summary" : "Content"}</span>
              <textarea value={editBody} onChange={(event) => setEditBody(event.target.value)} />
            </label>
          ) : null}
          {recordType === "session" ? (
            <div className="metadata-grid">
              <label>
                <span>Accomplishments</span>
                <input
                  value={editAccomplishments}
                  onChange={(event) => setEditAccomplishments(event.target.value)}
                  placeholder="comma separated"
                />
              </label>
              <label>
                <span>Remaining</span>
                <input
                  value={editRemaining}
                  onChange={(event) => setEditRemaining(event.target.value)}
                  placeholder="comma separated"
                />
              </label>
              <label>
                <span>Files Changed</span>
                <input
                  value={editFilesChanged}
                  onChange={(event) => setEditFilesChanged(event.target.value)}
                  placeholder="comma separated paths"
                />
              </label>
            </div>
          ) : null}
          <div className="form-actions">
            <button type="button" className="button secondary" onClick={() => setEditing(false)} disabled={busy}>
              Cancel
            </button>
            <button className="button primary" disabled={busy}>
              Save
            </button>
          </div>
        </form>
      ) : null}

      {props.loading ? (
        <section className="notice compact">
          <Sparkles size={16} />
          <span>Loading full memory record.</span>
        </section>
      ) : null}

      {props.error ? (
        <section className="notice compact">
          <AlertTriangle size={16} />
          <span>{props.error}</span>
        </section>
      ) : null}

      <section className="dense-list">
        <ListHeading title="Provenance" icon={PanelRight} />
        <Row title={detail?.scope || result?.scope || props.snapshot?.scope || "global"} meta="scope" />
        <Row title={detail?.id || result?.id || props.pane.recordId || "pending"} meta="record id" />
        <Row title={typeof result?.score === "number" ? result.score.toFixed(3) : "not scored"} meta="score" />
      </section>

      {detail?.metadata.length ? (
        <section className="dense-list">
          <ListHeading title="Metadata" icon={Database} />
          {detail.metadata.map((item) => (
            <Row key={`${item.label}-${item.value}`} title={item.value} meta={item.label} />
          ))}
        </section>
      ) : null}

      {detail?.related.length ? (
        <section className="dense-list">
          <ListHeading title="Related" icon={Network} />
          {detail.related.map((item) => (
            <Row key={`${item.record_type}-${item.id}-${item.relation}`} title={item.title} meta={item.relation} />
          ))}
        </section>
      ) : null}

      {detail?.events.length ? (
        <section className="dense-list">
          <ListHeading title="Recent Events" icon={History} />
          {detail.events.map((event) => (
            <Row key={event.id} title={event.summary} meta={event.event_type} />
          ))}
        </section>
      ) : null}
    </div>
  );
}

function CapturePane(props: {
  pane: PaneState;
  startDir: string;
  reduceMotion: boolean;
  onCaptured: (result: CaptureMemoryResult) => void;
}) {
  const type = props.pane.captureType ?? "observation";
  const copy = captureCopy[type];
  const [title, setTitle] = useState("");
  const [scope, setScope] = useState("");
  const [content, setContent] = useState("");
  const [key, setKey] = useState("");
  const [entityType, setEntityType] = useState("concept");
  const [category, setCategory] = useState(type === "context" ? "reference" : "general");
  const [status, setStatus] = useState("in-progress");
  const [priority, setPriority] = useState("medium");
  const [relationTarget, setRelationTarget] = useState("");
  const [relationType, setRelationType] = useState("works_with");
  const [tags, setTags] = useState("");
  const [alternatives, setAlternatives] = useState("");
  const [supersedes, setSupersedes] = useState("");
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState<CaptureMemoryResult | null>(null);
  const [error, setError] = useState<string | null>(null);

  const requiresContent = type === "observation" || type === "context";
  const canSubmit =
    title.trim().length > 0 &&
    (!requiresContent || content.trim().length > 0) &&
    (type !== "relation" || relationTarget.trim().length > 0);

  async function submit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!canSubmit || saving) return;

    setSaving(true);
    setError(null);
    setSaved(null);
    try {
      const result = await captureMemory({
        startDir: props.startDir,
        captureType: type,
        title,
        scope,
        content,
        key,
        entityType,
        category,
        status,
        priority,
        relationTarget,
        relationType,
        tags,
        alternatives,
        supersedes,
      });
      setSaved(result);
      props.onCaptured(result);
    } catch (captureError) {
      setError(String(captureError));
    } finally {
      setSaving(false);
    }
  }

  function clearForm() {
    setTitle("");
    setScope("");
    setContent("");
    setKey("");
    setRelationTarget("");
    setTags("");
    setAlternatives("");
    setSupersedes("");
    setSaved(null);
    setError(null);
  }

  return (
    <motion.form
      className="capture-form"
      onSubmit={submit}
      initial={props.reduceMotion ? false : { opacity: 0, y: 8 }}
      animate={{ opacity: 1, y: 0 }}
      transition={transition.quick}
    >
      <label>
        <span>{copy.title}</span>
        <input value={title} onChange={(event) => setTitle(event.target.value)} placeholder={copy.titlePlaceholder} />
      </label>

      {type !== "handoff" ? (
        <label>
          <span>Scope</span>
          <input value={scope} onChange={(event) => setScope(event.target.value)} placeholder="project/module" />
        </label>
      ) : null}

      {type === "state" || type === "context" ? (
        <label>
          <span>Key</span>
          <input value={key} onChange={(event) => setKey(event.target.value)} placeholder="auto from title" />
        </label>
      ) : null}

      {type === "relation" ? (
        <label>
          <span>Target Entity ID</span>
          <input
            value={relationTarget}
            onChange={(event) => setRelationTarget(event.target.value)}
            placeholder="existing-entity-id"
          />
        </label>
      ) : null}

      {type !== "handoff" ? (
        <label>
          <span>{copy.body}</span>
          <textarea value={content} onChange={(event) => setContent(event.target.value)} placeholder={copy.bodyPlaceholder} />
        </label>
      ) : null}

      {type === "decision" ? (
        <div className="metadata-grid">
          <label>
            <span>Tags</span>
            <input value={tags} onChange={(event) => setTags(event.target.value)} placeholder="desktop, retrieval" />
          </label>
          <label>
            <span>Alternatives</span>
            <input
              value={alternatives}
              onChange={(event) => setAlternatives(event.target.value)}
              placeholder="comma separated"
            />
          </label>
          <label>
            <span>Supersedes</span>
            <input value={supersedes} onChange={(event) => setSupersedes(event.target.value)} placeholder="decision id" />
          </label>
        </div>
      ) : null}

      {type === "observation" || type === "relation" ? (
        <div className="metadata-grid">
          <label>
            <span>Entity Type</span>
            <select value={entityType} onChange={(event) => setEntityType(event.target.value)}>
              {entityTypeOptions.map((option) => (
                <option key={option} value={option}>
                  {option}
                </option>
              ))}
            </select>
          </label>
          <label>
            <span>Category</span>
            <select value={category} onChange={(event) => setCategory(event.target.value)}>
              {observationCategories.map((option) => (
                <option key={option} value={option}>
                  {option}
                </option>
              ))}
            </select>
          </label>
          {type === "relation" ? (
            <label>
              <span>Relation</span>
              <select value={relationType} onChange={(event) => setRelationType(event.target.value)}>
                {relationTypes.map((option) => (
                  <option key={option} value={option}>
                    {option}
                  </option>
                ))}
              </select>
            </label>
          ) : null}
        </div>
      ) : null}

      {type === "state" ? (
        <div className="metadata-grid">
          <label>
            <span>Status</span>
            <select value={status} onChange={(event) => setStatus(event.target.value)}>
              {stateStatuses.map((option) => (
                <option key={option} value={option}>
                  {option}
                </option>
              ))}
            </select>
          </label>
          <label>
            <span>Priority</span>
            <select value={priority} onChange={(event) => setPriority(event.target.value)}>
              {statePriorities.map((option) => (
                <option key={option} value={option}>
                  {option}
                </option>
              ))}
            </select>
          </label>
        </div>
      ) : null}

      {type === "context" ? (
        <div className="metadata-grid">
          <label>
            <span>Category</span>
            <select value={category} onChange={(event) => setCategory(event.target.value)}>
              {contextCategories.map((option) => (
                <option key={option} value={option}>
                  {option}
                </option>
              ))}
            </select>
          </label>
        </div>
      ) : null}

      {saved ? <section className="notice compact good">{saved.message}</section> : null}
      {error ? <section className="notice compact">{error}</section> : null}

      <div className="form-actions">
        <motion.button type="button" className="button secondary" onClick={clearForm} {...pressMotion(props.reduceMotion)}>
          Clear
        </motion.button>
        <motion.button className="button primary" disabled={!canSubmit || saving} {...pressMotion(props.reduceMotion)}>
          {saving ? "Capturing" : "Capture"}
        </motion.button>
      </div>
    </motion.form>
  );
}

function Inspector(props: {
  snapshot: ProjectSnapshot | null;
  activePane?: PaneState;
  selectedResult: SearchResult | null;
  recordDetail: MemoryRecordDetail | null;
  recordDetailLoading: boolean;
  recordDetailError: string | null;
  onOpenDetail: () => void;
  onOpenGraph: (entityId: string) => void;
  onClose: () => void;
  reduceMotion: boolean;
}) {
  const embedding = props.snapshot?.embedding?.runtime;
  const detail = props.recordDetail;
  const selectedTitle = detail?.title ?? props.selectedResult?.title;
  const selectedType = detail?.record_type ?? props.selectedResult?.record_type;
  const selectedBody = detail?.body ?? props.selectedResult?.snippet;
  const selectedId = detail?.id ?? props.selectedResult?.id;

  return (
    <motion.aside
      className="inspector"
      initial={props.reduceMotion ? false : { opacity: 0, x: 18 }}
      animate={{ opacity: 1, x: 0 }}
      exit={props.reduceMotion ? undefined : { opacity: 0, x: 18 }}
      transition={transition.quick}
    >
      <header>
        <span>Inspector</span>
        <button className="icon-button" type="button" title="Hide inspector" onClick={props.onClose}>
          <PanelRight size={17} />
        </button>
      </header>

      {props.selectedResult || detail ? (
        <section className="inspector-section selected">
          <span className="record-type">{selectedType}</span>
          <h3>{selectedTitle}</h3>
          <p>{selectedBody}</p>
          {props.recordDetailLoading ? <p>Loading full record...</p> : null}
          {props.recordDetailError ? <p>{props.recordDetailError}</p> : null}
          <code>{selectedId}</code>
          <div className="inspector-actions">
            <button onClick={props.onOpenDetail}>Detail</button>
            {detail?.focus_entity_id ? (
              <button onClick={() => props.onOpenGraph(detail.focus_entity_id ?? "")}>Graph</button>
            ) : null}
            {selectedId ? <button onClick={() => navigator.clipboard?.writeText(selectedId)}>Copy ID</button> : null}
          </div>
        </section>
      ) : (
        <section className="inspector-section">
          <h3>{props.activePane?.title ?? "No pane"}</h3>
          <p>{props.activePane?.kind ?? "inactive"}</p>
        </section>
      )}

      <section className="inspector-section">
        <ListHeading title="Freshness" icon={Sparkles} />
        <Row title={`${embedding?.fresh_records ?? 0} fresh`} meta="records" />
        <Row title={`${embedding?.missing_or_stale_records ?? 0} stale`} meta="attention" />
        <Row title={embedding?.model ?? "not available"} meta="model" />
      </section>

      <section className="inspector-section">
        <ListHeading title="Project" icon={Database} />
        <Row title={props.snapshot?.project?.project ?? "Not initialized"} meta="name" />
        <Row title={props.snapshot?.start_dir ?? "Unknown"} meta="start dir" />
      </section>

      {detail?.related.length ? (
        <section className="inspector-section">
          <ListHeading title="Related" icon={Network} />
          {detail.related.slice(0, 4).map((item) => (
            <Row key={`${item.record_type}-${item.id}-${item.relation}`} title={item.title} meta={item.relation} />
          ))}
        </section>
      ) : null}
    </motion.aside>
  );
}

function CommandPalette(props: {
  snapshot: ProjectSnapshot | null;
  reduceMotion: boolean;
  onClose: () => void;
  onOpenPane: (kind: PaneKind) => void;
  onNewSearch: (query: string) => void;
  onCapture: (captureType: CaptureType) => void;
  onSplit: () => void;
}) {
  const [query, setQuery] = useState("");
  const commands = [
    ...navItems.map((item) => ({
      id: `open-${item.kind}`,
      title: `Open ${item.label}`,
      meta: "navigation",
      icon: item.icon,
      run: () => props.onOpenPane(item.kind),
    })),
    {
      id: "split-pane",
      title: "Duplicate Active Pane",
      meta: "layout",
      icon: SplitSquareHorizontal,
      run: props.onSplit,
    },
    ...captureItems.map((item) => ({
      id: `capture-${item.captureType}`,
      title: `Capture ${item.label}`,
      meta: "memory write",
      icon: item.icon,
      run: () => props.onCapture(item.captureType),
    })),
  ];
  const filtered = commands.filter((command) =>
    `${command.title} ${command.meta}`.toLowerCase().includes(query.toLowerCase()),
  );

  function submitSearch() {
    const clean = query.trim();
    if (clean) {
      props.onNewSearch(clean);
      props.onClose();
    }
  }

  return (
    <motion.div
      className="overlay"
      role="presentation"
      onMouseDown={props.onClose}
      initial={props.reduceMotion ? false : { opacity: 0 }}
      animate={{ opacity: 1 }}
      exit={props.reduceMotion ? undefined : { opacity: 0 }}
      transition={transition.quick}
    >
      <motion.section
        className="command-palette"
        onMouseDown={(event) => event.stopPropagation()}
        initial={props.reduceMotion ? false : { opacity: 0, y: 12, scale: 0.985 }}
        animate={{ opacity: 1, y: 0, scale: 1 }}
        exit={props.reduceMotion ? undefined : { opacity: 0, y: 8, scale: 0.985 }}
        transition={transition.modal}
      >
        <div className="command-input">
          <CommandIcon size={18} />
          <input
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter") submitSearch();
              if (event.key === "Escape") props.onClose();
            }}
            placeholder={`Search commands or ${props.snapshot?.project?.project ?? "memory"}`}
            autoFocus
          />
        </div>
        <div className="command-results">
          {filtered.slice(0, 9).map((command) => {
            const Icon = command.icon;
            return (
              <motion.button
                key={command.id}
                onClick={() => {
                  command.run();
                  props.onClose();
                }}
                initial={props.reduceMotion ? false : { opacity: 0, y: 6 }}
                animate={{ opacity: 1, y: 0 }}
                transition={transition.quick}
                whileHover={props.reduceMotion ? undefined : { x: 3 }}
                whileTap={props.reduceMotion ? undefined : { scale: 0.99 }}
              >
                <Icon size={17} />
                <span>{command.title}</span>
                <code>{command.meta}</code>
              </motion.button>
            );
          })}
        </div>
      </motion.section>
    </motion.div>
  );
}

function Launcher(props: {
  reduceMotion: boolean;
  onClose: () => void;
  onCapture: (captureType: CaptureType) => void;
}) {
  return (
    <motion.div
      className="overlay"
      role="presentation"
      onMouseDown={props.onClose}
      initial={props.reduceMotion ? false : { opacity: 0 }}
      animate={{ opacity: 1 }}
      exit={props.reduceMotion ? undefined : { opacity: 0 }}
      transition={transition.quick}
    >
      <motion.section
        className="launcher"
        onMouseDown={(event) => event.stopPropagation()}
        initial={props.reduceMotion ? false : { opacity: 0, y: 12, scale: 0.985 }}
        animate={{ opacity: 1, y: 0, scale: 1 }}
        exit={props.reduceMotion ? undefined : { opacity: 0, y: 8, scale: 0.985 }}
        transition={transition.modal}
      >
        <header>
          <strong>Capture Memory</strong>
          <motion.button onClick={props.onClose} {...pressMotion(props.reduceMotion)}>
            <X size={16} />
          </motion.button>
        </header>
        <div className="launcher-grid">
          {captureItems.map((item) => {
            const Icon = item.icon;
            return (
              <motion.button
                key={item.captureType}
                onClick={() => props.onCapture(item.captureType)}
                {...pressMotion(props.reduceMotion)}
              >
                <Icon size={22} />
                <span>{item.label}</span>
              </motion.button>
            );
          })}
        </div>
      </motion.section>
    </motion.div>
  );
}

function Metric({ label, value }: { label: string; value: number }) {
  return (
    <motion.div className="metric" layout whileHover={{ y: -1 }} transition={transition.quick}>
      <span>{label}</span>
      <strong>{value}</strong>
    </motion.div>
  );
}

function EntityList(props: {
  title: string;
  icon: typeof History;
  rows: string[];
  meta: string;
}) {
  return (
    <section className="dense-list">
      <ListHeading title={props.title} icon={props.icon} />
      {props.rows.map((row) => (
        <Row key={row} title={row} meta={props.meta} />
      ))}
    </section>
  );
}

function MemoryListHeader(props: {
  title: string;
  icon: typeof Activity;
  loading: boolean;
  onRefresh: () => void;
}) {
  const Icon = props.icon;
  return (
    <section className="memory-list-header">
      <div>
        <Icon size={16} />
        <strong>{props.title}</strong>
      </div>
      <button className="icon-button" onClick={props.onRefresh} title="Refresh records">
        <RefreshCcw size={15} className={props.loading ? "spin" : ""} />
      </button>
    </section>
  );
}

function MemoryRecordRow(props: {
  type: string;
  title: string;
  body: string;
  scope: string;
  meta: string;
  index: number;
  reduceMotion: boolean;
  onSelect: () => void;
  onOpen: () => void;
  onEdit?: () => void;
  onHandoff?: () => void;
  onComplete?: () => void;
  onDelete?: () => void;
  disabled?: boolean;
}) {
  return (
    <motion.article
      className={`record-list-row ${props.disabled ? "busy" : ""}`}
      onClick={() => {
        if (!props.disabled) props.onSelect();
      }}
      onDoubleClick={() => {
        if (!props.disabled) props.onOpen();
      }}
      initial={props.reduceMotion ? false : { opacity: 0, y: 8 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ ...transition.quick, delay: props.reduceMotion ? 0 : props.index * 0.025 }}
      whileHover={props.reduceMotion ? undefined : { x: 2 }}
    >
      <div>
        <span className="record-type">{props.type}</span>
        <strong>{props.title}</strong>
        <p>{props.body}</p>
        <footer>
          <code>{props.scope || "global"}</code>
          <code>{props.meta}</code>
        </footer>
      </div>
      <div className="record-row-actions">
        {props.onHandoff ? (
          <button
            className="icon-button"
            title={`Handoff ${props.type}`}
            disabled={props.disabled}
            onClick={(event) => {
              event.stopPropagation();
              props.onHandoff?.();
            }}
          >
            <FileClock size={15} />
          </button>
        ) : null}
        {props.onComplete ? (
          <button
            className="icon-button success"
            title={`Complete ${props.type}`}
            disabled={props.disabled}
            onClick={(event) => {
              event.stopPropagation();
              props.onComplete?.();
            }}
          >
            <CheckCircle2 size={15} />
          </button>
        ) : null}
        {props.onEdit ? (
          <button
            className="icon-button"
            title={`Edit ${props.type}`}
            disabled={props.disabled}
            onClick={(event) => {
              event.stopPropagation();
              props.onEdit?.();
            }}
          >
            <Pencil size={15} />
          </button>
        ) : null}
        {props.onDelete ? (
          <button
            className="icon-button danger"
            title={`Delete ${props.type}`}
            disabled={props.disabled}
            onClick={(event) => {
              event.stopPropagation();
              props.onDelete?.();
            }}
          >
            <Trash2 size={15} />
          </button>
        ) : null}
      </div>
    </motion.article>
  );
}

function EmptyRecordList({ text }: { text: string }) {
  return (
    <div className="empty-record-list">
      <CircleDot size={13} />
      <span>{text}</span>
    </div>
  );
}

interface CandidateGroup {
  key: string;
  title: string;
  meta: string;
  candidates: ExtractionCandidate[];
}

function groupCandidates(candidates: ExtractionCandidate[]): CandidateGroup[] {
  const groups = new Map<string, CandidateGroup>();
  for (const candidate of candidates) {
    const day = candidate.created_at?.slice(0, 10) || "unknown";
    const source = candidateSourceLabel(candidate);
    const key = `${day}:${source}`;
    const group = groups.get(key) ?? {
      key,
      title: source,
      meta: "",
      candidates: [],
    };
    group.candidates.push(candidate);
    groups.set(key, group);
  }

  return Array.from(groups.values()).map((group) => {
    const pending = group.candidates.filter((candidate) => candidate.status === "pending").length;
    const noisy = group.candidates.filter(candidateIsNoisy).length;
    const first = group.candidates[0];
    const dayLabel = first ? candidateCreatedDateLabel(first) : "unknown";
    const meta = [
      dayLabel,
      `${group.candidates.length} item${group.candidates.length === 1 ? "" : "s"}`,
      `${pending} pending`,
      noisy ? `${noisy} noisy` : null,
    ]
      .filter(Boolean)
      .join(" / ");
    return { ...group, meta };
  });
}

function candidateSourceLabel(candidate: ExtractionCandidate): string {
  const evidence = candidate.evidence?.[0];
  const source = candidate.source || evidence?.source || evidence?.title || evidence?.source_type;
  if (source) return source.length > 64 ? `${source.slice(0, 61)}...` : source;
  return candidate.source_type || "unknown source";
}

function candidateCreatedDateLabel(candidate: ExtractionCandidate): string {
  const date = new Date(candidate.created_at);
  if (Number.isNaN(date.getTime())) return "unknown date";
  const today = new Date();
  const yesterday = new Date();
  yesterday.setDate(today.getDate() - 1);
  if (date.toDateString() === today.toDateString()) return `Today ${date.toLocaleTimeString([], { hour: "numeric", minute: "2-digit" })}`;
  if (date.toDateString() === yesterday.toDateString()) return `Yesterday ${date.toLocaleTimeString([], { hour: "numeric", minute: "2-digit" })}`;
  return date.toLocaleDateString([], { month: "short", day: "numeric" });
}

function candidateIsNoisy(candidate: ExtractionCandidate): boolean {
  const hasEvidence = Boolean(candidate.evidence?.length);
  const payloadText = compactPayload(candidate.payload).trim();
  return candidate.confidence < 0.45 || (!hasEvidence && candidate.confidence < 0.65) || payloadText.length < 16;
}

function mergeIds(ids: string[], additions: string[]): string[] {
  const merged = new Set(ids);
  for (const id of additions) merged.add(id);
  return Array.from(merged);
}

function candidateToSearchResult(candidate: ExtractionCandidate): SearchResult | null {
  if (!candidate.trusted_record_type || !candidate.trusted_record_id) return null;
  return {
    record_type: candidate.trusted_record_type,
    id: candidate.trusted_record_id,
    title: candidateTitle(candidate),
    snippet: candidateBody(candidate),
    scope: candidate.scope,
    score: candidate.confidence,
    evidence: candidate.evidence ?? [],
  };
}

function candidateTitle(candidate: ExtractionCandidate): string {
  return (
    candidatePayloadString(candidate, ["title", "name", "entity_name", "key", "id"]) ??
    `${candidate.record_type} candidate`
  );
}

function candidateBody(candidate: ExtractionCandidate): string {
  const payloadSummary =
    candidatePayloadString(candidate, ["reasoning", "content", "details", "observe", "body"]) ??
    compactPayload(candidate.payload);
  const evidenceSummary = (candidate.evidence ?? [])
    .slice(0, 2)
    .map((evidence) => `${evidence.source_type}${evidence.title ? `: ${evidence.title}` : ""}`)
    .join(" / ");
  return [payloadSummary, candidate.rationale, evidenceSummary ? `Evidence: ${evidenceSummary}` : null]
    .filter(Boolean)
    .join(" ");
}

function candidatePayloadString(candidate: ExtractionCandidate, keys: string[]): string | null {
  for (const key of keys) {
    const value = candidate.payload[key];
    if (typeof value === "string" && value.trim()) return value;
    if (typeof value === "number" || typeof value === "boolean") return String(value);
  }
  return null;
}

function compactPayload(payload: Record<string, unknown>): string {
  return Object.entries(payload)
    .slice(0, 4)
    .map(([key, value]) => `${key}: ${payloadValue(value)}`)
    .join(", ");
}

function payloadValue(value: unknown): string {
  if (typeof value === "string") return value;
  if (typeof value === "number" || typeof value === "boolean") return String(value);
  if (Array.isArray(value)) return value.map(payloadValue).join(", ");
  if (value && typeof value === "object") return JSON.stringify(value);
  return "";
}

function contextToSearchResult(document: ContextSummary): SearchResult {
  return {
    record_type: "context",
    id: document.key,
    title: document.title,
    snippet: `${document.category} context, version ${document.version}`,
    scope: document.scope,
    score: null,
  };
}

function stateToSearchResult(item: StateItem): SearchResult {
  return {
    record_type: "state",
    id: item.key,
    title: item.title,
    snippet: `${item.status} priority ${item.priority}${item.owner ? `, owned by ${item.owner}` : ""}`,
    scope: item.scope,
    score: null,
  };
}

function decisionToSearchResult(decision: DecisionItem): SearchResult {
  return {
    record_type: "decision",
    id: decision.id,
    title: decision.title,
    snippet: decision.reasoning || decision.status,
    scope: decision.scope,
    score: null,
  };
}

function relationToSearchResult(relation: GraphRelation): SearchResult {
  return {
    record_type: "relation",
    id: relation.id,
    title: `${relation.from_entity} ${relation.relation} ${relation.to_entity}`,
    snippet: `weight ${relation.weight.toFixed(2)}, confidence ${relation.confidence.toFixed(2)}, source ${relation.source_type}`,
    scope: "",
    score: null,
  };
}

function sessionToSearchResult(session: SessionLogItem): SearchResult {
  return {
    record_type: "session",
    id: session.id,
    title: session.goal || session.id,
    snippet: sessionBody(session),
    scope: session.scope,
    score: null,
  };
}

function sessionBody(session: SessionLogItem): string {
  if (session.summary) return session.summary;
  if (session.handoff_context) return session.handoff_context.split("\n").find((line) => line.trim()) || "Handoff ready.";
  if (session.remaining.length) return `Remaining: ${session.remaining.slice(0, 3).join(", ")}`;
  if (session.accomplishments.length) return `Accomplished: ${session.accomplishments.slice(0, 3).join(", ")}`;
  return `${session.session_type} session is ${session.status}`;
}

function ListHeading({ title, icon: Icon }: { title: string; icon: typeof Activity }) {
  return (
    <header className="list-heading">
      <Icon size={15} />
      <span>{title}</span>
    </header>
  );
}

function Row({ title, meta }: { title: string; meta: string }) {
  return (
    <motion.div className="data-row" layout whileHover={{ x: 2 }} transition={transition.quick}>
      <CircleDot size={12} />
      <span>{title}</span>
      <code>{meta}</code>
    </motion.div>
  );
}

function Setting({ label, value, mono = false }: { label: string; value: string; mono?: boolean }) {
  return (
    <div className="setting-row">
      <span>{label}</span>
      <strong className={mono ? "mono" : ""}>{value}</strong>
    </div>
  );
}

function Segmented(props: {
  value: string;
  options: string[];
  onChange: (value: string) => void;
}) {
  return (
    <div className="segmented">
      {props.options.map((option) => (
        <button
          key={option}
          className={props.value === option ? "active" : ""}
          onClick={() => props.onChange(option)}
        >
          {option}
        </button>
      ))}
    </div>
  );
}
