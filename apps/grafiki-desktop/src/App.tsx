import {
  AnimatePresence,
  LayoutGroup,
  motion,
  useReducedMotion,
} from "framer-motion";
import {
  Activity,
  AlertTriangle,
  CheckCircle2,
  CircleDot,
  Database,
  Download,
  FileText,
  FolderOpen,
  History,
  LayoutDashboard,
  MessageSquare,
  Network,
  PanelRight,
  TerminalSquare,
  Pencil,
  Plus,
  RefreshCcw,
  Settings,
  ShieldQuestion,
  Sparkles,
  Trash2,
  Upload,
  X,
} from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { invoke, Channel } from "@tauri-apps/api/core";
import { Terminal as XTerm } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import "@xterm/xterm/css/xterm.css";
import {
  approveCandidate,
  bulkReviewCandidates,
  chatWithMemory,
  deleteMemoryRecord,
  editCandidate,
  exportMemoryToFile,
  extractSessionMemory,
  listLocalModels,
  getCaptureConfig,
  getDaemonStatus,
  getMemoryRecord,
  getProjectSnapshot,
  importMemoryFromFile,
  initializeProject,
  listCandidates,
  pickProjectFolder,
  processProjectEmbeddings,
  startDaemon,
  stopDaemon,
  rejectCandidate,
  updateMemoryRecord,
  updateCaptureConfig,
  isPreviewMode,
  confirmDialog,
} from "./api";
import { useModalDialog } from "./useModalDialog";
import {
  decodeLayoutFromHash,
  loadInitialLayout,
  newPaneId,
  persistLayout,
  titleForPane,
} from "./layout";
import type {
  CaptureConfigReport,
  CaptureSourceConfig,
  ChatReply,
  DaemonStatus,
  EvidenceLink,
  ExtractionCandidate,
  MemoryRecordDetail,
  PaneKind,
  PaneState,
  ProjectSnapshot,
  SearchResult,
  LayoutState,
} from "./types";

const PROJECT_ROOT_KEY = "grafiki.desktop.projectRoot";

// Ultra-minimal nav (Wispr/Granola feel): the whole app is this core loop.
// `detail` is reachable too (opened from a chat citation or an approved memory),
// it's just not a sidebar destination.
const navItems: Array<{ kind: PaneKind; label: string; icon: typeof LayoutDashboard }> = [
  { kind: "terminal", label: "Terminal", icon: TerminalSquare },
  { kind: "chat", label: "Chat", icon: MessageSquare },
  { kind: "candidates", label: "Review", icon: ShieldQuestion },
  { kind: "settings", label: "Settings", icon: Settings },
];

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

  async function refreshSnapshot() {
    const next = await getProjectSnapshot({ startDir: projectRoot });
    setSnapshot(next);
    return next;
  }

  async function initializeCurrentProject(path?: string) {
    const projectDir = path?.trim() || projectRoot.trim() || snapshot?.start_dir || "";
    if (!projectDir) return;
    await initializeProject({ projectDir });
    setProjectRoot(projectDir);
    const next = await getProjectSnapshot({ startDir: projectDir });
    setSnapshot(next);
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

  return (
    <LayoutGroup>
      <motion.div
        className={`app-shell ${inspectorOpen ? "inspector-open" : ""}`}
        initial={reduceMotion ? false : { opacity: 0 }}
        animate={{ opacity: 1 }}
        transition={transition.quick}
      >
      {isPreviewMode() ? (
        <div className="preview-banner" role="status">
          Preview mode — no Grafiki backend connected. Changes shown here are not saved.
        </div>
      ) : null}
      <Rail
        activeKind={activePane?.kind ?? "terminal"}
        onOpen={(kind) => switchPrimaryPane(kind)}
        reduceMotion={reduceMotion}
      />

      <main className="workspace">
        <TopStatus
          snapshot={snapshot}
          inspectorOpen={inspectorOpen}
          onToggleInspector={() => setInspectorOpen((current) => !current)}
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
                onUpdate={(patch) => updatePane(pane.id, patch)}
                onSelectResult={(result) => {
                  setSelectedResult(result);
                  setInspectorOpen(true);
                }}
                onOpenResult={openResultInPane}
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
            onClose={() => setInspectorOpen(false)}
            reduceMotion={reduceMotion}
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
  reduceMotion: boolean;
}) {
  return (
    <aside className="rail">
      <motion.button
        className="brand"
        aria-label="Grafiki home"
        onClick={() => props.onOpen("terminal")}
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
              title={item.label}
              {...pressMotion(props.reduceMotion)}
            >
              <Icon size={18} />
              <span>{item.label}</span>
            </motion.button>
          );
        })}
      </nav>
    </aside>
  );
}

function TopStatus(props: {
  snapshot: ProjectSnapshot | null;
  inspectorOpen: boolean;
  onToggleInspector: () => void;
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
        <StatusPill tone={memoryAvailable ? "good" : "warn"} icon={memoryAvailable ? CheckCircle2 : AlertTriangle}>
          {memoryAvailable ? "Memory online" : "Initialize needed"}
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
  onUpdate: (patch: Partial<PaneState>) => void;
  onSelectResult: (result: SearchResult) => void;
  onOpenResult: (result: SearchResult) => void;
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
          <button onClick={props.onClose} title="Close pane">
            <X size={15} />
          </button>
        </div>
      </header>

      <div className="pane-body">
        {pane.kind === "terminal" ? <TerminalPane projectRoot={props.projectRoot} /> : null}
        {pane.kind === "chat" ? (
          <ChatPane
            pane={pane}
            snapshot={props.snapshot}
            projectRoot={props.projectRoot}
            onUpdate={props.onUpdate}
            onOpenResult={props.onOpenResult}
          />
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
      </div>
    </motion.article>
  );
}

type TerminalSessionRef = { id: string; launch: string };

function terminalStorageKey(projectRoot: string) {
  return `grafiki-terminal:${projectRoot}`;
}

function loadTerminalSession(projectRoot: string): TerminalSessionRef | null {
  try {
    const raw = localStorage.getItem(terminalStorageKey(projectRoot));
    if (!raw) {
      return null;
    }
    const parsed = JSON.parse(raw) as TerminalSessionRef;
    return typeof parsed?.id === "string" ? parsed : null;
  } catch {
    return null;
  }
}

function TerminalPane(props: { projectRoot: string }) {
  // The session id is STABLE and persisted per project: switching tabs detaches
  // the UI but the PTY (and the agent inside it) keeps running; coming back
  // reattaches and replays scrollback. Only "End session" kills the process.
  const [session, setSession] = useState<TerminalSessionRef | null>(() =>
    loadTerminalSession(props.projectRoot),
  );
  const [error, setError] = useState<string | null>(null);
  const [ended, setEnded] = useState(false);
  // null = connecting; then the backend's honest answer (false = the folder
  // isn't an initialized Grafiki project, so nothing is being recorded).
  const [capturing, setCapturing] = useState<boolean | null>(null);
  const containerRef = useRef<HTMLDivElement | null>(null);
  // Set when the launcher just created `session`, so the effect spawns instead
  // of attaching. A ref (not state): StrictMode remounts must attach, not respawn.
  const spawnRef = useRef(false);

  useEffect(() => {
    setSession(loadTerminalSession(props.projectRoot));
    setError(null);
    setEnded(false);
  }, [props.projectRoot]);

  useEffect(() => {
    if (!session || !containerRef.current) {
      return;
    }
    const id = session.id;
    const launch = session.launch;
    const term = new XTerm({
      fontFamily: '"JetBrains Mono", ui-monospace, "SF Mono", Menlo, monospace',
      fontSize: 13,
      cursorBlink: true,
      scrollback: 5000,
      theme: { background: "#16181c", foreground: "#e8e6e0", cursor: "#ff7a33" },
    });
    const fit = new FitAddon();
    term.loadAddon(fit);
    term.open(containerRef.current);
    fit.fit();

    const channel = new Channel<number[]>();
    channel.onmessage = (bytes) => term.write(new Uint8Array(bytes));

    const resize = () => {
      try {
        fit.fit();
      } catch {
        /* pane not laid out yet */
      }
      void invoke("terminal_resize", { id, rows: term.rows, cols: term.cols });
    };

    let launchTimer: number | undefined;
    // Type a command into the fresh shell exactly once per session
    // (sessionStorage guard survives StrictMode's dev double-mount).
    const launchGuard = `grafiki-terminal-launched:${id}`;
    const scheduleType = (cmd: string) => {
      if (!cmd || sessionStorage.getItem(launchGuard)) {
        return;
      }
      launchTimer = window.setTimeout(() => {
        if (!sessionStorage.getItem(launchGuard)) {
          sessionStorage.setItem(launchGuard, "1");
          void invoke("terminal_write", { id, data: `${cmd}\r` });
        }
      }, 700);
    };

    let cancelled = false;
    const connect = async () => {
      try {
        if (spawnRef.current) {
          spawnRef.current = false;
          // Spawn the login shell (full PATH); the agent is typed in after.
          const opened = await invoke<{ id: string; capturing: boolean }>("terminal_open", {
            id,
            cwd: props.projectRoot,
            command: "",
            launch,
            rows: term.rows,
            cols: term.cols,
            onOutput: channel,
          });
          if (!cancelled) {
            setCapturing(opened.capturing);
            scheduleType(launch);
          }
          return;
        }
        const reply = await invoke<{
          found: boolean;
          exited: boolean;
          cwd: string;
          capturing: boolean;
        }>("terminal_attach", { id, onOutput: channel });
        if (cancelled) {
          return;
        }
        if (!reply.found) {
          // App relaunched, live PTY is gone: revive from the disk descriptor —
          // same folder, previous output replayed, agent resumed.
          const revive = await invoke<{
            found: boolean;
            launch: string;
            cwd: string;
            capturing: boolean;
          }>("terminal_revive", { id, rows: term.rows, cols: term.cols, onOutput: channel });
          if (cancelled) {
            return;
          }
          if (!revive.found) {
            // Nothing to revive (explicitly ended): back to the launcher.
            localStorage.removeItem(terminalStorageKey(props.projectRoot));
            sessionStorage.removeItem(launchGuard);
            setSession(null);
            return;
          }
          setCapturing(revive.capturing);
          // Resume the agent's own session where supported; otherwise relaunch it.
          scheduleType(revive.launch === "claude" ? "claude --continue" : revive.launch);
          return;
        }
        setCapturing(reply.capturing);
        if (reply.exited) {
          setEnded(true);
          return;
        }
        // Reattached to a live session: sync the PTY to the new pane size and
        // finish the launch typing if a dev remount interrupted it.
        resize();
        scheduleType(launch);
      } catch (connectError) {
        if (!cancelled) {
          setError(String(connectError));
        }
      }
    };
    void connect();

    const onData = term.onData((data) => {
      void invoke("terminal_write", { id, data });
    });
    const observer = new ResizeObserver(resize);
    observer.observe(containerRef.current);
    term.focus();

    // The Granola heartbeat: periodically turn this session's captured output
    // into Review candidates (backend is single-flight; silent — extraction
    // must never disturb the terminal).
    const extractTimer = window.setInterval(() => {
      extractSessionMemory({ startDir: props.projectRoot }).catch(() => undefined);
    }, 120_000);

    return () => {
      cancelled = true;
      if (launchTimer) {
        window.clearTimeout(launchTimer);
      }
      window.clearInterval(extractTimer);
      observer.disconnect();
      onData.dispose();
      // Detach ONLY — the session (and the agent) keeps running in the pool.
      void invoke("terminal_detach", { id });
      term.dispose();
      // One more pass on the way out, so Review is fresh when the user lands there.
      extractSessionMemory({ startDir: props.projectRoot }).catch(() => undefined);
    };
  }, [session, props.projectRoot]);

  const endSession = () => {
    if (session) {
      void invoke("terminal_close", { id: session.id });
      localStorage.removeItem(terminalStorageKey(props.projectRoot));
      sessionStorage.removeItem(`grafiki-terminal-launched:${session.id}`);
    }
    setSession(null);
    setEnded(false);
    setError(null);
    setCapturing(null);
  };

  const startSession = (cmd: string) => {
    const id = `term-${Math.random().toString(36).slice(2, 10)}-${Date.now().toString(36)}`;
    const next = { id, launch: cmd };
    localStorage.setItem(terminalStorageKey(props.projectRoot), JSON.stringify(next));
    spawnRef.current = true;
    setEnded(false);
    setError(null);
    setCapturing(null);
    setSession(next);
  };

  if (session === null) {
    const options = [
      { label: "Claude Code", cmd: "claude" },
      { label: "Codex", cmd: "codex" },
      { label: "Gemini", cmd: "gemini" },
      { label: "Shell", cmd: "" },
    ];
    return (
      <div
        className="view-stack"
        style={{ padding: 28, display: "flex", flexDirection: "column", gap: 16, alignItems: "flex-start" }}
      >
        <div>
          <h2 style={{ margin: 0 }}>Start a session</h2>
          <p className="muted" style={{ marginTop: 6, maxWidth: 460 }}>
            It runs inside Grafiki, in <code>{props.projectRoot || "this project"}</code>. Work
            normally — everything in this session is captured automatically, no setup.
          </p>
        </div>
        <div style={{ display: "flex", gap: 10, flexWrap: "wrap" }}>
          {options.map((option) => (
            <button key={option.label} onClick={() => startSession(option.cmd)} style={{ padding: "9px 18px" }}>
              {option.label}
            </button>
          ))}
        </div>
      </div>
    );
  }

  return (
    <div className="view-stack" style={{ display: "flex", flexDirection: "column", height: "100%", gap: 8 }}>
      <div className="toolbar-row" style={{ alignItems: "center", gap: 10 }}>
        <span className="muted" style={{ fontSize: 12 }}>
          {session.launch ? `Running: ${session.launch}` : "Shell"} ·{" "}
          {props.projectRoot || "this project"}
          {capturing === true ? " · capturing" : ""}
          {capturing === false ? " · not capturing — initialize this folder in Settings" : ""}
          {ended ? " · session ended" : ""}
        </span>
        <button style={{ marginLeft: "auto", padding: "4px 10px" }} onClick={endSession}>
          {ended ? "New session" : "End session"}
        </button>
      </div>
      {error ? <p style={{ color: "var(--danger, #ff6b6b)" }}>{error}</p> : null}
      <div
        ref={containerRef}
        style={{ flex: 1, minHeight: 0, background: "#16181c", borderRadius: 6, overflow: "hidden", padding: 6 }}
      />
    </div>
  );
}

function ChatPane(props: {
  pane: PaneState;
  snapshot: ProjectSnapshot | null;
  projectRoot: string;
  onUpdate: (patch: Partial<PaneState>) => void;
  onOpenResult: (result: SearchResult) => void;
}) {
  const [question, setQuestion] = useState("");
  const [scope, setScope] = useState(props.pane.scope ?? props.snapshot?.scope ?? "");
  const [useModel, setUseModel] = useState(false);
  const [model, setModel] = useState("gemma3:1b");
  // null = still probing Ollama; [] = Ollama down or no models pulled.
  const [localModels, setLocalModels] = useState<string[] | null>(null);
  const [turns, setTurns] = useState<
    Array<{ question: string; reply: ChatReply | null; error: string | null }>
  >([]);
  const [sending, setSending] = useState(false);

  // Offer the models the user actually HAS: keep the default only if it's
  // installed, otherwise switch to the first installed model. Never leave the
  // field pointing at a model that would silently fail.
  useEffect(() => {
    let cancelled = false;
    listLocalModels()
      .then((models) => {
        if (cancelled) {
          return;
        }
        setLocalModels(models);
        if (models.length > 0 && !models.includes("gemma3:1b")) {
          setModel((current) => (models.includes(current) ? current : models[0]));
        }
      })
      .catch(() => {
        if (!cancelled) {
          setLocalModels([]);
        }
      });
    return () => {
      cancelled = true;
    };
  }, []);

  async function ask() {
    const q = question.trim();
    if (!q || sending) return;
    setSending(true);
    setQuestion("");
    setTurns((prev) => [...prev, { question: q, reply: null, error: null }]);
    props.onUpdate({ title: `Chat: ${q}`, scope });
    try {
      const reply = await chatWithMemory({
        startDir: props.projectRoot,
        question: q,
        scope,
        model: useModel ? model : undefined,
      });
      setTurns((prev) =>
        prev.map((turn, index) => (index === prev.length - 1 ? { ...turn, reply } : turn)),
      );
    } catch (chatError) {
      setTurns((prev) =>
        prev.map((turn, index) =>
          index === prev.length - 1 ? { ...turn, error: String(chatError) } : turn,
        ),
      );
    } finally {
      setSending(false);
    }
  }

  return (
    <div
      className="view-stack chat-view"
      style={{ display: "flex", flexDirection: "column", height: "100%", gap: 12 }}
    >
      <div
        style={{
          flex: 1,
          overflowY: "auto",
          display: "flex",
          flexDirection: "column",
          gap: 16,
        }}
      >
        {turns.length === 0 ? (
          <p className="muted" style={{ margin: "auto", textAlign: "center", maxWidth: 380 }}>
            Ask your memory anything. Answers are built only from what Grafiki has stored — with
            sources — and it tells you honestly when it doesn't know.
          </p>
        ) : null}
        {turns.map((turn, index) => (
          <div key={index} style={{ display: "flex", flexDirection: "column", gap: 8 }}>
            <div
              style={{
                alignSelf: "flex-end",
                background: "rgba(255,255,255,0.06)",
                padding: "8px 12px",
                borderRadius: 10,
                maxWidth: "80%",
              }}
            >
              {turn.question}
            </div>
            <div style={{ alignSelf: "flex-start", maxWidth: "92%" }}>
              {turn.reply ? (
                <>
                  <div style={{ whiteSpace: "pre-wrap", lineHeight: 1.5 }}>{turn.reply.answer}</div>
                  {turn.reply.citations.length > 0 ? (
                    <div style={{ display: "flex", flexWrap: "wrap", gap: 6, marginTop: 8 }}>
                      {turn.reply.citations.map((citation) => (
                        <button
                          key={citation.index}
                          title={citation.snippet}
                          onClick={() =>
                            props.onOpenResult({
                              record_type: citation.record_type,
                              id: citation.id,
                              title: citation.title,
                              snippet: citation.snippet,
                              scope,
                            })
                          }
                          style={{
                            fontSize: 12,
                            padding: "3px 8px",
                            borderRadius: 999,
                            border: "1px solid rgba(255,255,255,0.15)",
                            background: "transparent",
                            cursor: "pointer",
                          }}
                        >
                          [{citation.index}] {citation.title || citation.record_type}
                        </button>
                      ))}
                    </div>
                  ) : null}
                  {turn.reply.flagged_injection ? (
                    <p className="muted" style={{ marginTop: 6, fontSize: 12 }}>
                      ⚠ Some retrieved memory looks like it contains instructions — treated as data,
                      not commands.
                    </p>
                  ) : null}
                </>
              ) : turn.error ? (
                <p style={{ color: "var(--danger, #ff6b6b)" }}>{turn.error}</p>
              ) : (
                <p className="muted">Thinking…</p>
              )}
            </div>
          </div>
        ))}
      </div>

      <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
        <div className="toolbar-row" style={{ gap: 12, alignItems: "center" }}>
          <label style={{ display: "flex", alignItems: "center", gap: 6 }}>
            <input
              type="checkbox"
              checked={useModel}
              onChange={(event) => setUseModel(event.target.checked)}
            />
            <span>Use local AI</span>
          </label>
          {useModel ? (
            <>
              <input
                value={model}
                onChange={(event) => setModel(event.target.value)}
                placeholder="gemma3:1b"
                list="grafiki-local-models"
                style={{ width: 170 }}
                title="Local model served by Ollama. Suggestions are the models you have installed."
              />
              <datalist id="grafiki-local-models">
                {(localModels ?? []).map((name) => (
                  <option key={name} value={name} />
                ))}
              </datalist>
              {localModels !== null && localModels.length === 0 ? (
                <span className="muted" style={{ fontSize: 12 }}>
                  Ollama not reachable — answers stay extractive
                </span>
              ) : null}
            </>
          ) : null}
          <label className="compact-select" style={{ marginLeft: "auto" }}>
            <span>Scope</span>
            <input
              value={scope}
              onChange={(event) => setScope(event.target.value)}
              placeholder="global or project/module"
            />
          </label>
        </div>
        <div className="search-box">
          <MessageSquare size={17} />
          <input
            value={question}
            onChange={(event) => setQuestion(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter" && !event.shiftKey) {
                event.preventDefault();
                void ask();
              }
            }}
            placeholder="Ask your memory…"
            autoComplete="off"
          />
          <button
            onClick={() => void ask()}
            disabled={sending || !question.trim()}
            style={{ padding: "6px 14px" }}
          >
            {sending ? "…" : "Ask"}
          </button>
        </div>
      </div>
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
  const [status, setStatus] = useState("pending");
  const [scope, setScope] = useState(props.snapshot?.scope ?? "");
  const [loading, setLoading] = useState(false);
  const [busyId, setBusyId] = useState<string | null>(null);
  const [selectedIds, setSelectedIds] = useState<string[]>([]);
  const [focusedCandidateId, setFocusedCandidateId] = useState<string | null>(null);
  const [minConfidence, setMinConfidence] = useState("0");
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editPayload, setEditPayload] = useState("");
  const [promptModal, setPromptModal] = useState<PromptConfig | null>(null);
  const [editScope, setEditScope] = useState("");
  const [editConfidence, setEditConfidence] = useState("0.5");
  const [editRationale, setEditRationale] = useState("");
  const [evidencePreview, setEvidencePreview] = useState<EvidenceLink | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const parsedConfidence = Number.parseFloat(minConfidence);
  // Confidence is 0..1; clamp so a stray "9" can't silently hide everything.
  const minConfidenceValue = Number.isFinite(parsedConfidence)
    ? Math.min(Math.max(parsedConfidence, 0), 1)
    : 0;
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
      const nextCandidates = await listCandidates({
        startDir: props.startDir,
        scope,
        status,
        limit: 100,
      });
      setCandidates(nextCandidates);
      setSelectedIds((ids) =>
        ids.filter((id) => nextCandidates.some((candidate) => candidate.id === id && candidate.status === "pending")),
      );
      setFocusedCandidateId((id) => {
        if (id && nextCandidates.some((candidate) => candidate.id === id)) return id;
        return nextCandidates.find((candidate) => candidate.status === "pending")?.id ?? nextCandidates[0]?.id ?? null;
      });
    } catch (listError) {
      setError(String(listError));
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    load();
  }, [props.startDir, props.snapshot, scope, status]);

  // Opening Review runs one extraction pass over anything captured since the
  // last one (terminal output, transcripts), so fresh candidates are waiting.
  useEffect(() => {
    let cancelled = false;
    extractSessionMemory({ startDir: props.startDir })
      .then((report) => {
        if (!cancelled && report && report.proposed > 0) {
          void load();
        }
      })
      .catch(() => undefined);
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [props.startDir]);

  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      // Only the active pane handles global candidate shortcuts; otherwise
      // pressing `a`/`r` while reading another pane would silently mutate
      // trusted memory. Also ignore shortcuts while a prompt modal is open,
      // otherwise a keystroke could act on the candidate hidden behind it.
      if (!props.active || promptModal) {
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
  }, [props.active, promptModal, busyId, editingId, focusedCandidateId, selectedIds, visibleCandidates]);

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

  function reject(candidate: ExtractionCandidate) {
    setPromptModal({
      title: "Reject candidate",
      submitLabel: "Reject",
      fields: [
        {
          name: "rationale",
          label: "Reject rationale",
          type: "textarea",
          defaultValue: candidate.rationale ?? "",
          placeholder: "Why is this being rejected?",
        },
      ],
      onSubmit: (values) => {
        setPromptModal(null);
        void performReject(candidate, values.rationale ?? "");
      },
    });
  }

  async function performReject(candidate: ExtractionCandidate, rationale: string) {
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

  function bulkReview(action: "approve" | "reject", ids = selectedIds) {
    if (!ids.length) {
      setMessage("Select at least one pending candidate.");
      return;
    }
    if (action === "reject") {
      setPromptModal({
        title: `Reject ${ids.length} candidate${ids.length === 1 ? "" : "s"}`,
        submitLabel: "Reject",
        fields: [
          {
            name: "rationale",
            label: "Reject rationale",
            type: "textarea",
            defaultValue: "Bulk review cleanup",
          },
        ],
        onSubmit: (values) => {
          setPromptModal(null);
          void performBulk(action, ids, values.rationale ?? "");
        },
      });
      return;
    }
    void performBulk(action, ids, "");
  }

  async function performBulk(action: "approve" | "reject", ids: string[], rationale: string) {
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

  return (
    <div className="view-stack">
      <AnimatePresence>
        {promptModal ? (
          <PromptModal
            config={promptModal}
            reduceMotion={props.reduceMotion}
            onClose={() => setPromptModal(null)}
          />
        ) : null}
      </AnimatePresence>
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
            type="number"
            min={0}
            max={1}
            step={0.05}
            value={minConfidence}
            onChange={(event) => setMinConfidence(event.target.value)}
            inputMode="decimal"
            placeholder="0"
          />
        </label>
        <span className="subtle">{visibleCandidates.length}/{candidates.length} candidates</span>
        {candidates.length > 0 && visibleCandidates.length === 0 && minConfidenceValue > 0 ? (
          <span className="subtle">
            All hidden below {minConfidenceValue.toFixed(2)} — lower Min Confidence to see them.
          </span>
        ) : null}
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
        startDir: draftRoot || props.projectRoot || snapshot?.start_dir || "",
        host: daemonHost,
        port: daemonPort,
        token: daemonToken,
      });
      // Surface the auto-generated token so the user can give it to external agents.
      if (result.token) setDaemonToken(result.token);
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
      const result = await stopDaemon({
        startDir: draftRoot || props.projectRoot || snapshot?.start_dir || "",
      });
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
              placeholder="auto-generated on Start"
            />
          </label>
          {daemonToken ? (
            <p className="daemon-token-hint">
              External agents authenticate with this token (header{" "}
              <code>X-Grafiki-Token</code>).{" "}
              <button
                className="link-button"
                type="button"
                onClick={() => {
                  void navigator.clipboard?.writeText(daemonToken);
                  setMessage("Daemon token copied to clipboard.");
                }}
              >
                Copy
              </button>
            </p>
          ) : null}
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
    if (!(await confirmDialog(`Delete ${recordType} "${title}"?`, { okLabel: "Delete" }))) return;
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

function Inspector(props: {
  snapshot: ProjectSnapshot | null;
  activePane?: PaneState;
  selectedResult: SearchResult | null;
  recordDetail: MemoryRecordDetail | null;
  recordDetailLoading: boolean;
  recordDetailError: string | null;
  onOpenDetail: () => void;
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

type PromptField = {
  name: string;
  label: string;
  type: "text" | "textarea" | "select";
  options?: { value: string; label: string }[];
  defaultValue?: string;
  placeholder?: string;
};

type PromptConfig = {
  title: string;
  fields: PromptField[];
  submitLabel?: string;
  onSubmit: (values: Record<string, string>) => void;
};

// In-app replacement for window.prompt (which the Tauri webview can suppress).
// Collects one or more text/textarea/select values; Cancel/Escape/backdrop close
// without submitting.
function PromptModal(props: { config: PromptConfig; reduceMotion: boolean; onClose: () => void }) {
  const { config } = props;
  const dialogRef = useModalDialog<HTMLElement>(props.onClose);
  const [values, setValues] = useState<Record<string, string>>(() =>
    Object.fromEntries(config.fields.map((field) => [field.name, field.defaultValue ?? ""])),
  );
  const setField = (name: string, value: string) =>
    setValues((current) => ({ ...current, [name]: value }));
  const submit = () => config.onSubmit(values);

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
        ref={dialogRef}
        className="launcher prompt-modal"
        role="dialog"
        aria-modal="true"
        aria-label={config.title}
        tabIndex={-1}
        onMouseDown={(event) => event.stopPropagation()}
        initial={props.reduceMotion ? false : { opacity: 0, y: 12, scale: 0.985 }}
        animate={{ opacity: 1, y: 0, scale: 1 }}
        exit={props.reduceMotion ? undefined : { opacity: 0, y: 8, scale: 0.985 }}
        transition={transition.modal}
      >
        <header>
          <strong>{config.title}</strong>
          <motion.button onClick={props.onClose} {...pressMotion(props.reduceMotion)}>
            <X size={16} />
          </motion.button>
        </header>
        <div className="prompt-fields">
          {config.fields.map((field, index) => (
            <label key={field.name}>
              <span>{field.label}</span>
              {field.type === "textarea" ? (
                <textarea
                  autoFocus={index === 0}
                  value={values[field.name]}
                  placeholder={field.placeholder}
                  onChange={(event) => setField(field.name, event.target.value)}
                />
              ) : field.type === "select" ? (
                <select
                  autoFocus={index === 0}
                  value={values[field.name]}
                  onChange={(event) => setField(field.name, event.target.value)}
                >
                  {field.options?.map((option) => (
                    <option key={option.value} value={option.value}>
                      {option.label}
                    </option>
                  ))}
                </select>
              ) : (
                <input
                  autoFocus={index === 0}
                  value={values[field.name]}
                  placeholder={field.placeholder}
                  onChange={(event) => setField(field.name, event.target.value)}
                  onKeyDown={(event) => {
                    if (event.key === "Enter") submit();
                  }}
                />
              )}
            </label>
          ))}
        </div>
        <footer className="prompt-actions">
          <button className="button secondary" onClick={props.onClose}>
            Cancel
          </button>
          <button className="button primary" onClick={submit}>
            {config.submitLabel ?? "Submit"}
          </button>
        </footer>
      </motion.section>
    </motion.div>
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

