import {
  AnimatePresence,
  LayoutGroup,
  motion,
  useReducedMotion,
} from "framer-motion";
import {
  Activity,
  AlertTriangle,
  ArrowLeft,
  CheckCircle2,
  CircleDot,
  Database,
  Download,
  FileText,
  FolderOpen,
  History,
  Home as HomeIcon,
  LayoutDashboard,
  MessageSquare,
  Network,
  PanelRight,
  TerminalSquare,
  Pencil,
  Plus,
  RefreshCcw,
  Search,
  Settings,
  ShieldQuestion,
  Sparkles,
  Trash2,
  Upload,
  X,
} from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { invoke, Channel } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import "@xterm/xterm/css/xterm.css";
import {
  approveCandidate,
  bulkReviewCandidates,
  chatWithMemory,
  captureMemory,
  deleteMemoryRecord,
  editCandidate,
  exportMemoryToFile,
  extractSessionMemory,
  getHomeLedger,
  getLiveTranscript,
  getSessionDetail,
  listLocalModels,
  getCaptureConfig,
  getDaemonStatus,
  getMemoryRecord,
  getProjectSnapshot,
  importMemoryFromFile,
  initializeProject,
  listCandidates,
  listAgentActivity,
  listProjectContext,
  listProjectDecisions,
  pickProjectFolder,
  processProjectEmbeddings,
  searchProjectMemory,
  startDaemon,
  stopDaemon,
  rejectCandidate,
  reopenCandidate,
  revertCandidateApproval,
  updateMemoryRecord,
  updateCaptureConfig,
  isPreviewMode,
  confirmDialog,
} from "./api";
import type { HomeLedgerReport, LiveTranscriptTurn, SessionDetailReport } from "./api";
import Onboarding from "./Onboarding";
import ErrorBoundary from "./ErrorBoundary";
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
  AgentQueryLogItem,
  ChatReply,
  ContextSummary,
  DecisionItem,
  DaemonStatus,
  EvidenceLink,
  ExtractionCandidate,
  MemoryRecordDetail,
  PaneKind,
  PaneState,
  ProjectSnapshot,
  SearchResult,
  SearchMode,
  LayoutState,
} from "./types";

const PROJECT_ROOT_KEY = "grafiki.desktop.projectRoot";
const THEME_KEY = "grafiki.theme";

type ThemePref = "system" | "light" | "dark";

function applyTheme(pref: ThemePref) {
  const dark =
    pref === "dark" ||
    (pref === "system" && window.matchMedia("(prefers-color-scheme: dark)").matches);
  document.documentElement.dataset.theme = dark ? "dark" : "light";
}

// Ultra-minimal nav (Wispr/Granola feel): the whole app is this core loop.
// `detail` is reachable too (opened from a chat citation or an approved memory),
// it's just not a sidebar destination.
const navItems: Array<{ kind: PaneKind; label: string; icon: typeof LayoutDashboard }> = [
  { kind: "home", label: "Home", icon: HomeIcon },
  { kind: "terminal", label: "Sessions", icon: TerminalSquare },
  { kind: "chat", label: "Memory", icon: MessageSquare },
  { kind: "candidates", label: "Review", icon: ShieldQuestion },
  { kind: "settings", label: "Settings", icon: Settings },
];

// Rail destinations are PAGES — the rail is the only way in/out, so they render
// no close ✕ (it was a no-op there anyway: closePane early-returns when there's
// one pane). Only stacked drill-ins get a Back affordance.
const DRILL_IN_PANE_KINDS = new Set<PaneKind>(["session", "detail"]);

// One-line purpose statement under a page title — replaces the incoherent
// eyebrow and makes the capture→review→ask lifecycle legible. "" = no line.
function paneSubtitle(kind: PaneKind): string {
  switch (kind) {
    case "candidates":
      return "Review captured candidates before they become memory.";
    case "chat":
      return "Ask your approved memories and project context.";
    case "settings":
      return "Capture, local AI, and project configuration.";
    default:
      return "";
  }
}

// Shown before any one-click "turn on capture" action outside onboarding, so
// enabling full terminal-output capture always carries the same disclosure the
// onboarding consent checkbox does.
const CAPTURE_ENABLE_DISCLOSURE =
  "Turn on terminal capture for this folder?\n\nGrafiki will store this workspace's terminal " +
  "output as local, redacted capture events so it can become reviewable memory. Nothing leaves " +
  "this Mac, and you can turn it off anytime in Settings → Capture & privacy.";

// Pane titles render as the page heading; an unbounded one (a 2,000-char ask)
// collapsed the chat scroller to 0px. The full text still flows as `query`.
function clampTitle(text: string, max = 90): string {
  const oneLine = text.replace(/\s+/g, " ").trim();
  return oneLine.length > max ? `${oneLine.slice(0, max)}…` : oneLine;
}

// ── Background-error channel ─────────────────────────────────────────────
// Best-effort work (capture, extraction, polls) must never disturb the pane
// it runs behind — but its failures must land SOMEWHERE the user can see, or
// the app silently stops learning while claiming to capture. Any code can
// call notifyBackground(); App renders the queue as dismissible notices.
type BackgroundNotice = { id: number; text: string };
const backgroundNoticeListeners = new Set<(notice: BackgroundNotice) => void>();
let backgroundNoticeSeq = 0;
const backgroundNoticeLastShown = new Map<string, number>();

function notifyBackground(text: string) {
  // The extraction heartbeat retries every 2 minutes; dedupe identical
  // failures so a down Ollama is one notice, not a toast storm.
  const now = Date.now();
  const last = backgroundNoticeLastShown.get(text) ?? 0;
  if (now - last < 5 * 60_000) return;
  backgroundNoticeLastShown.set(text, now);
  const notice = { id: ++backgroundNoticeSeq, text };
  backgroundNoticeListeners.forEach((listener) => listener(notice));
}

// Every extraction entry point funnels through here so a failure is surfaced
// once, uniformly, instead of each call site inventing its own silence.
async function runExtraction(
  options: Parameters<typeof extractSessionMemory>[0],
  source: string,
): Promise<Awaited<ReturnType<typeof extractSessionMemory>> | null> {
  try {
    return await extractSessionMemory(options);
  } catch (error) {
    notifyBackground(`Memory extraction failed (${source}): ${String(error)}`);
    return null;
  }
}

function handleTablistKeyDown(event: React.KeyboardEvent<HTMLElement>) {
  if (!["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key)) return;
  const target = (event.target as HTMLElement).closest<HTMLElement>('[role="tab"]');
  if (!target) return;
  const tabs = Array.from(
    event.currentTarget.querySelectorAll<HTMLElement>('[role="tab"]:not([disabled])'),
  );
  const index = tabs.indexOf(target);
  if (index < 0 || tabs.length === 0) return;
  event.preventDefault();
  const nextIndex =
    event.key === "Home"
      ? 0
      : event.key === "End"
        ? tabs.length - 1
        : (index + (event.key === "ArrowRight" ? 1 : -1) + tabs.length) % tabs.length;
  tabs[nextIndex].focus();
  tabs[nextIndex].click();
}

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

// Heuristic markers for "the agent is waiting on a permission/trust decision"
// in raw terminal output — the JSONL transcript the chat lens tails never
// records these TUI-only prompts, so without this the lens shows the last
// normal bubble while the session is actually stuck (lowercase, substring).
const PERMISSION_PROMPT_MARKERS = [
  "do you want to proceed",
  "do you trust the files",
  "1. yes",
  "2. no",
  "enter to confirm",
];
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

// Motion happens at moments of ARRIVAL (a pane appears, a banner lands, a
// memory is learned) — never on hover, never as springs. DESIGN.md §7.
const transition = {
  quick: { duration: 0.14, ease: [0.2, 0, 0, 1] },
  pane: { duration: 0.16, ease: [0.2, 0, 0, 1] },
  modal: { duration: 0.18, ease: [0.2, 0, 0, 1] },
} as const;

export default function App() {
  const [layout, setLayout] = useState<LayoutState>(() => loadInitialLayout());
  const [projectRoot, setProjectRoot] = useState(() => localStorage.getItem(PROJECT_ROOT_KEY) ?? "");
  const [snapshot, setSnapshot] = useState<ProjectSnapshot | null>(null);
  const [selectedResult, setSelectedResult] = useState<SearchResult | null>(null);
  const [recordDetail, setRecordDetail] = useState<MemoryRecordDetail | null>(null);
  const [recordDetailError, setRecordDetailError] = useState<string | null>(null);
  const [recordDetailLoading, setRecordDetailLoading] = useState(false);
  const [detailRevision, setDetailRevision] = useState(0);
  const [inspectorOpen, setInspectorOpen] = useState(false);
  // At ≤1100px (the app's normal/min width) the Inspector renders as a fixed
  // overlay ON TOP of the workspace — so the covered controls must not stay
  // keyboard-focusable behind it. Track the breakpoint to make the workspace
  // `inert` while the overlay is open.
  // Initialize synchronously from the media query — a false→true flip in a
  // post-mount effect added a re-render that raced the boot pane transition
  // (AnimatePresence briefly kept two Home panes, tripping the e2e).
  const [inspectorIsOverlay, setInspectorIsOverlay] = useState(
    () => window.matchMedia("(max-width: 1100px)").matches,
  );
  useEffect(() => {
    const media = window.matchMedia("(max-width: 1100px)");
    const sync = () => setInspectorIsOverlay(media.matches);
    media.addEventListener("change", sync);
    return () => media.removeEventListener("change", sync);
  }, []);
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [onboarding, setOnboarding] = useState(
    () => !localStorage.getItem("grafiki.onboarded") && !localStorage.getItem(PROJECT_ROOT_KEY),
  );
  const reduceMotion = useReducedMotion() ?? false;
  const snapshotRequestRef = useRef(0);
  const ledgerRequestRef = useRef(0);

  const finishOnboarding = (launch: string | null) => {
    localStorage.setItem("grafiki.onboarded", "1");
    setOnboarding(false);
    if (launch !== null) {
      switchPrimaryPane("terminal", { query: launch });
    }
  };

  useEffect(() => {
    refreshSnapshot();
  }, [projectRoot]);

  useEffect(() => {
    persistLayout(layout);
  }, [layout]);

  const [homeLedger, setHomeLedger] = useState<HomeLedgerReport | null>(null);
  const refreshLedger = () => {
    const request = ++ledgerRequestRef.current;
    getHomeLedger({ startDir: projectRoot })
      .then((ledger) => {
        if (request === ledgerRequestRef.current) setHomeLedger(ledger);
      })
      .catch((ledgerError) => {
        if (request === ledgerRequestRef.current) {
          setHomeLedger(null);
          // A blank Home is indistinguishable from "no activity this week" —
          // say why the numbers vanished instead of letting trust erode.
          notifyBackground(`Couldn't load the Home ledger: ${String(ledgerError)}`);
        }
      });
  };
  useEffect(() => {
    refreshLedger();
    // Keep the ledger (and the Review badge) fresh while the app sits open.
    const timer = window.setInterval(refreshLedger, 60_000);
    return () => window.clearInterval(timer);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [projectRoot]);
  useEffect(() => {
    // Coming back to Home or Review should always show current numbers.
    if (activePane?.kind === "home" || activePane?.kind === "candidates") {
      refreshLedger();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [layout.activePaneId]);

  useEffect(() => {
    if (projectRoot.trim()) localStorage.setItem(PROJECT_ROOT_KEY, projectRoot);
    else localStorage.removeItem(PROJECT_ROOT_KEY);
    setSelectedResult(null);
    setRecordDetail(null);
  }, [projectRoot]);

  // Background-error notices: the landing surface for best-effort failures
  // (extraction, capture polls) that would otherwise vanish into `void`.
  const [backgroundNotices, setBackgroundNotices] = useState<BackgroundNotice[]>([]);
  useEffect(() => {
    const onNotice = (notice: BackgroundNotice) => {
      setBackgroundNotices((current) => [...current.slice(-2), notice]);
      window.setTimeout(() => {
        setBackgroundNotices((current) => current.filter((n) => n.id !== notice.id));
      }, 12_000);
    };
    backgroundNoticeListeners.add(onNotice);
    return () => {
      backgroundNoticeListeners.delete(onNotice);
    };
  }, []);

  useEffect(() => {
    const pref = (localStorage.getItem(THEME_KEY) as ThemePref) ?? "light";
    applyTheme(pref);
    const media = window.matchMedia("(prefers-color-scheme: dark)");
    const onSystemChange = () => {
      if (((localStorage.getItem(THEME_KEY) as ThemePref) ?? "light") === "system") {
        applyTheme("system");
      }
    };
    media.addEventListener("change", onSystemChange);
    return () => media.removeEventListener("change", onSystemChange);
  }, []);

  useEffect(() => {
    if (isPreviewMode()) {
      return; // no Tauri event bridge in browser preview
    }
    // Tray menu deep-links (e.g. "Review: n pending" in the menubar).
    const unlisten = listen<string>("grafiki://navigate", (event) => {
      switchPrimaryPane(event.payload as PaneKind);
    });
    return () => {
      void unlisten.then((dispose) => dispose());
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    const onPaletteKey = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "k") {
        event.preventDefault();
        setPaletteOpen((current) => !current);
      }
    };
    window.addEventListener("keydown", onPaletteKey);
    return () => window.removeEventListener("keydown", onPaletteKey);
  }, []);

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
  }, [detailTarget?.recordType, detailTarget?.id, detailTarget?.scope, detailTarget?.startDir, detailRevision]);

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
    const current = layout.panes.find((candidate) => candidate.id === layout.activePaneId);
    if (current && current.kind === kind && Object.keys(patch).length === 0) {
      // Already showing this pane with no new params — skip the pointless
      // exit/enter transition a fresh pane id would trigger (e.g. clicking
      // the brand logo or a rail item while already on that screen briefly
      // rendered two copies of the pane mid-transition).
      return;
    }
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

  async function refreshSnapshot(startDir = projectRoot) {
    const request = ++snapshotRequestRef.current;
    const next = await getProjectSnapshot({ startDir });
    if (request === snapshotRequestRef.current) setSnapshot(next);
    return next;
  }

  async function refreshMemory() {
    const next = await refreshSnapshot();
    setDetailRevision((revision) => revision + 1);
    return next;
  }

  async function initializeCurrentProject(path?: string) {
    const projectDir = path?.trim() || projectRoot.trim() || snapshot?.start_dir || "";
    if (!projectDir) return;
    await initializeProject({ projectDir });
    setProjectRoot(projectDir);
    await refreshSnapshot(projectDir);
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

  if (onboarding) {
    return (
      <Onboarding
        onProjectReady={(dir) => setProjectRoot(dir)}
        onFinished={finishOnboarding}
      />
    );
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
        activeKind={activePane?.kind ?? "home"}
        pendingCount={homeLedger?.ledger.pending_candidates ?? 0}
        onOpen={(kind) => switchPrimaryPane(kind)}
        onOpenPalette={() => setPaletteOpen(true)}
        reduceMotion={reduceMotion}
      />

      <main className="workspace" inert={inspectorOpen && inspectorIsOverlay ? true : undefined}>
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
                onMemoryChanged={refreshMemory}
                ledger={homeLedger}
                onRefreshLedger={refreshLedger}
                onNavigate={switchPrimaryPane}
              />
            ))}
          </AnimatePresence>
        </section>
      </main>

      <AnimatePresence initial={false}>
        {paletteOpen ? (
          <CommandPalette
            onClose={() => setPaletteOpen(false)}
            onAsk={(question) =>
              switchPrimaryPane("chat", { query: question, title: `Memory: ${clampTitle(question)}` })
            }
            actions={[
              { id: "home", label: "Go to Home", hint: "ledger", run: () => switchPrimaryPane("home") },
              { id: "sessions", label: "Go to Sessions", run: () => switchPrimaryPane("terminal") },
              { id: "memory", label: "Go to Memory", hint: "chat", run: () => switchPrimaryPane("chat") },
              {
                id: "review",
                label: `Go to Review${(homeLedger?.ledger.pending_candidates ?? 0) > 0 ? ` (${homeLedger?.ledger.pending_candidates} pending)` : ""}`,
                run: () => switchPrimaryPane("candidates"),
              },
              { id: "settings", label: "Go to Settings", run: () => switchPrimaryPane("settings") },
              {
                id: "start-claude",
                label: "Start Claude Code session",
                run: () => switchPrimaryPane("terminal", { query: "claude" }),
              },
              {
                id: "start-shell",
                label: "Start shell session",
                run: () => switchPrimaryPane("terminal", { query: "" }),
              },
              ...(homeLedger?.resumable
                ? [
                    {
                      id: "resume",
                      label: "Resume last session",
                      hint: homeLedger.resumable.launch || "shell",
                      run: () => {
                        localStorage.setItem(
                          terminalStorageKey(projectRoot),
                          JSON.stringify({
                            id: homeLedger.resumable!.id,
                            launch: homeLedger.resumable!.launch,
                            captureId: homeLedger.resumable!.capture_id,
                            captureMode: homeLedger.resumable!.capture_mode,
                          }),
                        );
                        // The title patch forces a fresh pane even when Sessions
                        // is already active — a bare switch early-returns and the
                        // resume key would sit unadopted until some later remount.
                        switchPrimaryPane("terminal", { title: "Sessions" });
                      },
                    },
                  ]
                : []),
              {
                id: "theme",
                label: "Toggle dark mode",
                run: () => {
                  const current =
                    (localStorage.getItem(THEME_KEY) as ThemePref) ?? "light";
                  const next: ThemePref = current === "dark" ? "light" : "dark";
                  localStorage.setItem(THEME_KEY, next);
                  applyTheme(next);
                },
              },
              {
                id: "extract",
                label: "Extract memories now",
                hint: "runs the local model",
                run: () => {
                  void runExtraction({ startDir: projectRoot }, "command palette").then(
                    (report) => {
                      if (report) void refreshLedger();
                    },
                  );
                },
              },
            ]}
          />
        ) : null}
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
      {backgroundNotices.length > 0 ? (
        <div className="bg-notices" role="status" aria-live="polite">
          {backgroundNotices.map((notice) => (
            <div key={notice.id} className="bg-notice">
              <AlertTriangle size={14} aria-hidden />
              <span>{notice.text}</span>
              <button
                type="button"
                aria-label="Dismiss"
                onClick={() =>
                  setBackgroundNotices((current) => current.filter((n) => n.id !== notice.id))
                }
              >
                <X size={13} />
              </button>
            </div>
          ))}
        </div>
      ) : null}
      </motion.div>
    </LayoutGroup>
  );
}

function Rail(props: {
  activeKind: PaneKind;
  pendingCount: number;
  onOpen: (kind: PaneKind) => void;
  onOpenPalette: () => void;
  reduceMotion: boolean;
}) {
  return (
    <aside className="rail">
      <button className="brand" aria-label="Grafiki home" onClick={() => props.onOpen("home")}>
        <span className="brand-mark">G</span>
        <span className="brand-text">Grafiki</span>
      </button>

      {/* The palette is the app's fastest entry point — without a visible
          signifier it simply doesn't exist for a new user (DESIGN.md §5). */}
      <button
        className="rail-search"
        type="button"
        onClick={props.onOpenPalette}
        title="Search commands or ask your memory (⌘K)"
      >
        <Search size={14} />
        <span>Search…</span>
        <kbd>⌘K</kbd>
      </button>

      <nav className="rail-nav" aria-label="Primary">
        {navItems.map((item) => {
          const Icon = item.icon;
          return (
            <button
              key={item.kind}
              className={`rail-item ${props.activeKind === item.kind ? "active" : ""}`}
              onClick={() => props.onOpen(item.kind)}
              title={item.label}
            >
              <Icon size={16} />
              <span>{item.label}</span>
              {item.kind === "candidates" && props.pendingCount > 0 ? (
                <span className="rail-badge">{props.pendingCount}</span>
              ) : null}
            </button>
          );
        })}
      </nav>
    </aside>
  );
}

function tidyPath(path: string): string {
  return path.replace(/^\/Users\/[^/]+\//, "~/").replace(/^\/home\/[^/]+\//, "~/");
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
    <header className="top-status">
      <div className="project-lockup">
        <Database size={16} />
        <div>
          <strong>{project}</strong>
          <span title={snapshot?.project?.db_path ?? snapshot?.start_dir ?? ""}>
            {tidyPath(snapshot?.project?.db_path ?? snapshot?.start_dir ?? "Waiting for memory")}
          </span>
        </div>
      </div>

      <div className="status-cluster">
        {props.snapshot?.error ? (
          // A failing backend is NOT "not initialized" — telling a healthy
          // project to re-initialize is the classic misdiagnosis (audit H1).
          <StatusPill tone="warn" icon={AlertTriangle} title={props.snapshot.error}>
            Backend error
          </StatusPill>
        ) : (
          <StatusPill tone={memoryAvailable ? "good" : "warn"} icon={memoryAvailable ? CheckCircle2 : AlertTriangle}>
            {memoryAvailable ? "Memory online" : "Initialize needed"}
          </StatusPill>
        )}
        {embedding && embedding.embeddable_records > 0 ? (
          <StatusPill
            tone="accent"
            icon={Sparkles}
            title={`${embedding.fresh_records}/${embedding.embeddable_records} records embedded and searchable`}
          >
            {embedding.fresh_records >= embedding.embeddable_records
              ? "Memory up to date"
              : `${embedding.embeddable_records - embedding.fresh_records} ${embedding.embeddable_records - embedding.fresh_records === 1 ? "memory" : "memories"} indexing`}
          </StatusPill>
        ) : null}
        <button
          className={`icon-button inspector-toggle ${props.inspectorOpen ? "active" : ""}`}
          type="button"
          title={props.inspectorOpen ? "Hide inspector" : "Show inspector"}
          onClick={props.onToggleInspector}
        >
          <PanelRight size={16} />
        </button>
      </div>
    </header>
  );
}

function StatusPill(props: {
  tone: "good" | "warn" | "neutral" | "accent";
  icon: typeof CheckCircle2;
  title?: string;
  children: React.ReactNode;
}) {
  const Icon = props.icon;
  return (
    <span className={`status-pill ${props.tone}`} title={props.title}>
      <Icon size={14} />
      {props.children}
    </span>
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
  ledger: HomeLedgerReport | null;
  onRefreshLedger: () => void;
  onNavigate: (kind: PaneKind, patch?: Partial<PaneState>) => void;
}) {
  const pane = props.pane;

  return (
    <motion.article
      className={`memory-pane ${props.active ? "active" : ""}`}
      onPointerDown={props.onActivate}
      initial={props.reduceMotion ? false : { opacity: 0, y: 6 }}
      animate={{ opacity: 1, y: 0 }}
      exit={props.reduceMotion ? undefined : { opacity: 0 }}
      transition={transition.pane}
    >
      {pane.kind !== "home" ? (
        <header className="pane-header">
          {DRILL_IN_PANE_KINDS.has(pane.kind) ? (
            <button
              className="pane-back"
              onClick={() => (pane.kind === "detail" ? props.onClose() : props.onNavigate("home"))}
              title="Back"
            >
              <ArrowLeft size={15} /> Back
            </button>
          ) : null}
          <div className="pane-heading">
            <h2>{pane.title}</h2>
            {paneSubtitle(pane.kind) ? <p className="pane-purpose">{paneSubtitle(pane.kind)}</p> : null}
          </div>
        </header>
      ) : null}

      <div className={`pane-body ${pane.kind === "terminal" || pane.kind === "chat" ? "fill" : ""}`}>
        {/* Pane-scoped boundary: one view's render crash (e.g. a malformed
            candidate payload) must not take down the rail and a live terminal
            with it — the app-level boundary is the last resort, not the first. */}
        <ErrorBoundary compact>
        {pane.kind === "home" ? (
          <HomePane
            snapshot={props.snapshot}
            projectRoot={props.projectRoot}
            ledger={props.ledger}
            onNavigate={props.onNavigate}
            onRefreshLedger={props.onRefreshLedger}
          />
        ) : null}
        {pane.kind === "session" ? (
          <SessionDetailPane
            captureId={pane.recordId ?? ""}
            projectRoot={props.projectRoot}
            onNavigate={props.onNavigate}
          />
        ) : null}
        {pane.kind === "terminal" ? (
          <SessionsHost
            projectRoot={props.projectRoot}
            fallbackCwd={props.snapshot?.start_dir ?? ""}
            initialLaunch={pane.query}
            handoffPrompt={pane.handoffPrompt}
          />
        ) : null}
        {pane.kind === "chat" ? (
          <ChatPane
            pane={pane}
            snapshot={props.snapshot}
            projectRoot={props.projectRoot}
            onUpdate={props.onUpdate}
            onOpenResult={props.onOpenResult}
            onNavigate={props.onNavigate}
            onMemoryChanged={props.onMemoryChanged}
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
            totalPendingCount={props.ledger?.ledger.pending_candidates ?? 0}
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
        </ErrorBoundary>
      </div>
    </motion.article>
  );
}

/// Minimal, dependency-free rendering for lens bubbles: fenced code blocks
/// become <pre>, inline backticks become <code>. No HTML injection surface —
/// everything stays React text nodes.
function renderLensText(text: string) {
  const segments = text.split(/```[a-zA-Z0-9_-]*\n?/);
  return segments.map((segment, index) =>
    index % 2 === 1 ? (
      <pre key={index} className="lens-code">
        {segment.replace(/\n?$/, "")}
      </pre>
    ) : (
      <span key={index}>
        {segment.split(/(`[^`\n]+`)/).map((piece, pieceIndex) =>
          piece.startsWith("`") && piece.endsWith("`") ? (
            <code key={pieceIndex} className="lens-inline-code">
              {piece.slice(1, -1)}
            </code>
          ) : (
            piece
          ),
        )}
      </span>
    ),
  );
}

/// A wall of raw text (a git-log dump, a pasted file) collapses to a short
/// preview with an expand control — the same calm-reading rule LensBubble
/// applies to agent transcripts, here applied to review candidates so a raw
/// import doesn't render at full, overwhelming length as if it were a
/// finished memory (2026-07-04: "the text is not formatted").
function CollapsibleBody({ text }: { text: string }) {
  const [expanded, setExpanded] = useState(false);
  const lines = text.split("\n");
  const isWall = text.length > 400 || lines.length > 5;
  const preview = lines.slice(0, 3).join("\n").slice(0, 400);
  return (
    <>
      <p className={isWall && !expanded ? "candidate-body-collapsed" : undefined}>
        {expanded || !isWall ? text : `${preview}…`}
      </p>
      {isWall ? (
        <button
          className="link-button"
          type="button"
          onClick={(event) => {
            event.stopPropagation();
            setExpanded((current) => !current);
          }}
        >
          {expanded ? "Show less" : "Show full text"}
        </button>
      ) : null}
    </>
  );
}

/// One conversation bubble. Long tool-output walls collapse to a preview with
/// an expand control — the Granola calm rule applied to agent transcripts.
function LensBubble(props: { role: "user" | "assistant" | "system"; text: string }) {
  const [expanded, setExpanded] = useState(false);
  const lines = props.text.split("\n");
  const isWall = props.text.length > 600 || lines.length > 10;
  const shown = expanded || !isWall ? props.text : lines.slice(0, 6).join("\n");
  return (
    <div className={`lens-bubble ${props.role}`}>
      {renderLensText(shown)}
      {isWall ? (
        <button className="link-button lens-expand" onClick={() => setExpanded(!expanded)}>
          {expanded ? "Show less" : "Show full response"}
        </button>
      ) : null}
    </div>
  );
}

// Claude Code records tool_results and raw bash/tool output under role "user" —
// the SAME role a human's typed message uses — so rendered verbatim, git-push
// output ("To https://github.com/…") and "file updated successfully"
// bookkeeping masquerade as right-aligned "you" bubbles (2026-07-04 audit fix:
// crossed alignment + leaked tool plumbing). Reclassify obvious tool/system
// output to a neutral system line and drop pure file-state plumbing, so only
// genuine human prose stays a "you" bubble. The durable fix is backend (tag
// tool_result turns as role "tool" in get_live_transcript); this is the
// render-layer mitigation.
const LENS_DROP = [
  /has been updated successfully/i,
  /file state is current in your context/i,
  /no need to Read it back/i,
];
function classifyTurn(turn: LiveTranscriptTurn): "user" | "assistant" | "system" | null {
  const text = turn.text.trim();
  if (!text) return null;
  if (LENS_DROP.some((re) => re.test(text))) return null;
  if (turn.role === "assistant") return "assistant";
  // The backend now tags tool output as "tool" (and system/developer turns
  // normalize to "system") — render those as neutral full-width lines. This is
  // the durable fix; the heuristic below is a fallback for anything untagged.
  if (turn.role === "tool" || turn.role === "system") return "system";
  // role === "user": a genuine message OR a tool_result. Route command / tool
  // output to a neutral system line instead of a green "you" bubble.
  const looksLikeToolOutput =
    /^(To |remote:|fatal:|error:|warning:|\d+\s+\/)/m.test(text) ||
    /(->|→)\s*\S+\s*$/m.test(text) ||
    /^[0-9a-f]{7,}\.\.[0-9a-f]{7,}/m.test(text);
  return looksLikeToolOutput ? "system" : "user";
}

type PaletteAction = {
  id: string;
  label: string;
  hint?: string;
  run: () => void;
};

function CommandPalette(props: {
  actions: PaletteAction[];
  onAsk: (question: string) => void;
  onClose: () => void;
}) {
  const [query, setQuery] = useState("");
  const [index, setIndex] = useState(0);
  const dialogRef = useModalDialog<HTMLDivElement>(props.onClose);

  const q = query.trim().toLowerCase();
  const matches = props.actions.filter((action) => action.label.toLowerCase().includes(q));
  // Anything that matches no action becomes a memory question — the palette IS
  // the ask bar when you type a sentence.
  const askRow = query.trim().length > 0;
  const total = matches.length + (askRow ? 1 : 0);
  const clamped = Math.min(index, Math.max(0, total - 1));

  const execute = (position: number) => {
    if (position < matches.length) {
      matches[position].run();
    } else if (askRow) {
      props.onAsk(query.trim());
    }
    props.onClose();
  };

  return (
    <motion.div
      className="overlay palette-overlay"
      role="presentation"
      onMouseDown={props.onClose}
      initial={{ opacity: 0 }}
      animate={{ opacity: 1 }}
      transition={transition.quick}
    >
      <motion.div
        ref={dialogRef}
        className="palette"
        role="dialog"
        aria-modal="true"
        aria-label="Command palette"
        tabIndex={-1}
        onMouseDown={(event) => event.stopPropagation()}
        initial={{ opacity: 0, y: -8 }}
        animate={{ opacity: 1, y: 0 }}
        transition={transition.quick}
      >
        <input
          autoFocus
          aria-label="Command or question"
          role="combobox"
          aria-autocomplete="list"
          aria-expanded="true"
          aria-controls="command-palette-options"
          aria-activedescendant={total > 0 ? `command-option-${clamped}` : undefined}
          value={query}
          placeholder="Type a command, or ask your memory…"
          onChange={(event) => {
            setQuery(event.target.value);
            setIndex(0);
          }}
          onKeyDown={(event) => {
            if (event.key === "ArrowDown") {
              event.preventDefault();
              setIndex(Math.min(total - 1, clamped + 1));
            } else if (event.key === "ArrowUp") {
              event.preventDefault();
              setIndex(Math.max(0, clamped - 1));
            } else if (event.key === "Enter") {
              event.preventDefault();
              if (total > 0) execute(clamped);
            } else if (event.key === "Escape") {
              props.onClose();
            }
          }}
        />
        <div className="palette-list" id="command-palette-options" role="listbox">
          {matches.map((action, position) => (
            <div
              key={action.id}
              id={`command-option-${position}`}
              role="option"
              aria-selected={position === clamped}
              className={`palette-row ${position === clamped ? "active" : ""}`}
              onMouseEnter={() => setIndex(position)}
              onClick={() => execute(position)}
            >
              <span>{action.label}</span>
              {action.hint ? <span className="palette-hint">{action.hint}</span> : null}
            </div>
          ))}
          {askRow ? (
            <div
              id={`command-option-${matches.length}`}
              role="option"
              aria-selected={clamped === matches.length}
              className={`palette-row ${clamped === matches.length ? "active" : ""}`}
              onMouseEnter={() => setIndex(matches.length)}
              onClick={() => execute(matches.length)}
            >
              <span>
                Ask memory: <em>“{query.trim()}”</em>
              </span>
              <span className="palette-hint">↵</span>
            </div>
          ) : null}
          {total === 0 ? <div className="palette-empty">Nothing matches.</div> : null}
        </div>
      </motion.div>
    </motion.div>
  );
}

function SessionDetailPane(props: {
  captureId: string;
  projectRoot: string;
  onNavigate: (kind: PaneKind, patch?: Partial<PaneState>) => void;
}) {
  const [detail, setDetail] = useState<SessionDetailReport | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [tab, setTab] = useState<"memories" | "events">("memories");

  useEffect(() => {
    let cancelled = false;
    setDetail(null);
    setError(null);
    getSessionDetail({ startDir: props.projectRoot, captureId: props.captureId })
      .then((next) => {
        if (!cancelled) setDetail(next);
      })
      .catch((detailError) => {
        if (!cancelled) setError(String(detailError));
      });
    return () => {
      cancelled = true;
    };
  }, [props.captureId, props.projectRoot]);

  if (error) {
    return <p style={{ color: "var(--danger)" }}>{error}</p>;
  }
  if (!detail) {
    return <p className="muted">Opening session…</p>;
  }
  const session = detail.session;

  return (
    <div className="view-stack">
      <div className="session-head">
        <div className="ledger-icon">{agentGlyph(session.source_app)}</div>
        <div>
          <b>
            {agentLabel(session.source_app)} · {ledgerDayLabel(session.started_at)} ·{" "}
            {ledgerTimeLabel(session.started_at)}
            {session.ended_at ? `–${ledgerTimeLabel(session.ended_at)}` : " · in progress"}
          </b>
          <span className="subtle">
            {session.event_count} captured events · {session.memory_count}{" "}
            {session.memory_count === 1 ? "memory" : "memories"}
          </span>
        </div>
      </div>

      <div className="seg-tabs" role="tablist" aria-label="Session detail" onKeyDown={handleTablistKeyDown}>
        <button role="tab" aria-selected={tab === "memories"} className={`seg-tab ${tab === "memories" ? "active" : ""}`} onClick={() => setTab("memories")}>
          Memories ({detail.memories.length})
        </button>
        <button role="tab" aria-selected={tab === "events"} className={`seg-tab ${tab === "events" ? "active" : ""}`} onClick={() => setTab("events")}>
          Raw events ({detail.events.length})
        </button>
      </div>

      {tab === "memories" ? (
        detail.memories.length === 0 ? (
          <div className="empty-record-list">
            Nothing was extracted from this session{session.ended_at ? "" : " yet"}.
          </div>
        ) : (
          <div className="dense-list">
            {detail.memories.map((memory) => (
              <div key={memory.id} className="data-row">
                <span className="record-type">{memory.record_type}</span>
                <b style={{ fontWeight: 550 }}>{candidateTitle(memory)}</b>
                <span className="subtle" style={{ marginLeft: "auto" }}>
                  {memory.status}
                  {memory.status === "pending" ? " · review it" : ""}
                </span>
                {memory.status === "pending" ? (
                  <button className="link-button" onClick={() => props.onNavigate("candidates")}>
                    Review
                  </button>
                ) : null}
              </div>
            ))}
          </div>
        )
      ) : (
        <div className="dense-list">
          {detail.events.map((event) => (
            <div key={event.id} className="event-row">
              <div className="event-row-head">
                <span className="record-type">{event.source_type}</span>
                <span className="subtle">{event.title ?? ""}</span>
                <span className="subtle" style={{ marginLeft: "auto" }}>
                  {ledgerTimeLabel(event.captured_at)}
                </span>
              </div>
              {event.text ? <pre className="event-text">{event.text}</pre> : null}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

/// "Today" / "Yesterday" / "Mon, Jun 29" — Granola-style day group labels.
function ledgerDayLabel(iso: string): string {
  const date = new Date(iso);
  const today = new Date();
  const yesterday = new Date(today);
  yesterday.setDate(today.getDate() - 1);
  const sameDay = (a: Date, b: Date) =>
    a.getFullYear() === b.getFullYear() &&
    a.getMonth() === b.getMonth() &&
    a.getDate() === b.getDate();
  if (sameDay(date, today)) return "Today";
  if (sameDay(date, yesterday)) return "Yesterday";
  return date.toLocaleDateString(undefined, { weekday: "short", month: "short", day: "numeric" });
}

function ledgerTimeLabel(iso: string): string {
  return new Date(iso).toLocaleTimeString(undefined, { hour: "numeric", minute: "2-digit" });
}

function agentGlyph(sourceApp: string | null | undefined): string {
  const name = (sourceApp || "").toLowerCase();
  if (name.includes("claude")) return "C";
  if (name.includes("codex")) return "X";
  if (name.includes("gemini")) return "G";
  return "%";
}

function agentLabel(sourceApp: string | null | undefined): string {
  const name = (sourceApp || "").toLowerCase();
  if (name === "grafiki-terminal") return "session";
  if (name.includes("claude")) return "claude";
  if (name.includes("codex")) return "codex";
  return sourceApp || "session";
}

/// Serif stat numbers count up on arrival — the one Wispr-ism the stats keep.
function useCountUp(target: number | undefined): number | undefined {
  const [value, setValue] = useState(0);
  useEffect(() => {
    if (target === undefined) return;
    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) {
      setValue(target);
      return;
    }
    const start = performance.now();
    const duration = 450;
    let frame = 0;
    const tick = (now: number) => {
      const progress = Math.min(1, (now - start) / duration);
      setValue(Math.round(target * (1 - Math.pow(1 - progress, 3))));
      if (progress < 1) frame = requestAnimationFrame(tick);
    };
    frame = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(frame);
  }, [target]);
  return target === undefined ? undefined : value;
}

function HomePane(props: {
  snapshot: ProjectSnapshot | null;
  projectRoot: string;
  ledger: HomeLedgerReport | null;
  onNavigate: (kind: PaneKind, patch?: Partial<PaneState>) => void;
  onRefreshLedger: () => void;
}) {
  const [ask, setAsk] = useState("");
  const ledger = props.ledger?.ledger;
  const live = props.ledger?.live?.[0] ?? null;
  const resumable = props.ledger?.resumable ?? null;
  const sessions = ledger?.sessions ?? [];
  const projectLabel =
    props.projectRoot || props.snapshot?.start_dir || "no project folder set";
  // True first-run = no live/resumable session, no sessions this week, no
  // memories, nothing pending. The marketing hero is reserved for THAT only —
  // a returning user with real state should see activity, not the pitch.
  const hasAnyState =
    !!live ||
    !!resumable ||
    sessions.length > 0 ||
    (ledger?.memories_week ?? 0) > 0 ||
    (ledger?.pending_candidates ?? 0) > 0;

  // The truthful capture→extraction chain, first broken link named. Three
  // independent gates (init, consent, local model) previously failed silently
  // with the identical symptom — an empty Home — leaving a working user
  // convinced the app was broken (2026-07-04 don-norman-design-critic, H1).
  const [pipelineIssue, setPipelineIssue] = useState<string | null>(null);
  // Set when the issue is fixable with one click (capture off) — the hint
  // then renders a "Turn on capture" action instead of a settings scavenger hunt.
  const [pipelineFix, setPipelineFix] = useState<"enable-capture" | null>(null);
  const [pipelineFixBusy, setPipelineFixBusy] = useState(false);
  const [pipelineCheck, setPipelineCheck] = useState(0);
  useEffect(() => {
    let cancelled = false;
    setPipelineFix(null);
    if (!props.snapshot?.memory_available) {
      // A failing backend is not "not initialized" — don't misdiagnose (audit H1).
      setPipelineIssue(
        props.snapshot?.error
          ? `Grafiki backend error — memory is temporarily unavailable: ${props.snapshot.error}`
          : "This folder isn't initialized — set it up in Settings to start remembering.",
      );
      return;
    }
    getCaptureConfig({ startDir: props.projectRoot })
      .then((config) => {
        if (cancelled) return;
        if (!config.config.sources.terminal || config.config.terminal_output === "off") {
          setPipelineIssue("Terminal capture is off, so sessions aren't being remembered.");
          setPipelineFix("enable-capture");
          return;
        }
        return listLocalModels().then((models) => {
          if (cancelled) return;
          setPipelineIssue(
            models.length === 0
              ? "No local model installed — run `ollama pull gemma3:1b` to turn sessions into memory."
              : null,
          );
        });
      })
      .catch((checkError) => {
        // The one surface built to explain silent failure must not itself
        // fail silently (it used to hide the banner here).
        if (!cancelled) setPipelineIssue(`Couldn't check the capture pipeline: ${String(checkError)}`);
      });
    return () => {
      cancelled = true;
    };
  }, [props.projectRoot, props.snapshot?.memory_available, pipelineCheck]);

  const enableCaptureNow = async () => {
    // Re-disclose before enabling full-output capture — the same consent the
    // onboarding checkbox obtains. A one-click "Turn on capture" that silently
    // starts storing terminal output would bypass the initial disclosure.
    // (confirmDialog = the Tauri dialog plugin: window.confirm can be
    // suppressed inside the packaged webview.)
    const consented = await confirmDialog(CAPTURE_ENABLE_DISCLOSURE, {
      title: "Turn on terminal capture?",
      kind: "info",
      okLabel: "Turn on capture",
    });
    if (!consented) return;
    setPipelineFixBusy(true);
    updateCaptureConfig({ startDir: props.projectRoot, terminal: true, terminalOutput: "full" })
      .then(() => {
        setPipelineCheck((n) => n + 1);
        props.onRefreshLedger();
      })
      .catch((enableError) => notifyBackground(`Couldn't turn on capture: ${String(enableError)}`))
      .finally(() => setPipelineFixBusy(false));
  };

  const submitAsk = () => {
    const question = ask.trim();
    if (!question) return;
    // The full question still travels as `query`; only the pane TITLE is
    // clamped — an unbounded title collapsed the chat layout to zero height.
    props.onNavigate("chat", { query: question, title: `Memory: ${clampTitle(question)}` });
  };

  const resume = () => {
    if (!resumable) return;
    // Point the terminal pane at the resumable session, then open it — the
    // pane's attach-miss path revives from the on-disk descriptor.
    localStorage.setItem(
      terminalStorageKey(props.projectRoot),
      JSON.stringify({
        id: resumable.id,
        launch: resumable.launch,
        captureId: resumable.capture_id,
        captureMode: resumable.capture_mode,
      }),
    );
    props.onNavigate("terminal");
  };

  // Grouped by day label, preserving the newest-first order.
  const groups: Array<{ day: string; items: typeof sessions }> = [];
  for (const session of sessions) {
    const day = ledgerDayLabel(session.started_at);
    const group = groups[groups.length - 1];
    if (group && group.day === day) group.items.push(session);
    else groups.push({ day, items: [session] });
  }
  // "Today" only when a real today-group exists — otherwise it's a mapping
  // falsehood on the hero element (2026-07-04 don-norman-design-critic).
  const hasToday = groups.some((group) => group.day === "Today");

  return (
    <div className="home-view">
      <h1 className="home-title">{hasToday ? "Today" : "Home"}</h1>
        <p className="home-meta" title={projectLabel}>
          {tidyPath(projectLabel)}
          {props.snapshot?.memory_available ? "" : " · initialize in Settings"}
        </p>
        {pipelineIssue ? (
          <p className="home-pipeline-hint">
            {pipelineIssue}
            {pipelineFix === "enable-capture" ? (
              <>
                {" "}
                <button
                  className="link-button"
                  type="button"
                  disabled={pipelineFixBusy}
                  onClick={() => void enableCaptureNow()}
                >
                  {pipelineFixBusy ? "Turning on…" : "Turn on capture"}
                </button>
              </>
            ) : null}
          </p>
        ) : null}

        <div className="stat-strip">
          <div className={`stat-card ${(ledger?.sessions_week ?? 0) === 0 ? "stat-card--muted" : ""}`}>
            <b>{useCountUp(ledger?.sessions_week) ?? "–"}</b>
            <span>sessions this week</span>
          </div>
          <div className={`stat-card ${(ledger?.memories_week ?? 0) === 0 ? "stat-card--muted" : ""}`}>
            <b>{useCountUp(ledger?.memories_week) ?? "–"}</b>
            <span>memories this week</span>
          </div>
          <div
            className={`stat-card ${(ledger?.pending_candidates ?? 0) > 0 ? "stat-card--action" : "stat-card--muted"}`}
            role={(ledger?.pending_candidates ?? 0) > 0 ? "button" : undefined}
            tabIndex={(ledger?.pending_candidates ?? 0) > 0 ? 0 : undefined}
            onClick={(ledger?.pending_candidates ?? 0) > 0 ? () => props.onNavigate("candidates") : undefined}
            onKeyDown={(event) => {
              if ((ledger?.pending_candidates ?? 0) > 0 && (event.key === "Enter" || event.key === " ")) {
                event.preventDefault();
                props.onNavigate("candidates");
              }
            }}
          >
            <b>{useCountUp(ledger?.pending_candidates) ?? "–"}</b>
            <span>waiting for review</span>
            {(ledger?.pending_candidates ?? 0) > 0 ? <span className="stat-go">Review →</span> : null}
          </div>
        </div>

        {live ? (
          <motion.div
            className="live-card"
            initial={{ opacity: 0, y: -6 }}
            animate={{ opacity: 1, y: 0 }}
            transition={transition.quick}
          >
            <div className="live-avatar">{agentGlyph(live.launch)}</div>
            <div className="live-body">
              <div className="live-row1">
                <b>{live.launch ? agentLabel(live.launch) : "shell"}</b>
                {live.capturing ? (
                  <span className="chip chip-live">
                    <span className="pulse-dot" /> Capturing
                  </span>
                ) : (
                  <span className="chip chip-warn" title={live.capture_hint ?? "check Settings"}>
                    Not capturing
                  </span>
                )}
                <span className="live-cwd" title={live.cwd}>
                  {tidyPath(live.cwd)}
                </span>
              </div>
              <div className="live-hint">
                {live.capturing
                  ? "Capturing this session into memory"
                  : (live.capture_hint ?? "Turn capture on in Settings → Capture Consent")}
              </div>
            </div>
            <button className="button primary live-open" onClick={() => props.onNavigate("terminal")}>
              Open →
            </button>
          </motion.div>
        ) : resumable ? (
          <motion.div
            className="suggest-banner"
            initial={{ opacity: 0, y: -6 }}
            animate={{ opacity: 1, y: 0 }}
            transition={transition.quick}
          >
            <span className="spark">↻</span>
            Pick up where you left off — {resumable.launch || "shell"} in {resumable.cwd}
            <span className="banner-action">
              <button className="button primary" onClick={resume}>
                Resume session
              </button>
            </span>
          </motion.div>
        ) : null}

        {(ledger?.pending_candidates ?? 0) > 0 ? (
          <motion.div
            className="suggest-banner"
            initial={{ opacity: 0, y: -6 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ ...transition.quick, delay: 0.05 }}
          >
            <span className="spark">✦</span>
            {ledger?.pending_candidates} new{" "}
            {ledger?.pending_candidates === 1 ? "memory" : "memories"} from your sessions
            {ledger?.pending_titles?.length ? ` — “${ledger.pending_titles[0]}”` : ""}
            <span className="banner-action">
              <button className="button primary" onClick={() => props.onNavigate("candidates")}>
                Review
              </button>
            </span>
          </motion.div>
        ) : null}

        {!hasAnyState ? (
          <div className="home-empty">
            <h3>
              Your agent forgets every session. Grafiki <em>remembers</em>.
            </h3>
            <p className="muted" style={{ maxWidth: 420 }}>
              Start your first session — work normally, and everything worth keeping comes back
              here as memory.
            </p>
            {pipelineIssue ? (
              <p className="home-pipeline-hint" style={{ maxWidth: 420 }}>
                {pipelineIssue}
              </p>
            ) : null}
            <div className="agent-buttons">
              <button className="button primary" onClick={() => props.onNavigate("terminal")}>
                Start a session
              </button>
            </div>
          </div>
        ) : sessions.length > 0 ? (
          groups.map((group) => (
            <div key={group.day}>
              <div className="ledger-day">{group.day}</div>
              {group.items.map((session) => (
                <button
                  type="button"
                  key={session.id}
                  className="ledger-row"
                  onClick={() =>
                    props.onNavigate("session", {
                      recordId: session.id,
                      title: `Session · ${ledgerDayLabel(session.started_at)}`,
                    })
                  }
                >
                  <div className="ledger-icon">{agentGlyph(session.source_app)}</div>
                  <div className="ledger-body">
                    <b>
                      {agentLabel(session.source_app)}
                      {session.status === "active" ? " · in progress" : ""}
                    </b>
                    {session.recent_memory_titles.length > 0 ? (
                      <span>
                        {session.recent_memory_titles.map((title, index) => (
                          <span key={title}>
                            {index > 0 ? " · " : ""}
                            &ldquo;{title}&rdquo;
                          </span>
                        ))}
                      </span>
                    ) : (
                      <span>{session.event_count} captured events</span>
                    )}
                  </div>
                  {session.memory_count > 0 ? (
                    <span className="ledger-mem">
                      {session.memory_count} {session.memory_count === 1 ? "memory" : "memories"}
                    </span>
                  ) : null}
                  <span className="ledger-time">{ledgerTimeLabel(session.started_at)}</span>
                </button>
              ))}
            </div>
          ))
        ) : (
          <div className="home-quiet">
            <p>No sessions yet this week — your recent activity will show up here.</p>
            <button className="button primary" onClick={() => props.onNavigate("terminal")}>
              Start a session
            </button>
          </div>
        )}

        <div className="ask-bar-wrap">
        <div className="search-box">
          <Sparkles size={15} />
          <input
            aria-label="Ask your memory"
            value={ask}
            onChange={(event) => setAsk(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter") {
                event.preventDefault();
                submitAsk();
              }
            }}
            placeholder="Ask your memory…"
          />
          <button onClick={submitAsk}>Ask</button>
        </div>
      </div>
    </div>
  );
}

type TerminalSessionRef = {
  id: string;
  launch: string;
  captureId?: string | null;
  captureMode?: "off" | "digest" | "full";
};

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

// Per-project LIST of sessions (the tab strip). The backend PTY pool already
// holds many concurrent detached sessions; this is the UI's record of which
// ones belong to this project and their launch command.
function sessionListKey(projectRoot: string) {
  return `grafiki-terminal-list:${projectRoot}`;
}

function loadSessionList(projectRoot: string): TerminalSessionRef[] {
  try {
    const raw = localStorage.getItem(sessionListKey(projectRoot));
    if (raw) {
      const parsed = JSON.parse(raw) as TerminalSessionRef[];
      if (Array.isArray(parsed)) {
        return parsed.filter((ref) => typeof ref?.id === "string");
      }
    }
  } catch {
    /* no valid list yet */
  }
  return [];
}

function saveSessionList(projectRoot: string, list: TerminalSessionRef[]) {
  try {
    localStorage.setItem(sessionListKey(projectRoot), JSON.stringify(list));
  } catch {
    /* best-effort persistence */
  }
}

// Tab label from the launch command.
function sessionTabTitle(launch: string): string {
  const name = (launch || "").toLowerCase();
  if (name.includes("claude")) return "Claude";
  if (name.includes("codex")) return "Codex";
  if (name.includes("gemini")) return "Gemini";
  return "Shell";
}

// The Sessions screen: a per-project tab strip over the detached-PTY pool.
// The pool already keeps every session's agent alive when you switch away
// (detach, not kill) and replays scrollback on return — so this is a thin
// list/switcher on top: it owns which sessions belong to the project and which
// tab is active, and mounts ONE controlled TerminalPane for the active one.
function SessionsHost(props: {
  projectRoot: string;
  fallbackCwd: string;
  initialLaunch?: string;
  handoffPrompt?: string;
}) {
  const [list, setList] = useState<TerminalSessionRef[]>(() => loadSessionList(props.projectRoot));
  const [activeId, setActiveId] = useState<string | null>(() => {
    // An explicit launch request must start in launcher/spawn mode on the VERY
    // first render — TerminalPane's spawn effect runs once on mount, so if the
    // last tab is adopted here first, the launch (and its handoff prompt) is
    // consumed as a no-op before the mount effect below can correct it.
    if (props.initialLaunch !== undefined && !loadTerminalSession(props.projectRoot)) {
      return null;
    }
    const l = loadSessionList(props.projectRoot);
    return l.length ? l[l.length - 1].id : null;
  });

  // On mount / project change: load the list and adopt any session handed in
  // via the legacy single-session key (a pre-tab-strip session, or Home's
  // "Resume"/"Open" which writes that key), then clear it so it's adopted once.
  useEffect(() => {
    let next = loadSessionList(props.projectRoot);
    const legacy = loadTerminalSession(props.projectRoot);
    let active: string | null = next.length ? next[next.length - 1].id : null;
    if (legacy) {
      localStorage.removeItem(terminalStorageKey(props.projectRoot));
      if (!next.some((s) => s.id === legacy.id)) next = [...next, legacy];
      active = legacy.id;
    }
    if (props.initialLaunch !== undefined && !legacy) {
      // An explicit launch request (palette "Start …", Memory's "Continue this
      // with Claude", onboarding) must open a NEW session — adopting the last
      // tab here silently discarded the launch and its handoff prompt.
      active = null;
    }
    setList(next);
    setActiveId(active);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [props.projectRoot]);

  // Persist the list whenever it changes.
  useEffect(() => {
    saveSessionList(props.projectRoot, list);
  }, [props.projectRoot, list]);

  const active = list.find((s) => s.id === activeId) ?? null;

  const handleStarted = (ref: TerminalSessionRef) => {
    setList((prev) => (prev.some((s) => s.id === ref.id) ? prev : [...prev, ref]));
    setActiveId(ref.id);
  };
  const handleUpdated = (ref: TerminalSessionRef) => {
    setList((previous) => previous.map((item) => (item.id === ref.id ? ref : item)));
  };
  const dropSession = (id: string) => {
    // State updaters must stay pure (StrictMode runs them twice) — compute the
    // next active id from current state instead of setting state inside setList.
    setList((prev) => prev.filter((s) => s.id !== id));
    setActiveId((cur) => {
      if (cur !== id) return cur;
      const remaining = list.filter((s) => s.id !== id);
      return remaining.length ? remaining[remaining.length - 1].id : null;
    });
  };
  // Closing a tab ends that session's process (only "End session"/close kills;
  // a plain tab switch keeps it running).
  const closeTab = (id: string) => {
    // The tab is dropped either way (the common failure is "already exited"),
    // but a failed close means the PTY may still be running — say so.
    invoke("terminal_close", { id }).catch((closeError) =>
      notifyBackground(`Couldn't end the session cleanly: ${String(closeError)}`),
    );
    sessionStorage.removeItem(`grafiki-terminal-launched:${id}`);
    dropSession(id);
  };

  return (
    <div className="sessions-host">
      {list.length > 0 || active !== null ? (
        <div className="session-tabs" role="tablist">
          {list.map((s) => (
            <div
              key={s.id}
              className={`session-tab-wrap ${s.id === activeId ? "active" : ""}`}
            >
              <button
                type="button"
                id={`session-tab-${s.id}`}
                role="tab"
                tabIndex={s.id === activeId ? 0 : -1}
                aria-selected={s.id === activeId}
                aria-controls="active-session-panel"
                className="session-tab"
                onClick={() => setActiveId(s.id)}
                onKeyDown={(event) => {
                  const currentIndex = list.findIndex((item) => item.id === s.id);
                  const delta = event.key === "ArrowRight" ? 1 : event.key === "ArrowLeft" ? -1 : 0;
                  if (!delta) return;
                  event.preventDefault();
                  const next = list[(currentIndex + delta + list.length) % list.length];
                  setActiveId(next.id);
                  requestAnimationFrame(() => document.getElementById(`session-tab-${next.id}`)?.focus());
                }}
              >
                <span className="session-tab-glyph">{agentGlyph(s.launch)}</span>
                <span className="session-tab-label">{sessionTabTitle(s.launch)}</span>
              </button>
              <button
                type="button"
                className="session-tab-close"
                aria-label={`End ${sessionTabTitle(s.launch)} session`}
                title="End this session"
                onClick={(event) => {
                  event.stopPropagation();
                  closeTab(s.id);
                }}
              >
                <X size={12} />
              </button>
            </div>
          ))}
          <button className="session-tab-new" onClick={() => setActiveId(null)} title="New session">
            <Plus size={15} />
          </button>
        </div>
      ) : null}
      <div
        className="sessions-host-body"
        id="active-session-panel"
        role="tabpanel"
        aria-labelledby={activeId ? `session-tab-${activeId}` : undefined}
      >
        <TerminalPane
          projectRoot={props.projectRoot}
          fallbackCwd={props.fallbackCwd}
          initialLaunch={active === null ? props.initialLaunch : undefined}
          handoffPrompt={props.handoffPrompt}
          sessionRef={active}
          onStarted={handleStarted}
          onUpdated={handleUpdated}
          onEnded={dropSession}
        />
      </div>
    </div>
  );
}

function TerminalPane(props: {
  projectRoot: string;
  fallbackCwd: string;
  initialLaunch?: string;
  handoffPrompt?: string;
  // When SessionsHost drives the tab strip it passes the active session here
  // (`controlled` mode) and owns the per-project list; startSession/endSession
  // report up via onStarted/onEnded instead of writing the single-session key.
  // Omit these props entirely for the standalone (uncontrolled) behavior.
  sessionRef?: TerminalSessionRef | null;
  onStarted?: (ref: TerminalSessionRef) => void;
  onUpdated?: (ref: TerminalSessionRef) => void;
  onEnded?: (id: string) => void;
}) {
  const controlled = props.sessionRef !== undefined;
  // The session id is STABLE and persisted per project: switching tabs detaches
  // the UI but the PTY (and the agent inside it) keeps running; coming back
  // reattaches and replays scrollback. Only "End session" kills the process.
  const [session, setSession] = useState<TerminalSessionRef | null>(() =>
    controlled ? props.sessionRef ?? null : loadTerminalSession(props.projectRoot),
  );
  const [error, setError] = useState<string | null>(null);
  const [ended, setEnded] = useState(false);
  // null = connecting; then the backend's honest answer (false = the folder
  // isn't an initialized Grafiki project, so nothing is being recorded).
  const [capturing, setCapturing] = useState<boolean | null>(null);
  const [captureHint, setCaptureHint] = useState<string | null>(null);
  const [captureMode, setCaptureMode] = useState<"off" | "digest" | "full">(
    props.sessionRef?.captureMode ?? "off",
  );
  // Pre-launch consent check, so the launcher screen can tell the truth about
  // whether this session will be captured instead of asserting it always is
  // (2026-07-04 don-norman-design-critic: this copy was unconditionally false
  // whenever consent was off or the folder wasn't initialized).
  const [launcherCaptureConfig, setLauncherCaptureConfig] = useState<CaptureConfigReport | null>(
    null,
  );
  const containerRef = useRef<HTMLDivElement | null>(null);
  // Set when the launcher just created `session`, so the effect spawns instead
  // of attaching. A ref (not state): StrictMode remounts must attach, not respawn.
  const spawnRef = useRef(false);
  const captureIdRef = useRef<string | null | undefined>(session?.captureId);

  // Controlled mode: when the host switches the active tab, adopt that session.
  // The big attach/detach effect below then detaches the old PTY (keeping it
  // alive in the pool) and attaches the new one — exactly like tab-away/back.
  useEffect(() => {
    if (!controlled) return;
    const nextId = props.sessionRef?.id ?? null;
    if (nextId !== (session?.id ?? null)) {
      spawnRef.current = false;
      setSession(props.sessionRef ?? null);
      setEnded(false);
      setError(null);
      setCapturing(null);
      captureIdRef.current = props.sessionRef?.captureId;
      setCaptureMode(props.sessionRef?.captureMode ?? "off");
      setLens("terminal"); // lens is per-session; a shell tab has no chat lens
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [props.sessionRef?.id]);

  useEffect(() => {
    // Uncontrolled only — in controlled mode the host owns which session is
    // active and re-passes sessionRef on project change.
    if (controlled) return;
    setSession(loadTerminalSession(props.projectRoot));
    setError(null);
    setEnded(false);
    getCaptureConfig({ startDir: props.projectRoot || props.fallbackCwd })
      .then(setLauncherCaptureConfig)
      .catch(() => setLauncherCaptureConfig(null));
  }, [props.projectRoot, props.fallbackCwd]);

  // Load the launcher's capture-consent copy (both modes need it for the
  // "will this be captured?" line on the Start-a-session screen).
  useEffect(() => {
    getCaptureConfig({ startDir: props.projectRoot || props.fallbackCwd })
      .then(setLauncherCaptureConfig)
      .catch(() => setLauncherCaptureConfig(null));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [props.projectRoot]);

  // Onboarding (or Home) can hand us an agent to launch immediately.
  useEffect(() => {
    if (props.initialLaunch !== undefined && session === null) {
      startSession(props.initialLaunch);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // "Learned this session" side peek: pending LLM-extracted candidates that
  // appeared since this pane opened, refreshed on the extraction cadence.
  const [peek, setPeek] = useState<ExtractionCandidate[]>([]);
  const [peekBusy, setPeekBusy] = useState<string | null>(null);
  const [peekOpen, setPeekOpen] = useState(true);
  const loadPeek = async () => {
    // Pin the session this call was made for: if the pane switches sessions
    // while listCandidates is in flight, a slow response for the OLD session
    // must not be applied over the NEW session's (or emptied) peek.
    const captureId = captureIdRef.current;
    try {
      const candidates = await listCandidates({
        startDir: props.projectRoot,
        scope: "",
        status: "pending",
        limit: 50,
        captureId: captureId ?? undefined,
      });
      if (captureId !== captureIdRef.current) return;
      setPeek(
        candidates.filter(
          (candidate) =>
            candidate.source_type === "capture:llm" &&
            Boolean(captureId) &&
            candidate.source === captureId,
        ),
      );
    } catch {
      /* the peek is best-effort; the terminal must never suffer for it */
    }
  };
  useEffect(() => {
    if (!session) {
      return;
    }
    void loadPeek();
    const timer = window.setInterval(() => void loadPeek(), 45_000);
    return () => window.clearInterval(timer);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [session?.id]);
  const peekNew = peek;
  const peekOlder = 0;

  // The chat LENS: the same session rendered as a conversation (Claude Code
  // only — we tail its transcript with the parser capture already uses).
  const [lens, setLens] = useState<"terminal" | "chat">("terminal");
  const [turns, setTurns] = useState<LiveTranscriptTurn[]>([]);
  const [transcriptError, setTranscriptError] = useState<string | null>(null);
  const [composer, setComposer] = useState("");
  const chatScrollRef = useRef<HTMLDivElement | null>(null);
  const chatCapable = session?.launch === "claude";
  // Filter tool-plumbing noise and reclassify Claude Code's tool_result turns
  // (which it files under role "user") before rendering the chat lens — so
  // terminal/git output stops masquerading as right-aligned "you" bubbles.
  const visibleTurns = useMemo(
    () =>
      turns
        .map((turn) => ({ turn, display: classifyTurn(turn) }))
        .filter(
          (item): item is { turn: LiveTranscriptTurn; display: "user" | "assistant" | "system" } =>
            item.display !== null,
        ),
    [turns],
  );
  // Raw terminal tail (decoded, unbounded ANSI-included) so the chat lens can
  // detect a permission prompt the JSONL transcript never records — otherwise
  // the last rendered bubble looks like a normal finished turn while the agent
  // is actually stuck waiting on a decision (2026-07-04 don-norman-design-critic
  // bonus finding: the session "appears hung").
  const termTailRef = useRef("");
  const [pendingPrompt, setPendingPrompt] = useState(false);
  useEffect(() => {
    setTurns([]);
    setTranscriptError(null);
    termTailRef.current = "";
    setPendingPrompt(false);
    setPeek([]);
  }, [session?.id]);
  useEffect(() => {
    if (!session || lens !== "chat") {
      setPendingPrompt(false);
      return;
    }
    let cancelled = false;
    const load = () =>
      getLiveTranscript({
        startDir: props.projectRoot || props.fallbackCwd,
        terminalId: session.id,
      })
        .then((next) => {
          if (!cancelled) {
            // Keep the previous array when nothing changed — a fresh array
            // every 3s re-renders the entire bubble list of a long session.
            setTurns((previous) =>
              previous.length === next.length &&
              JSON.stringify(previous[previous.length - 1] ?? null) ===
                JSON.stringify(next[next.length - 1] ?? null)
                ? previous
                : next,
            );
            setTranscriptError(null);
          }
        })
        .catch((transcriptLoadError) => {
          if (!cancelled) {
            setTranscriptError(`Chat view unavailable: ${String(transcriptLoadError)}`);
          }
        });
    void load();
    const timer = window.setInterval(() => {
      void load();
      const tail = termTailRef.current.toLowerCase();
      setPendingPrompt(PERMISSION_PROMPT_MARKERS.some((marker) => tail.includes(marker)));
    }, 3000);
    return () => {
      cancelled = true;
      window.clearInterval(timer);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [lens, session?.id]);
  useEffect(() => {
    // Scroll ONLY the bubble list — scrollIntoView would drag every scrollable
    // ancestor (including the window root) and shove the chrome off-screen.
    const scroller = chatScrollRef.current;
    if (scroller) {
      scroller.scrollTop = scroller.scrollHeight;
    }
  }, [turns.length]);
  const sendComposer = async () => {
    const text = composer.trim();
    if (!text || !session || ended) return;
    try {
      await invoke("terminal_write", { id: session.id, data: `${text}\r` });
      setComposer("");
    } catch (writeError) {
      setError(`Message was not sent: ${String(writeError)}`);
    }
  };

  const reviewPeek = async (candidate: ExtractionCandidate, accept: boolean) => {
    setPeekBusy(candidate.id);
    try {
      if (accept) {
        await approveCandidate({ startDir: props.projectRoot, id: candidate.id });
      } else {
        await rejectCandidate({ startDir: props.projectRoot, id: candidate.id, rationale: "" });
      }
      await loadPeek();
    } catch (peekError) {
      // It was NOT surfaced anywhere before — the tick just un-spun and the
      // candidate stayed put while the user believed it was approved.
      notifyBackground(
        `Couldn't ${accept ? "approve" : "dismiss"} "${candidateTitle(candidate)}": ${String(peekError)}`,
      );
    } finally {
      setPeekBusy(null);
    }
  };

  useEffect(() => {
    if (!session || !containerRef.current) {
      return;
    }
    if (isPreviewMode()) {
      // No PTY bridge in browser preview — `new Channel()` (and every
      // terminal_* invoke) needs Tauri. Show a notice instead of a blank crash.
      setError("The hosted terminal needs the desktop app — this is a browser preview.");
      return;
    }
    let disposed = false;
    let cleanup: (() => void) | undefined;
    void Promise.all([import("@xterm/xterm"), import("@xterm/addon-fit")])
      .then(([{ Terminal }, { FitAddon }]) => {
    if (disposed || !containerRef.current) return;
    const id = session.id;
    const launch = session.launch;
    const term = new Terminal({
      fontFamily: '"JetBrains Mono", ui-monospace, "SF Mono", Menlo, monospace',
      fontSize: 13,
      cursorBlink: true,
      screenReaderMode: true,
      scrollback: 5000,
      theme: { background: "#16181c", foreground: "#e8e6e0", cursor: "#ff7a33" },
    });
    const fit = new FitAddon();
    term.loadAddon(fit);
    term.open(containerRef.current);
    fit.fit();

    const channel = new Channel<number[]>();
    const tailDecoder = new TextDecoder("utf-8", { fatal: false });
    channel.onmessage = (bytes) => {
      const chunk = new Uint8Array(bytes);
      term.write(chunk);
      const text = tailDecoder.decode(chunk, { stream: true });
      // Backend exit sentinel (terminal.rs GRAFIKI_EXIT_SENTINEL): the child
      // died while we were attached — flip the header/composer to "ended"
      // instead of leaving a live dot over a dead PTY.
      if (text.includes("\x1b]7777;grafiki-session-exited\x07")) {
        setEnded(true);
      }
      termTailRef.current = (termTailRef.current + text).slice(-4000);
    };

    const resize = () => {
      try {
        fit.fit();
      } catch {
        /* pane not laid out yet */
      }
      void invoke("terminal_resize", { id, rows: term.rows, cols: term.cols });
    };

    let launchTimer: number | undefined;
    let handoffTimer: number | undefined;
    // Type a command into the fresh shell exactly once per session
    // (sessionStorage guard survives StrictMode's dev double-mount).
    const launchGuard = `grafiki-terminal-launched:${id}`;
    const handoffGuard = `grafiki-terminal-handoff:${id}`;
    // Recall→act: once the agent has booted, hand it the cited memory context
    // as its opening prompt (single line — newlines would submit early).
    const scheduleHandoff = () => {
      const prompt = props.handoffPrompt?.replace(/\s+/g, " ").trim();
      if (!prompt || sessionStorage.getItem(handoffGuard)) {
        return;
      }
      handoffTimer = window.setTimeout(() => {
        if (!sessionStorage.getItem(handoffGuard)) {
          sessionStorage.setItem(handoffGuard, "1");
          void invoke("terminal_write", { id, data: prompt }).then(() =>
            window.setTimeout(() => void invoke("terminal_write", { id, data: "\r" }), 400),
          );
        }
      }, 7000);
    };
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
    // This session's capture id, pinned to THIS effect run. The shared
    // captureIdRef is overwritten by the adoption effect before cleanup runs
    // on a tab switch, so cleanup reading the ref would extract the WRONG
    // (incoming) session's capture instead of the departing one.
    let effectCaptureId: string | null | undefined = captureIdRef.current;
    const connect = async () => {
      try {
        if (spawnRef.current) {
          spawnRef.current = false;
          // Spawn the login shell (full PATH); the agent is typed in after.
          const opened = await invoke<{
            id: string;
            capturing: boolean;
            capture_hint: string | null;
            capture_id: string | null;
            capture_mode: "off" | "digest" | "full";
          }>("terminal_open", {
            id,
            cwd: props.projectRoot || props.fallbackCwd,
            command: "",
            launch,
            rows: term.rows,
            cols: term.cols,
            onOutput: channel,
          });
          if (!cancelled) {
            setCapturing(opened.capturing);
            setCaptureHint(opened.capture_hint);
            setCaptureMode(opened.capture_mode);
            captureIdRef.current = opened.capture_id;
            effectCaptureId = opened.capture_id;
            const nextRef = {
              ...session,
              captureId: opened.capture_id,
              captureMode: opened.capture_mode,
            };
            setSession(nextRef);
            props.onUpdated?.(nextRef);
            window.setTimeout(resize, 350); // refit after the pane settles
            scheduleType(launch);
            scheduleHandoff();
          }
          return;
        }
        const reply = await invoke<{
          found: boolean;
          exited: boolean;
          cwd: string;
          capturing: boolean;
          capture_hint: string | null;
          capture_id: string | null;
          capture_mode: "off" | "digest" | "full";
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
            capture_hint: string | null;
            capture_id: string | null;
            capture_mode: "off" | "digest" | "full";
          }>("terminal_revive", { id, rows: term.rows, cols: term.cols, onOutput: channel });
          if (cancelled) {
            return;
          }
          if (!revive.found) {
            // Nothing to revive (explicitly ended): drop this session.
            sessionStorage.removeItem(launchGuard);
            if (controlled && session) {
              props.onEnded?.(session.id);
            } else {
              localStorage.removeItem(terminalStorageKey(props.projectRoot));
            }
            setSession(null);
            return;
          }
          setCapturing(revive.capturing);
          setCaptureHint(revive.capture_hint);
          setCaptureMode(revive.capture_mode);
          captureIdRef.current = revive.capture_id;
          effectCaptureId = revive.capture_id;
          const revivedRef = {
            ...session,
            captureId: revive.capture_id,
            captureMode: revive.capture_mode,
          };
          setSession(revivedRef);
          props.onUpdated?.(revivedRef);
          // Resume the agent's own session where supported; otherwise relaunch it.
          scheduleType(revive.launch === "claude" ? "claude --continue" : revive.launch);
          return;
        }
        setCapturing(reply.capturing);
        setCaptureHint(reply.capture_hint);
        setCaptureMode(reply.capture_mode);
        captureIdRef.current = reply.capture_id;
        effectCaptureId = reply.capture_id;
        const attachedRef = {
          ...session,
          captureId: reply.capture_id,
          captureMode: reply.capture_mode,
        };
        setSession(attachedRef);
        props.onUpdated?.(attachedRef);
        if (reply.exited) {
          setEnded(true);
          return;
        }
        // Reattached to a live session: sync the PTY to the new pane size and
        // finish the launch typing if a dev remount interrupted it.
        resize();
        window.setTimeout(resize, 350); // second fit once layout has settled
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
    // into Review candidates (backend is single-flight; extraction must never
    // disturb the terminal, but its failures surface as background notices).
    const extractTimer = window.setInterval(() => {
      void runExtraction(
        { startDir: props.projectRoot, captureId: effectCaptureId },
        "session heartbeat",
      );
    }, 120_000);

    cleanup = () => {
      cancelled = true;
      if (launchTimer) {
        window.clearTimeout(launchTimer);
      }
      if (handoffTimer) {
        window.clearTimeout(handoffTimer);
      }
      window.clearInterval(extractTimer);
      observer.disconnect();
      onData.dispose();
      // Detach ONLY — the session (and the agent) keeps running in the pool.
      void invoke("terminal_detach", { id });
      term.dispose();
      // One more pass on the way out, so Review is fresh when the user lands
      // there — pinned to the DEPARTING session's capture id (see above).
      void runExtraction(
        { startDir: props.projectRoot, captureId: effectCaptureId },
        "session end",
      );
    };
      })
      .catch((loadError) => {
        if (!disposed) setError(`Could not load the terminal renderer: ${String(loadError)}`);
      });
    return () => {
      disposed = true;
      cleanup?.();
    };
  }, [session?.id, props.projectRoot]);

  const endSession = () => {
    if (session) {
      void invoke("terminal_close", { id: session.id });
      sessionStorage.removeItem(`grafiki-terminal-launched:${session.id}`);
      if (controlled) {
        props.onEnded?.(session.id);
      } else {
        localStorage.removeItem(terminalStorageKey(props.projectRoot));
      }
    }
    setSession(null);
    setEnded(false);
    setError(null);
    setCapturing(null);
    setCaptureMode("off");
    // The lens is per-session state: without this reset the NEXT session opens
    // straight into the Chat lens with the terminal (and its trust prompts)
    // invisible — the pane never remounts across end→start.
    setLens("terminal");
    captureIdRef.current = null;
  };

  const startSession = (cmd: string) => {
    const id = `term-${Math.random().toString(36).slice(2, 10)}-${Date.now().toString(36)}`;
    const next = { id, launch: cmd };
    if (controlled) {
      props.onStarted?.(next);
    } else {
      localStorage.setItem(terminalStorageKey(props.projectRoot), JSON.stringify(next));
    }
    spawnRef.current = true;
    setEnded(false);
    setError(null);
    setCapturing(null);
    setLens("terminal");
    captureIdRef.current = null;
    setSession(next);
  };

  if (session === null) {
    const options = [
      { label: "Claude Code", cmd: "claude" },
      { label: "Codex", cmd: "codex" },
      { label: "Gemini", cmd: "gemini" },
      { label: "Shell", cmd: "" },
    ];
    const willCapture =
      launcherCaptureConfig !== null &&
      launcherCaptureConfig.config.sources.terminal &&
      launcherCaptureConfig.config.terminal_output !== "off";
    return (
      <div
        className="view-stack"
        style={{ padding: 28, display: "flex", flexDirection: "column", gap: 16, alignItems: "flex-start" }}
      >
        <div>
          <h2 style={{ margin: 0 }}>Start a session</h2>
          <p className="muted" style={{ marginTop: 6, maxWidth: 460 }}>
            It runs inside Grafiki, in <code>{props.projectRoot || props.fallbackCwd || "this project"}</code>.{" "}
            {willCapture
              ? "Work normally — this session is captured automatically, no setup."
              : "Capture is off for this folder — nothing will be remembered. Turn it on in Settings → Capture Consent."}
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
        <div className="session-status">
          <span className={`status-dot ${ended ? "ended" : "live"}`} aria-hidden />
          <span className="session-status-label">
            {ended ? "Session ended" : session.launch ? `${session.launch} running` : "Shell"}
          </span>
          <span
            className="session-status-path"
            title={props.projectRoot || props.fallbackCwd || "this project"}
          >
            {tidyPath(props.projectRoot || props.fallbackCwd || "this project")}
          </span>
          {capturing === true ? (
            <span className="chip chip-live">Capturing · {captureMode}</span>
          ) : null}
          {capturing === false ? (
            <span
              className="chip chip-warn"
              title={
                (captureHint ?? "Turn on Terminal capture in Settings → Capture & privacy") +
                " — capture is decided when a session starts, so it applies to the next session."
              }
            >
              Not capturing
            </span>
          ) : null}
        </div>
        {chatCapable ? (
          <span className="seg-tabs lens-tabs" role="tablist" aria-label="Session view" onKeyDown={handleTablistKeyDown}>
            <button
              className={`seg-tab ${lens === "terminal" ? "active" : ""}`}
              role="tab"
              aria-selected={lens === "terminal"}
              onClick={() => setLens("terminal")}
            >
              Terminal
            </button>
            <button
              className={`seg-tab ${lens === "chat" ? "active" : ""}`}
              role="tab"
              aria-selected={lens === "chat"}
              onClick={() => setLens("chat")}
            >
              Chat
            </button>
          </span>
        ) : null}
        <button
          className={`button secondary session-peek-toggle ${peekOpen ? "active" : ""}`}
          style={{ marginLeft: "auto" }}
          onClick={() => setPeekOpen((current) => !current)}
          title={peekOpen ? "Hide the learned panel" : "Show the learned panel"}
        >
          Learned
          {peekNew.length > 0 ? <span className="count-badge">{peekNew.length}</span> : null}
        </button>
        <button className={`button ${ended ? "primary" : "danger"}`} onClick={endSession}>
          {ended ? "New session" : "End session"}
        </button>
      </div>
      {error ? <p style={{ color: "var(--danger)" }}>{error}</p> : null}
      <div style={{ flex: 1, minHeight: 0, display: "flex", gap: 10 }}>
        <div
          ref={containerRef}
          role="region"
          aria-label={`${sessionTabTitle(session.launch)} interactive terminal`}
          style={{
            flex: 1,
            minHeight: 0,
            background: "#16181c",
            borderRadius: 8,
            overflow: "hidden",
            padding: 6,
            display: lens === "chat" ? "none" : "block",
          }}
        />
        {lens === "chat" ? (
          <div className="chat-lens">
            {pendingPrompt ? (
              <div className="notice compact chat-lens-prompt-banner">
                <AlertTriangle size={15} />
                <span>Agent is waiting on a permission decision</span>
                <button className="button" type="button" onClick={() => setLens("terminal")}>
                  Switch to Terminal
                </button>
              </div>
            ) : null}
            <div className="chat-lens-scroll" ref={chatScrollRef}>
              {transcriptError ? (
                <div className="notice compact" role="alert">
                  {transcriptError}
                </div>
              ) : null}
              {visibleTurns.length === 0 ? (
                <p className="muted" style={{ margin: "auto", textAlign: "center" }}>
                  Waiting for the conversation… (the transcript appears after Claude's first
                  reply — flip to Terminal for permission prompts)
                </p>
              ) : (
                visibleTurns.map((item, index) => (
                  <LensBubble key={index} role={item.display} text={item.turn.text} />
                ))
              )}
            </div>
            <div className="search-box">
              <input
                value={composer}
                aria-label="Message the live Claude session"
                placeholder="Message Claude… (sent to the live session)"
                onChange={(event) => setComposer(event.target.value)}
                onKeyDown={(event) => {
                  if (event.key === "Enter" && !event.shiftKey) {
                    event.preventDefault();
                    void sendComposer();
                  }
                }}
              />
              <button onClick={() => void sendComposer()} disabled={ended || !composer.trim()}>
                Send
              </button>
            </div>
          </div>
        ) : null}
        {peekOpen ? (
          <aside className="term-peek">
            <div className="term-peek-title">Learned this session</div>
            {peekNew.length === 0 ? (
              <div className="peek-empty">
                {capturing === true ? (
                  <p className="subtle">
                    No durable memories captured yet. Grafiki is watching this session for
                    decisions, fixes, and commands worth keeping.
                  </p>
                ) : capturing === false ? (
                  <p className="subtle">
                    Capture is off for this folder, so nothing is being learned from this
                    session. Turn on Terminal capture in Settings → Capture &amp; privacy.
                  </p>
                ) : (
                  <p className="subtle">No durable memories captured yet.</p>
                )}
                {capturing === true ? (
                  <ul className="watch-list">
                    <li>
                      <CheckCircle2 size={13} /> Terminal
                    </li>
                    <li>
                      <CheckCircle2 size={13} /> Git
                    </li>
                    <li>
                      <CheckCircle2 size={13} /> Files
                    </li>
                    <li>
                      <CheckCircle2 size={13} /> Transcript
                    </li>
                  </ul>
                ) : null}
                {peekOlder > 0 ? <p className="subtle">{peekOlder} older waiting in Review.</p> : null}
              </div>
            ) : (
              peekNew.map((candidate) => (
                <motion.div
                  key={candidate.id}
                  className="peek-item"
                  initial={{ opacity: 0, x: 10 }}
                  animate={{ opacity: 1, x: 0 }}
                  transition={transition.quick}
                >
                  <span className="record-type">{candidate.record_type}</span>
                  <b>{candidateTitle(candidate)}</b>
                  <div className="peek-actions">
                    <button
                      className="icon-button success"
                      disabled={peekBusy === candidate.id}
                      title="Approve"
                      onClick={() => void reviewPeek(candidate, true)}
                    >
                      <CheckCircle2 size={14} />
                    </button>
                    <button
                      className="icon-button danger"
                      disabled={peekBusy === candidate.id}
                      title="Reject"
                      onClick={() => void reviewPeek(candidate, false)}
                    >
                      <X size={14} />
                    </button>
                  </div>
                </motion.div>
              ))
            )}
          </aside>
        ) : null}
      </div>
    </div>
  );
}

function ChatPane(props: {
  pane: PaneState;
  snapshot: ProjectSnapshot | null;
  projectRoot: string;
  onUpdate: (patch: Partial<PaneState>) => void;
  onOpenResult: (result: SearchResult) => void;
  onNavigate: (kind: PaneKind, patch?: Partial<PaneState>) => void;
  onMemoryChanged: () => Promise<ProjectSnapshot>;
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
  const [memTab, setMemTab] = useState<
    "chat" | "search" | "decisions" | "context" | "activity"
  >("chat");
  const [decisions, setDecisions] = useState<DecisionItem[] | null>(null);
  const [contexts, setContexts] = useState<ContextSummary[] | null>(null);
  // A backend failure is NOT an empty list — "no decisions yet" told the user
  // their memory was empty when the query actually errored.
  const [browseError, setBrowseError] = useState<string | null>(null);
  const [activity, setActivity] = useState<AgentQueryLogItem[] | null>(null);
  const [searchQuery, setSearchQuery] = useState("");
  const [searchMode, setSearchMode] = useState<SearchMode>("keyword");
  const [searchRecordType, setSearchRecordType] = useState("all");
  const [searchResults, setSearchResults] = useState<SearchResult[] | null>(null);
  const [searching, setSearching] = useState(false);
  const [surfaceError, setSurfaceError] = useState<string | null>(null);
  const [surfaceMessage, setSurfaceMessage] = useState<string | null>(null);
  const [manualOpen, setManualOpen] = useState(false);
  const [manualType, setManualType] = useState<"decision" | "context">("decision");
  const [manualTitle, setManualTitle] = useState("");
  const [manualContent, setManualContent] = useState("");
  const [manualBusy, setManualBusy] = useState(false);

  useEffect(() => {
    let cancelled = false;
    if (memTab === "decisions") {
      setDecisions(null);
      setBrowseError(null);
      listProjectDecisions({ startDir: props.projectRoot, scope })
        .then((items) => {
          if (!cancelled) setDecisions(items);
        })
        .catch((loadError) => {
          if (!cancelled) setBrowseError(String(loadError));
        });
    }
    if (memTab === "context") {
      setContexts(null);
      setBrowseError(null);
      listProjectContext({ startDir: props.projectRoot, scope })
        .then((items) => {
          if (!cancelled) setContexts(items);
        })
        .catch((loadError) => {
          if (!cancelled) setBrowseError(String(loadError));
        });
    }
    if (memTab === "activity") {
      setActivity(null);
      listAgentActivity({ startDir: props.projectRoot, scope, limit: 100 })
        .then((items) => {
          if (!cancelled) setActivity(items);
        })
        .catch((activityError) => {
          if (!cancelled) {
            setActivity([]);
            setSurfaceError(String(activityError));
          }
        });
    }
    return () => {
      cancelled = true;
    };
  }, [memTab, props.projectRoot, scope]);

  async function runTrustedSearch() {
    const query = searchQuery.trim();
    if (!query || searching) return;
    setSearching(true);
    setSurfaceError(null);
    try {
      const report = await searchProjectMemory({
        startDir: props.projectRoot,
        query,
        mode: searchMode,
        scope,
        recordType: searchRecordType,
        limit: 100,
      });
      setSearchResults(report.results);
      if (report.fallback) setSurfaceMessage(report.fallback);
    } catch (searchError) {
      setSurfaceError(String(searchError));
      setSearchResults([]);
    } finally {
      setSearching(false);
    }
  }

  async function createManualMemory(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!manualTitle.trim() || !manualContent.trim() || manualBusy) return;
    setManualBusy(true);
    setSurfaceError(null);
    setSurfaceMessage(null);
    try {
      const result = await captureMemory({
        startDir: props.projectRoot,
        captureType: manualType,
        title: manualTitle.trim(),
        content: manualContent.trim(),
        scope,
        category: manualType === "context" ? "reference" : undefined,
      });
      setSurfaceMessage(result.message);
      setManualTitle("");
      setManualContent("");
      setManualOpen(false);
      await props.onMemoryChanged();
      setMemTab(manualType === "decision" ? "decisions" : "context");
    } catch (captureError) {
      setSurfaceError(String(captureError));
    } finally {
      setManualBusy(false);
    }
  }

  // A question routed in from Home's ask bar starts the conversation.
  const askedInitial = useRef(false);
  useEffect(() => {
    const initial = props.pane.query?.trim();
    if (initial && !askedInitial.current) {
      askedInitial.current = true;
      void ask(initial);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

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

  async function ask(preset?: string) {
    const q = (preset ?? question).trim();
    if (!q || sending) return;
    setSending(true);
    setQuestion("");
    setTurns((prev) => [...prev, { question: q, reply: null, error: null }]);
    // Keep the pane title stable ("Memory") — a permanent nav destination
    // shouldn't rename itself to the last question asked.
    props.onUpdate({ scope });
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
      <div className="seg-tabs" role="tablist" aria-label="Memory views" onKeyDown={handleTablistKeyDown}>
        {/* Full tab semantics: roving tabIndex + tab↔panel id links. */}
        {(
          [
            ["chat", "Ask"],
            ["search", "Search"],
            ["decisions", "Decisions"],
            ["context", "Context"],
            ["activity", "Agent activity"],
          ] as const
        ).map(([key, label]) => (
          <button
            key={key}
            role="tab"
            id={`mem-tab-${key}`}
            aria-controls={`mem-panel-${key}`}
            aria-selected={memTab === key}
            tabIndex={memTab === key ? 0 : -1}
            className={`seg-tab ${memTab === key ? "active" : ""}`}
            onClick={() => setMemTab(key)}
          >
            {label}
          </button>
        ))}
      </div>
      <div className="memory-surface-actions">
        <button className="button primary btn-sm" type="button" onClick={() => setManualOpen((open) => !open)}>
          <Plus size={14} /> New memory
        </button>
      </div>
      <p className="mem-caption">
        {memTab === "chat"
          ? "Answers come only from your approved memory — always with sources."
          : memTab === "search"
            ? "Search trusted memory exactly or semantically, scoped to the project area you choose."
            : memTab === "decisions"
              ? "Browse durable project choices Grafiki can recall later."
              : memTab === "context"
                ? "Browse trusted context imported or created for this project."
                : "See which questions agents asked Grafiki and which memory records were returned."}
      </p>
      {surfaceMessage ? <section className="notice compact good" role="status" aria-live="polite">{surfaceMessage}</section> : null}
      {surfaceError ? <section className="notice compact" role="alert">{surfaceError}</section> : null}
      {manualOpen ? (
        <form className="manual-memory-form settings-editor" onSubmit={createManualMemory}>
          <strong>Create trusted memory manually</strong>
          <p className="muted">Use this for a decision or context you are intentionally recording yourself.</p>
          <div className="settings-duo">
            <label className="field-label">
              <span>Memory type</span>
              <select value={manualType} onChange={(event) => setManualType(event.target.value as "decision" | "context")}>
                <option value="decision">Decision</option>
                <option value="context">Context</option>
              </select>
            </label>
            <label className="field-label">
              <span>Scope</span>
              <input value={scope} onChange={(event) => setScope(event.target.value)} placeholder="All memory" />
            </label>
          </div>
          <label className="field-label">
            <span>Title</span>
            <input required value={manualTitle} onChange={(event) => setManualTitle(event.target.value)} />
          </label>
          <label className="field-label">
            <span>{manualType === "decision" ? "Reasoning" : "Context"}</span>
            <textarea required value={manualContent} onChange={(event) => setManualContent(event.target.value)} />
          </label>
          <div className="form-actions">
            <button type="button" className="button secondary" onClick={() => setManualOpen(false)}>Cancel</button>
            <button className="button primary" disabled={manualBusy || !manualTitle.trim() || !manualContent.trim()}>
              {manualBusy ? "Saving…" : "Save memory"}
            </button>
          </div>
        </form>
      ) : null}
      {memTab === "search" ? (
        <div
          className="mem-tab-panel"
          role="tabpanel"
          id="mem-panel-search"
          aria-labelledby="mem-tab-search"
        >
          <form
            className="trusted-search-form"
            onSubmit={(event) => {
              event.preventDefault();
              void runTrustedSearch();
            }}
          >
            <label className="field-label trusted-search-query">
              <span>Search trusted memory</span>
              <input
                value={searchQuery}
                onChange={(event) => setSearchQuery(event.target.value)}
                placeholder="Exact words, decision, file, or concept"
              />
            </label>
            <label className="compact-select">
              <span>Match</span>
              <select value={searchMode} onChange={(event) => setSearchMode(event.target.value as SearchMode)}>
                <option value="keyword">Exact / keyword</option>
                <option value="semantic">Semantic</option>
                <option value="hybrid">Hybrid</option>
              </select>
            </label>
            <label className="compact-select">
              <span>Type</span>
              <select value={searchRecordType} onChange={(event) => setSearchRecordType(event.target.value)}>
                <option value="all">All trusted memory</option>
                <option value="decision">Decisions</option>
                <option value="context">Context</option>
                <option value="state">State</option>
                <option value="entity">Entities</option>
                <option value="observation">Observations</option>
                <option value="session">Sessions</option>
              </select>
            </label>
            <label className="field-label">
              <span>Scope</span>
              <input value={scope} onChange={(event) => setScope(event.target.value)} placeholder="All scopes" />
            </label>
            <button className="button primary" disabled={searching || !searchQuery.trim()}>
              {searching ? "Searching…" : "Search"}
            </button>
          </form>
          {searchResults === null ? (
            <EmptyRecordList text="Enter a query to search trusted memory." />
          ) : searchResults.length === 0 ? (
            <EmptyRecordList text="No trusted memory matched this query and scope." />
          ) : (
            <div className="dense-list" aria-label="Trusted memory search results">
              {searchResults.map((result) => (
                <button
                  type="button"
                  key={`${result.record_type}-${result.id}`}
                  className="data-row data-row-button"
                  onClick={() => props.onOpenResult(result)}
                >
                  <span className="record-type">{result.record_type}</span>
                  <b>{result.title}</b>
                  <span className="subtle">{typeof result.score === "number" ? result.score.toFixed(3) : result.scope || "global"}</span>
                </button>
              ))}
            </div>
          )}
        </div>
      ) : null}
      {memTab === "activity" ? (
        <div
          className="mem-tab-panel"
          role="tabpanel"
          id="mem-panel-activity"
          aria-labelledby="mem-tab-activity"
        >
          {activity === null ? (
            <p className="muted" role="status">Loading agent activity…</p>
          ) : activity.length === 0 ? (
            <EmptyRecordList text="No agent memory queries have been recorded for this scope." />
          ) : (
            <div className="dense-list" aria-label="Agent activity">
              {activity.map((item) => (
                <div className="data-row agent-activity-row" key={item.id}>
                  <span className="record-type">{item.agent}</span>
                  <span>{item.question}</span>
                  <code>{item.returned_ids.length} records · {item.latency_ms} ms</code>
                </div>
              ))}
            </div>
          )}
        </div>
      ) : null}
      {memTab === "decisions" ? (
        <div
          className="mem-tab-panel"
          role="tabpanel"
          id="mem-panel-decisions"
          aria-labelledby="mem-tab-decisions"
        >
          {browseError ? (
            <div className="mem-empty" role="alert">
              <h3>Couldn't load decisions</h3>
              <p>{browseError}</p>
            </div>
          ) : decisions === null ? (
            <p className="muted">Loading…</p>
          ) : decisions.length === 0 ? (
            <div className="mem-empty">
              <h3>No approved decisions yet</h3>
              <p>
                Decisions are durable project choices Grafiki can recall later. Approve
                decision-like candidates in Review to build a lasting project record.
              </p>
              <button className="button primary" onClick={() => props.onNavigate("candidates")}>
                Open Review
              </button>
            </div>
          ) : (
            <div className="dense-list">
              {decisions.map((decision) => (
              <button
                type="button"
                key={decision.id}
                className="data-row data-row-button"
                style={{ cursor: "pointer" }}
                onClick={() =>
                  props.onOpenResult({
                    record_type: "decision",
                    id: decision.id,
                    title: decision.title,
                    snippet: decision.reasoning ?? "",
                    scope: decision.scope,
                  })
                }
              >
                <span className="record-type">decision</span>
                <b style={{ fontWeight: 550 }}>{decision.title}</b>
                <span className="subtle" style={{ marginLeft: "auto" }}>
                  {decision.status}
                </span>
              </button>
              ))}
            </div>
          )}
        </div>
      ) : null}
      {memTab === "context" ? (
        <div
          className="mem-tab-panel"
          role="tabpanel"
          id="mem-panel-context"
          aria-labelledby="mem-tab-context"
        >
          {browseError ? (
            <div className="mem-empty" role="alert">
              <h3>Couldn't load context</h3>
              <p>{browseError}</p>
            </div>
          ) : contexts === null ? (
            <p className="muted">Loading…</p>
          ) : contexts.length === 0 ? (
            <div className="mem-empty">
              <h3>No project context yet</h3>
              <p>
                Grafiki imports context automatically from CLAUDE.md, git history, session
                transcripts, and files. Run a session and it fills in as you work.
              </p>
              <button className="button" onClick={() => props.onNavigate("terminal", { query: "claude" })}>
                Start a session
              </button>
            </div>
          ) : (
            <div className="dense-list">
              {contexts.map((context) => (
                <button
                  type="button"
                  key={context.key}
                  className="data-row data-row-button"
                  style={{ cursor: "pointer" }}
                  onClick={() =>
                    props.onOpenResult({
                      record_type: "context",
                      id: context.key,
                      title: context.title,
                      snippet: "",
                      scope: context.scope,
                    })
                  }
                >
                  <span className="record-type">context</span>
                  <b style={{ fontWeight: 550 }}>{context.title}</b>
                  <span className="subtle" style={{ marginLeft: "auto" }}>
                    {context.category}
                  </span>
                </button>
              ))}
            </div>
          )}
        </div>
      ) : null}
      <div
        className="mem-chat-scroll"
        role="tabpanel"
        id="mem-panel-chat"
        aria-labelledby="mem-tab-chat"
        style={{ display: memTab === "chat" ? "flex" : "none" }}
      >
        <div className="mem-column" style={{ display: "flex", flexDirection: "column", flex: 1, gap: 16 }}>
          {turns.length === 0 ? (
            <div className="mem-empty">
              <h3>Ask your project memory</h3>
              <p>
                It answers only from what Grafiki has stored — always with sources — and says so
                honestly when it doesn't know. Try one of these:
              </p>
              <div className="mem-examples">
                {[
                  "What did we decide about local AI?",
                  "What bugs were fixed last session?",
                  "What paths are blocked from capture?",
                ].map((example) => (
                  <button key={example} className="mem-example" onClick={() => void ask(example)}>
                    {example}
                  </button>
                ))}
              </div>
            </div>
          ) : null}
          {turns.map((turn, index) => (
            <div key={index} className="mem-turn">
              <div className="mem-bubble user">{turn.question}</div>
              <div className="mem-answer">
                {turn.reply ? (
                  <>
                    <div style={{ whiteSpace: "pre-wrap", lineHeight: 1.6 }}>{turn.reply.answer}</div>
                    {turn.reply.citations.length > 0 ? (
                      <div className="mem-cites">
                        {turn.reply.citations.map((citation) => (
                          <button
                            key={citation.index}
                            className="mem-cite"
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
                          >
                            [{citation.index}] {citation.title || citation.record_type}
                          </button>
                        ))}
                      </div>
                    ) : null}
                    {turn.reply.used_memory ? (
                      <button
                        className="link-button"
                        style={{ marginTop: 8, fontSize: 12 }}
                        onClick={() =>
                          props.onNavigate("terminal", {
                            query: "claude",
                            handoffPrompt: `Context from my project memory (Grafiki): Q: ${turn.question} — A: ${turn.reply?.answer ?? ""} — Continue working from these decisions.`,
                          })
                        }
                      >
                        Continue this with Claude →
                      </button>
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
      </div>

      {memTab === "chat" ? (
      <div className="mem-composer mem-column">
        <div className="mem-options">
          <label>
            <input
              type="checkbox"
              checked={useModel}
              onChange={(event) => setUseModel(event.target.checked)}
            />
            <span>Answer locally</span>
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
          <label className="compact-select mem-scope">
            <span>Scope</span>
            <input
              value={scope}
              onChange={(event) => setScope(event.target.value)}
              placeholder="All memory"
              title="Limit answers to a scope such as a project or module path. Leave empty to search everything."
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
            aria-label="Ask your memory"
            autoComplete="off"
          />
          <button onClick={() => void ask()} disabled={sending || !question.trim()}>
            {sending ? "…" : "Ask"}
          </button>
        </div>
      </div>
      ) : null}
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
  totalPendingCount: number;
}) {
  const [candidates, setCandidates] = useState<ExtractionCandidate[]>([]);
  const [status, setStatus] = useState("pending");
  const [scope, setScope] = useState(props.snapshot?.scope ?? "");
  const [loading, setLoading] = useState(false);
  const [extracting, setExtracting] = useState(false);
  const [busyId, setBusyId] = useState<string | null>(null);
  const [selectedIds, setSelectedIds] = useState<string[]>([]);
  const [focusedCandidateId, setFocusedCandidateId] = useState<string | null>(null);
  const [minConfidence, setMinConfidence] = useState("0");
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editTitle, setEditTitle] = useState("");
  const [editContent, setEditContent] = useState("");
  const [editPayload, setEditPayload] = useState("");
  const [promptModal, setPromptModal] = useState<PromptConfig | null>(null);
  const [editScope, setEditScope] = useState("");
  const [editRationale, setEditRationale] = useState("");
  const [evidencePreview, setEvidencePreview] = useState<EvidenceLink | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const loadRequestRef = useRef(0);
  // Undo affordance for approve — the highest-blast-radius action in the queue
  // (it briefs future agent sessions) previously had no way back at all
  // (2026-07-04 don-norman-design-critic, H3). Cleared after a short window.
  const [undo, setUndo] = useState<{ ids: string[]; label: string } | null>(null);
  const undoTimerRef = useRef<number | undefined>(undefined);
  // Known scopes for the Scope filter + edit-form dropdowns, derived from a
  // status:"all" / scope:"" fetch decoupled from the review-queue's own
  // (narrower) fetch — free-text scope was a source of silent empty queues.
  const [knownScopes, setKnownScopes] = useState<string[]>([]);
  useEffect(() => {
    let cancelled = false; // a slow reply must not write the OLD project's scopes
    listCandidates({ startDir: props.startDir, scope: "", status: "all", limit: 200 })
      .then((all) => {
        if (!cancelled) {
          setKnownScopes(Array.from(new Set(all.map((c) => c.scope))).sort());
        }
      })
      .catch(() => undefined);
    return () => {
      cancelled = true;
    };
  }, [props.startDir]);

  function armUndo(ids: string[], label: string) {
    window.clearTimeout(undoTimerRef.current);
    setUndo({ ids, label });
    undoTimerRef.current = window.setTimeout(() => setUndo(null), 10_000);
  }
  // Any action that isn't itself arming a NEW undo must retire a stale one —
  // otherwise the Undo button lingers next to an unrelated later message.
  function clearUndo() {
    window.clearTimeout(undoTimerRef.current);
    setUndo(null);
  }
  useEffect(() => () => window.clearTimeout(undoTimerRef.current), []);

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
    const request = ++loadRequestRef.current;
    setLoading(true);
    setError(null);
    try {
      const nextCandidates = await listCandidates({
        startDir: props.startDir,
        scope,
        status,
        limit: 100,
      });
      if (request !== loadRequestRef.current) return;
      setCandidates(nextCandidates);
      setSelectedIds((ids) =>
        ids.filter((id) => nextCandidates.some((candidate) => candidate.id === id && candidate.status === "pending")),
      );
      setFocusedCandidateId((id) => {
        if (id && nextCandidates.some((candidate) => candidate.id === id)) return id;
        return nextCandidates.find((candidate) => candidate.status === "pending")?.id ?? nextCandidates[0]?.id ?? null;
      });
    } catch (listError) {
      if (request === loadRequestRef.current) setError(String(listError));
    } finally {
      if (request === loadRequestRef.current) setLoading(false);
    }
  }

  useEffect(() => {
    load();
    // totalPendingCount doubles as a staleness signal: candidates created or
    // resolved OUTSIDE this pane (heartbeat extraction, terminal peek) used to
    // leave the list showing stale rows until a manual refresh.
  }, [props.startDir, props.snapshot, scope, status, props.totalPendingCount]);

  // Opening Review runs one extraction pass over anything captured since the
  // last one (terminal output, transcripts), so fresh candidates are waiting.
  useEffect(() => {
    let cancelled = false;
    void runExtraction({ startDir: props.startDir }, "opening Review").then((report) => {
      if (!cancelled && report && report.proposed > 0) {
        void load();
      }
    });
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
      // System chords must pass through untouched — ⌘A is select-all, not
      // approve; there is no native menu to intercept them first.
      if (event.metaKey || event.ctrlKey || event.altKey) {
        return;
      }
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
        if (event.key === "r" && candidate.status === "pending") void performReject(candidate, "");
        if (event.key === "r" && candidate.status === "rejected") void reopen(candidate);
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
    let result;
    try {
      // The MUTATION is the operation whose success/failure we report. It
      // committed or it didn't — a later list-refresh error must not overwrite
      // the success message and tell the user the approval "failed".
      result = await approveCandidate({ startDir: props.startDir, id: candidate.id });
    } catch (approveError) {
      setError(String(approveError));
      setBusyId(null);
      return;
    }
    setMessage(`Approved “${candidateTitle(candidate)}” — now briefs your agent.`);
    armUndo([candidate.id], candidateTitle(candidate));
    const trustedResult = candidateToSearchResult(result.candidate);
    if (trustedResult) props.onSelectResult(trustedResult);
    try {
      await load();
      await props.onMemoryChanged();
    } catch (refreshError) {
      notifyBackground(`Approved, but the view couldn't refresh: ${String(refreshError)}`);
    } finally {
      setBusyId(null);
    }
  }

  // Single reject is now INSTANT (no modal) — it was the far more expensive of
  // the two review actions even though approve was the more dangerous one
  // (2026-07-04 don-norman-design-critic, H3). "Reject with note" below is the
  // opt-in slower path for anyone who wants to record why.
  function rejectWithNote(candidate: ExtractionCandidate) {
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
    clearUndo();
    setBusyId(candidate.id);
    setMessage(null);
    setError(null);
    let result;
    try {
      result = await rejectCandidate({ startDir: props.startDir, id: candidate.id, rationale });
    } catch (rejectError) {
      setError(String(rejectError));
      setBusyId(null);
      return;
    }
    // Mutation succeeded — a refresh failure is non-fatal, not a reject failure.
    setMessage(result.message);
    try {
      await load();
      await props.onMemoryChanged();
    } catch (refreshError) {
      notifyBackground(`Rejected, but the view couldn't refresh: ${String(refreshError)}`);
    } finally {
      setBusyId(null);
    }
  }

  // Reject was otherwise the only terminal, unrecoverable action in the queue
  // (every button disables once status != "pending") — this is the way back.
  async function reopen(candidate: ExtractionCandidate) {
    clearUndo();
    setBusyId(candidate.id);
    setMessage(null);
    setError(null);
    try {
      const result = await reopenCandidate({ startDir: props.startDir, id: candidate.id });
      setMessage(result.message);
      await load();
    } catch (reopenError) {
      setError(String(reopenError));
    } finally {
      setBusyId(null);
    }
  }

  async function performUndo() {
    if (!undo) return;
    window.clearTimeout(undoTimerRef.current);
    const { ids, label } = undo;
    setBusyId("bulk");
    setMessage(null);
    setError(null);
    // Undo each independently and track what didn't revert. A mid-loop failure
    // used to leave the rest un-undone AND had already cleared the Undo button,
    // stranding the user with a half-applied bulk approve and no recovery.
    const failed: string[] = [];
    let lastError: unknown = null;
    for (const id of ids) {
      try {
        await revertCandidateApproval({ startDir: props.startDir, id });
      } catch (undoError) {
        failed.push(id);
        lastError = undoError;
      }
    }
    // Refresh FIRST: load() clears the error state on entry, so reporting the
    // partial-failure outcome before it ran wiped the report a frame later.
    try {
      await load();
      await props.onMemoryChanged();
    } catch (refreshError) {
      notifyBackground(`Undo finished, but the view couldn't refresh: ${String(refreshError)}`);
    } finally {
      setBusyId(null);
    }
    const undone = ids.length - failed.length;
    if (failed.length === 0) {
      setUndo(null);
      setMessage(undone === 1 ? "Approval undone." : `${undone} approvals undone.`);
    } else {
      // Keep the still-approved ones armed so the recovery action survives.
      setUndo({ ids: failed, label });
      undoTimerRef.current = window.setTimeout(() => setUndo(null), 10_000);
      setError(
        `Undid ${undone} of ${ids.length}; ${failed.length} could not be undone (${String(lastError)}). Try Undo again.`,
      );
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

  // Which payload key holds the free-text body, per record type — mirrors
  // `approve_candidate_payload`'s per-type field-PREFERENCE ORDER in
  // crates/grafiki-core/src/memory.rs exactly. This must match precisely: an
  // earlier version always wrote to "content", but approve reads
  // ["observe","content"] for entity and ["details","content"] for state — so
  // editing an entity/state candidate's body silently lost the edit at
  // approval time, because the untouched, still-present preferred key won
  // (2026-07-04 adversarial review finding — a real, reproduced data-loss bug
  // this same audit fix introduced). Always resolve to whichever key already
  // has a value so the edit lands on the key approval will actually read.
  const CONTENT_KEY_PREFERENCE: Record<string, string[]> = {
    decision: ["reasoning", "content"],
    context: ["content", "body"],
    entity: ["observe", "content"],
    observation: ["content", "observe"],
    state: ["details", "content"],
  };
  const CONTENT_KEY_LABELS: Record<string, string> = {
    reasoning: "Reasoning",
    details: "Details",
    observe: "Observation",
  };
  function contentFieldKeyFor(candidate: ExtractionCandidate): string {
    const keys = CONTENT_KEY_PREFERENCE[candidate.record_type] ?? ["content"];
    return keys.find((key) => candidatePayloadString(candidate, [key])) ?? keys[0];
  }
  const [editContentKey, setEditContentKeyState] = useState("content");

  function beginEdit(candidate: ExtractionCandidate) {
    const contentKey = contentFieldKeyFor(candidate);
    setEditingId(candidate.id);
    setEditContentKeyState(contentKey);
    setEditTitle(candidatePayloadString(candidate, ["title", "name", "entity_name"]) ?? "");
    setEditContent(candidatePayloadString(candidate, [contentKey]) ?? "");
    setEditPayload(JSON.stringify(candidate.payload, null, 2));
    setEditScope(candidate.scope);
    setEditRationale(candidate.rationale ?? "");
    setMessage(null);
    setError(null);
  }

  // The Title/Content fields and the Advanced JSON textarea share ONE source
  // of truth (editPayload) — typing in a friendly field patches that key into
  // the parsed JSON immediately, so Save always just parses editPayload. If
  // the Advanced JSON is currently invalid, the friendly fields quietly stop
  // syncing (the JSON error surfaces at Save, same as before this rewrite).
  function syncEditPayload(key: string, value: string) {
    try {
      const parsed = JSON.parse(editPayload);
      if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) {
        parsed[key] = value;
        setEditPayload(JSON.stringify(parsed, null, 2));
      }
    } catch {
      /* Advanced JSON is invalid right now — leave it for the user to fix there. */
    }
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
    // Approval requires a title later; failing here beats saving a candidate
    // that "updates successfully" and then dead-ends at Approve with a raw
    // backend error.
    if (typeof payload.title === "string" && !payload.title.trim()) {
      setError("Title can't be empty — it's required when this candidate is approved.");
      return;
    }

    setBusyId(candidate.id);
    setMessage(null);
    setError(null);
    try {
      clearUndo();
      // Confidence is intentionally not sent: a human edit doesn't need the
      // reviewer to also pick a decimal — omitting it keeps the existing
      // value (2026-07-04 don-norman-design-critic).
      const result = await editCandidate({
        startDir: props.startDir,
        id: candidate.id,
        payload,
        scope: editScope,
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
      if (action === "approve" && result.succeeded > 0) {
        const approvedIds = result.results
          .filter((item) => item.candidate.status === "approved")
          .map((item) => item.candidate.id);
        armUndo(approvedIds, `${approvedIds.length} candidates`);
      } else {
        clearUndo();
      }
      setSelectedIds([]);
      // Refresh FIRST: load() clears the error state on entry, so reporting
      // the outcome (including partial-failure detail) before it ran wiped
      // the report a frame later (same bug class as performUndo above).
      await load();
      await props.onMemoryChanged();
      setMessage(`${result.action} complete: ${result.succeeded}/${result.requested} candidates reviewed.`);
      if (result.failed) {
        setError(result.errors.map((item) => `${item.id}: ${item.error}`).join("\n"));
      }
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
        title="Review queue"
        subtitle="Approve what Grafiki captured before it becomes memory your agents can recall."
        icon={ShieldQuestion}
        loading={loading}
        onRefresh={load}
      />
      <details className="kbd-help">
        <summary>Keyboard shortcuts</summary>
        <div className="kbd-help-row">
          <span>
            <kbd>j</kbd>/<kbd>k</kbd> move
          </span>
          <span>
            <kbd>a</kbd> approve
          </span>
          <span>
            <kbd>r</kbd> reject / reopen
          </span>
          <span>
            <kbd>e</kbd> edit
          </span>
          <span>
            <kbd>v</kbd> evidence
          </span>
          <span>
            <kbd>space</kbd> select
          </span>
        </div>
      </details>
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
        <label className="compact-select">
          <span>Scope</span>
          <select value={scope} onChange={(event) => setScope(event.target.value)}>
            <option value="">All scopes</option>
            {knownScopes.map((option) => (
              <option key={option || "global"} value={option}>
                {option || "global"}
              </option>
            ))}
          </select>
        </label>
        <label
          className="compact-input confidence-filter"
          title="How sure Grafiki was about an extraction (0–1). Candidates below this are hidden; 0 shows everything."
        >
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
        <button
          className="link-button select-shortcut"
          type="button"
          onClick={selectAllPending}
          disabled={!candidates.some((candidate) => candidate.status === "pending")}
        >
          Select all pending
        </button>
        <button
          className="link-button select-shortcut"
          type="button"
          onClick={selectLowConfidence}
          disabled={!candidates.some((candidate) => candidate.status === "pending" && candidateIsNoisy(candidate))}
          title="Select pending candidates that look like noise (low confidence or thin content)"
        >
          Select low-value
        </button>
        {candidates.length > 0 && visibleCandidates.length === 0 && minConfidenceValue > 0 ? (
          <span className="subtle">
            All hidden below {minConfidenceValue.toFixed(2)} — lower Min Confidence to see them.
          </span>
        ) : null}
        {scope !== "" && status === "pending" && props.totalPendingCount > candidates.length ? (
          <span className="subtle">
            {props.totalPendingCount - candidates.length} more pending in other scopes —{" "}
            <button className="link-button" type="button" onClick={() => setScope("")}>
              clear the scope filter
            </button>{" "}
            to see them.
          </span>
        ) : null}
      </div>
      {selectedIds.length > 0 ? (
        <div className="bulk-action-bar">
          <span className="bulk-count">{selectedIds.length} selected</span>
          <button className="button primary btn-sm" type="button" onClick={() => bulkReview("approve")} disabled={Boolean(busyId)}>
            <CheckCircle2 size={14} />
            Approve selected
          </button>
          <button className="button danger-button btn-sm" type="button" onClick={() => bulkReview("reject")} disabled={Boolean(busyId)}>
            <Trash2 size={14} />
            Reject selected
          </button>
          <button className="link-button" type="button" onClick={() => setSelectedIds([])}>
            Clear
          </button>
        </div>
      ) : null}
      {message ? (
        <section className="notice compact good">
          <span>{message}</span>
          {undo ? (
            <button className="link-button" type="button" onClick={() => void performUndo()}>
              Undo
            </button>
          ) : null}
        </section>
      ) : null}
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
                  <div className="candidate-actions candidate-group-actions">
                    <button className="button btn-sm" type="button" onClick={() => setSelectedIds((ids) => mergeIds(ids, pendingIds))} disabled={!pendingIds.length || Boolean(busyId)}>
                      Select group
                    </button>
                    <button className="button btn-sm" type="button" onClick={() => bulkReview("approve", pendingIds)} disabled={!pendingIds.length || Boolean(busyId)}>
                      Approve group
                    </button>
                    <button className="button btn-sm danger-button" type="button" onClick={() => bulkReview("reject", noisyIds)} disabled={!noisyIds.length || Boolean(busyId)}>
                      Reject low-signal
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
                              <small className="candidate-submeta">
                                <span className="chip chip-type">{candidate.record_type}</span>
                                {candidate.status !== "pending" ? (
                                  <span className={`chip chip-status chip-${candidate.status}`}>{candidate.status}</span>
                                ) : null}
                                <ConfidenceChip confidence={candidate.confidence} />
                                <span className="candidate-submeta-time">{candidateCreatedDateLabel(candidate)}</span>
                              </small>
                            </span>
                          </label>
                          <div className="candidate-actions candidate-primary-actions">
                            <button className="button primary btn-sm" type="button" onClick={() => approve(candidate)} disabled={candidate.status !== "pending" || isBusy}>
                              <CheckCircle2 size={14} />
                              Approve
                            </button>
                            <button className="button btn-sm" type="button" onClick={() => beginEdit(candidate)} disabled={candidate.status !== "pending" || isBusy}>
                              <Pencil size={14} />
                              Edit
                            </button>
                            <button className="button danger-button btn-sm" type="button" onClick={() => performReject(candidate, "")} disabled={candidate.status !== "pending" || isBusy}>
                              <Trash2 size={14} />
                              Reject
                            </button>
                            <button className="icon-button" type="button" onClick={() => rejectWithNote(candidate)} disabled={candidate.status !== "pending" || isBusy} title="Reject with a note">
                              <MessageSquare size={15} />
                            </button>
                            {candidate.status === "rejected" ? (
                              <button className="icon-button" type="button" onClick={() => reopen(candidate)} disabled={isBusy} title="Reopen for review">
                                <RefreshCcw size={15} />
                              </button>
                            ) : null}
                            <button className="icon-button" type="button" onClick={() => openTrusted(candidate)} disabled={!trustedResult} title="Open trusted memory">
                              <FileText size={15} />
                            </button>
                          </div>
                        </header>
                        {isEditing ? (
                          <div className="candidate-edit-grid">
                            <label className="candidate-edit-wide">
                              <span>Title</span>
                              <input
                                value={editTitle}
                                onChange={(event) => {
                                  setEditTitle(event.target.value);
                                  syncEditPayload("title", event.target.value);
                                }}
                              />
                            </label>
                            <label>
                              <span>Scope</span>
                              <select value={editScope} onChange={(event) => setEditScope(event.target.value)}>
                                <option value="">global</option>
                                {Array.from(new Set([...knownScopes, editScope]))
                                  .filter(Boolean)
                                  .sort()
                                  .map((option) => (
                                    <option key={option} value={option}>
                                      {option}
                                    </option>
                                  ))}
                              </select>
                            </label>
                            <label className="candidate-edit-wide">
                              <span>{CONTENT_KEY_LABELS[editContentKey] ?? "Content"}</span>
                              <textarea
                                value={editContent}
                                onChange={(event) => {
                                  setEditContent(event.target.value);
                                  syncEditPayload(editContentKey, event.target.value);
                                }}
                              />
                            </label>
                            <label className="candidate-edit-wide">
                              <span>Rationale</span>
                              <input value={editRationale} onChange={(event) => setEditRationale(event.target.value)} />
                            </label>
                            <details className="candidate-edit-advanced candidate-edit-wide">
                              <summary>Advanced — raw payload JSON</summary>
                              <textarea value={editPayload} onChange={(event) => setEditPayload(event.target.value)} spellCheck={false} />
                            </details>
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
                            <CollapsibleBody text={candidateBody(candidate)} />
                            <div className="candidate-meta-row">
                              <span>{candidate.scope || "global"}</span>
                              {candidate.source ? <span>{candidate.source}</span> : null}
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
                                    {evidence.source ? `${evidence.source_type} · ${evidence.source}` : evidence.source_type}
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
          <EmptyRecordList
            text="No candidates in this view."
            action={
              <button
                className="button primary"
                type="button"
                disabled={extracting}
                onClick={() => {
                  setExtracting(true);
                  setError(null);
                  extractSessionMemory({ startDir: props.startDir })
                    .then(() => load())
                    .catch((extractError) => setError(String(extractError)))
                    .finally(() => setExtracting(false));
                }}
              >
                {extracting ? "Checking sessions…" : "Check sessions for new memories"}
              </button>
            }
          />
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
  const [themePref, setThemePref] = useState<ThemePref>(
    () => (localStorage.getItem(THEME_KEY) as ThemePref) ?? "light",
  );
  const changeTheme = (pref: ThemePref) => {
    setThemePref(pref);
    localStorage.setItem(THEME_KEY, pref);
    applyTheme(pref);
  };
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
  // Settings is a tabbed sheet per docs/UX_REDESIGN.md §5.6 ("Tabs, each one
  // screen, no scroll-of-doom") — it used to be one long undifferentiated
  // scroll (2026-07-04 don-norman-design-critic finding).
  const [settingsTab, setSettingsTab] = useState<
    "projects" | "capture" | "local-ai" | "hookups" | "about"
  >("projects");
  const [localModels, setLocalModels] = useState<string[] | null>(null);
  useEffect(() => {
    if (settingsTab === "local-ai" && localModels === null) {
      listLocalModels()
        .then(setLocalModels)
        .catch(() => setLocalModels([]));
    }
  }, [settingsTab, localModels]);
  const copyToClipboard = async (text: string, label: string) => {
    setError(null);
    try {
      if (!navigator.clipboard) throw new Error("Clipboard access is unavailable.");
      await navigator.clipboard.writeText(text);
      setMessage(`${label} copied to clipboard.`);
    } catch (clipboardError) {
      setMessage(null);
      setError(`Could not copy ${label.toLowerCase()}: ${String(clipboardError)}`);
    }
  };
  // Only sources an actual capture path checks (2026-07-04 don-norman-design-critic:
  // ide/system/screen/browser/audio rendered as live-looking checkboxes that wrote
  // to the config file but gated nothing — false affordances in a consent surface
  // are worse than clutter). Re-add here the day each one gets a real producer.
  // Terminal is deliberately absent here: capture only happens when the
  // terminal source AND an output mode are both on, so the "Terminal capture"
  // dropdown below is the single control that drives both fields — two
  // controls for one effective state contradicted each other on screen.
  const captureSourceLabels: Array<[keyof CaptureSourceConfig, string]> = [
    ["git", "Git"],
    ["transcripts", "Transcripts"],
    ["files", "Files"],
  ];
  const effectiveTerminalCapture: "off" | "digest" | "full" =
    captureConfig && captureConfig.config.sources.terminal
      ? captureConfig.config.terminal_output
      : "off";

  useEffect(() => {
    setDraftRoot(props.projectRoot || snapshot?.start_dir || "");
  }, [props.projectRoot, snapshot?.start_dir]);

  // Monotonic guard: a slow getCaptureConfig/getDaemonStatus for project A must
  // not write A's privacy settings (or token) into project B's Settings after a
  // switch. Each refresh captures the current id and applies only if still current.
  const settingsRequestRef = useRef(0);
  useEffect(() => {
    const request = ++settingsRequestRef.current;
    // A per-project daemon token must NOT carry into the next project — reusing
    // project A's bearer token to start project B's daemon is a cross-workspace
    // credential leak. Reset the daemon fields on every switch.
    setDaemonToken("");
    setDaemonStatus(null);
    setDaemonHost("127.0.0.1");
    setDaemonPort(9700);
    // A stale captureConfig must not linger and stay visible/interactive for
    // the previous project if the new project's fetch fails or is slow.
    setCaptureConfig(null);
    // Pass the fresh root explicitly: this effect runs in the same commit as
    // the setDraftRoot above, so the functions' closures still hold the OLD
    // project's draftRoot and would show the previous project's config.
    const freshDir = props.projectRoot || snapshot?.start_dir || "";
    refreshDaemonStatus(freshDir, { request });
    refreshCaptureConfig(freshDir, request);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [props.projectRoot, snapshot?.project?.project]);

  // The daemon can die (or be killed) outside this app; without a live check
  // Settings kept saying "Running" indefinitely. Poll quietly while open.
  useEffect(() => {
    const timer = window.setInterval(
      () =>
        refreshDaemonStatus(undefined, { silent: true, request: settingsRequestRef.current }),
      20_000,
    );
    return () => window.clearInterval(timer);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [props.projectRoot]);

  async function refreshDaemonStatus(dir?: string, opts?: { silent?: boolean; request?: number }) {
    if (!opts?.silent) setDaemonBusy("status");
    try {
      const next = await getDaemonStatus({
        startDir: dir ?? (draftRoot || props.projectRoot || snapshot?.start_dir || ""),
      });
      // Drop a response that arrived after the project changed under it.
      if (opts?.request !== undefined && opts.request !== settingsRequestRef.current) return;
      setDaemonStatus(next);
      if (next.host) setDaemonHost(next.host);
      if (next.port) setDaemonPort(next.port);
    } catch (daemonError) {
      if (!opts?.silent) setError(String(daemonError));
    } finally {
      if (!opts?.silent) setDaemonBusy(null);
    }
  }

  async function refreshCaptureConfig(dir?: string, request?: number) {
    const startDir = dir ?? (draftRoot || props.projectRoot || snapshot?.start_dir || "");
    if (!startDir.trim()) return;
    setCaptureConfigBusy(true);
    try {
      const next = await getCaptureConfig({ startDir });
      // Drop a stale project's config so it can't overwrite the current one.
      if (request !== undefined && request !== settingsRequestRef.current) return;
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
    // Era-pin the mutation: a save racing a project switch must not display
    // the OLD project's config (or its "saved" toast) on the new one.
    const era = settingsRequestRef.current;
    setCaptureConfigBusy(true);
    setMessage(null);
    setError(null);
    try {
      const next = await updateCaptureConfig({ startDir, ...input });
      if (era !== settingsRequestRef.current) return;
      setCaptureConfig(next);
      setMessage("Capture settings saved.");
    } catch (configError) {
      if (era === settingsRequestRef.current) setError(String(configError));
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
    // Pin this mutation to the current project era: if the project switches
    // while the daemon starts, its result (incl. the bearer token) must not
    // land in the NEW project's Settings.
    const era = settingsRequestRef.current;
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
      if (era !== settingsRequestRef.current) return;
      // Surface the auto-generated token so the user can give it to external agents.
      if (result.token) setDaemonToken(result.token);
      setMessage(`${result.message} ${result.url}`);
      await refreshDaemonStatus(undefined, { request: era });
    } catch (daemonError) {
      if (era === settingsRequestRef.current) setError(String(daemonError));
    } finally {
      setDaemonBusy(null);
    }
  }

  async function stopProjectDaemon() {
    const era = settingsRequestRef.current;
    setDaemonBusy("stop");
    setMessage(null);
    setError(null);
    try {
      const result = await stopDaemon({
        startDir: draftRoot || props.projectRoot || snapshot?.start_dir || "",
      });
      if (era !== settingsRequestRef.current) return;
      setMessage(result.message);
      await refreshDaemonStatus(undefined, { request: era });
    } catch (daemonError) {
      if (era === settingsRequestRef.current) setError(String(daemonError));
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

  const mcpAddCommand = `claude mcp add grafiki -- grafiki mcp --read-only --path "${draftRoot || "."}"`;
  const cursorJson = JSON.stringify(
    {
      mcpServers: {
        grafiki: {
          command: "grafiki",
          args: ["mcp", "--read-only", "--path", draftRoot || "."],
        },
      },
    },
    null,
    2,
  );
  const shellHookCommand = `grafiki capture shell-hook --path "${draftRoot || "."}"`;

  return (
    <div className="view-stack settings-stack">
      <div className="seg-tabs" role="tablist" aria-label="Settings sections" onKeyDown={handleTablistKeyDown}>
        {/* Full tab semantics: roving tabIndex (one Tab stop, arrows move within)
            and tab↔panel id links, not just role+selected. */}
        {(
          [
            ["projects", "Projects"],
            ["capture", "Capture & privacy"],
            ["local-ai", "Local AI"],
            ["hookups", "Agent hookups"],
            ["about", "About"],
          ] as const
        ).map(([key, label]) => (
          <button
            key={key}
            role="tab"
            id={`settings-tab-${key}`}
            aria-controls={`settings-panel-${key}`}
            aria-selected={settingsTab === key}
            tabIndex={settingsTab === key ? 0 : -1}
            className={`seg-tab ${settingsTab === key ? "active" : ""}`}
            onClick={() => setSettingsTab(key)}
          >
            {label}
          </button>
        ))}
      </div>

      {message ? <section className="notice compact good">{message}</section> : null}
      {error ? (
        <section className="notice compact">
          <AlertTriangle size={16} />
          <span>{error}</span>
        </section>
      ) : null}

      {settingsTab === "projects" ? (
        <section
          className="settings-grid"
          role="tabpanel"
          id="settings-panel-projects"
          aria-labelledby="settings-tab-projects"
        >
          <div className="settings-editor">
            <label className="field-label">
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
          <div className="settings-editor">
            <Setting label="Current project" value={snapshot?.project?.project ?? "Not initialized"} />
            <Setting label="Database" value={snapshot?.project?.db_path ?? "Unavailable"} mono />
          </div>
        </section>
      ) : null}

      {settingsTab === "capture" ? (
        <section
          className="settings-grid"
          role="tabpanel"
          id="settings-panel-capture"
          aria-labelledby="settings-tab-capture"
        >
          <ListHeading title="Capture Consent" icon={ShieldQuestion} />
          <p className="muted">
            Choose what Grafiki may read from this project. Everything stays on this machine —
            nothing is uploaded.
          </p>
          <div className="settings-editor">
            <div className="capture-config-summary">
              <span title={captureConfig?.config_path ?? ""}>
                {tidyPath(captureConfig?.config_path ?? "No capture config loaded")}
              </span>
              <code title="How captured text is scrubbed before it is stored">
                Redaction profile: {captureConfig?.config.redaction_profile ?? "default"}
              </code>
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
            <div className="settings-duo">
              <label className="field-label">
                <span>Terminal capture</span>
                <select
                  // key remounts the native select when the loaded value arrives or
                  // changes — the embedded WebKit select can skip repainting its
                  // label when only the bound value updates, showing a stale option.
                  key={`terminal-capture-${captureConfig ? "ready" : "loading"}-${effectiveTerminalCapture}`}
                  value={effectiveTerminalCapture}
                  disabled={captureConfigBusy || !captureConfig}
                  onChange={(event) => {
                    const next = event.currentTarget.value as "off" | "digest" | "full";
                    patchCaptureConfig({ terminalOutput: next, terminal: next !== "off" });
                  }}
                >
                  <option value="off">Off</option>
                  <option value="digest">Digest only</option>
                  <option value="full">Full output</option>
                </select>
              </label>
              <label className="field-label">
                <span>Screenshots</span>
                <select
                  key={`screen-policy-${captureConfig ? "ready" : "loading"}-${captureConfig?.config.screen_policy ?? "manual"}`}
                  value={captureConfig?.config.screen_policy ?? "manual"}
                  disabled={captureConfigBusy || !captureConfig}
                  onChange={(event) =>
                    patchCaptureConfig({ screenPolicy: event.currentTarget.value as "off" | "manual" | "allowlist" })
                  }
                >
                  <option value="off">Off</option>
                  <option value="manual">Ask each time</option>
                  <option value="allowlist">Allowlist</option>
                </select>
              </label>
            </div>
            <label className="field-label">
              <span>Blocked Path</span>
              <input
                value={blockedPathDraft}
                onChange={(event) => setBlockedPathDraft(event.target.value)}
                placeholder="secrets or .env.local"
              />
            </label>
            <div className="maintenance-actions">
              <button className="button secondary" type="button" onClick={() => void refreshCaptureConfig(undefined, settingsRequestRef.current)} disabled={captureConfigBusy || !draftRoot.trim()}>
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
                  <X size={11} />
                </button>
              ))}
            </div>
          </div>
        </section>
      ) : null}

      {settingsTab === "local-ai" ? (
        <section
          className="settings-grid"
          role="tabpanel"
          id="settings-panel-local-ai"
          aria-labelledby="settings-tab-local-ai"
        >
          <ListHeading title="Local AI" icon={Sparkles} />
          <div className="settings-editor">
            {localModels === null ? (
              <p className="muted">Checking for Ollama…</p>
            ) : localModels.length > 0 ? (
              <>
                <p className="muted">
                  Ollama is running with {localModels.length} model{localModels.length === 1 ? "" : "s"} installed.
                  Grafiki uses one of these to turn sessions into memory, entirely on this machine.
                </p>
                <div className="capture-blocked-list">
                  {localModels.map((model) => (
                    <span key={model} className="evidence-chip">
                      {model}
                    </span>
                  ))}
                </div>
              </>
            ) : (
              <p className="muted">
                No local model found. Extraction still works manually, but automatic memory extraction needs
                Ollama — install it, then run <code>ollama pull gemma3:1b</code> (or any model).
              </p>
            )}
            <div className="form-actions">
              <button className="button secondary" type="button" onClick={() => setLocalModels(null)}>
                <RefreshCcw size={15} />
                Recheck
              </button>
            </div>
          </div>
        </section>
      ) : null}

      {settingsTab === "hookups" ? (
        <section
          className="settings-grid"
          role="tabpanel"
          id="settings-panel-hookups"
          aria-labelledby="settings-tab-hookups"
        >
          <ListHeading title="Agent Hookups" icon={Activity} />
          <div className="settings-editor">
            <p className="muted">
              Give other tools access to this project&apos;s memory over MCP — they can search and cite it, never
              write directly (writes still go through Review).
            </p>
            <label className="field-label">
              <span>Claude Code</span>
              <div className="copy-row">
                <code>{mcpAddCommand}</code>
                <button className="button secondary" type="button" onClick={() => copyToClipboard(mcpAddCommand, "Command")}>
                  Copy
                </button>
              </div>
            </label>
            <label className="field-label">
              <span>Cursor (.cursor/mcp.json)</span>
              <div className="copy-row">
                <code className="copy-row-multiline">{cursorJson}</code>
                <button className="button secondary" type="button" onClick={() => copyToClipboard(cursorJson, "Cursor config")}>
                  Copy
                </button>
              </div>
            </label>
            <label className="field-label">
              <span>Shell hook (records terminal command metadata)</span>
              <div className="copy-row">
                <code>{shellHookCommand}</code>
                <button className="button secondary" type="button" onClick={() => copyToClipboard(shellHookCommand, "Command")}>
                  Copy
                </button>
              </div>
            </label>
            <p className="muted" style={{ fontSize: 11.5 }}>
              Run that shell-hook command in a terminal and paste its output into <code>~/.zshrc</code>.
            </p>
          </div>
          <details className="settings-advanced">
            <summary>Advanced — HTTP daemon</summary>
            <div className="settings-editor">
              <dl className={`daemon-facts ${daemonStatus?.running ? "running" : ""}`}>
                <div>
                  <dt>Status</dt>
                  <dd className="daemon-state">{daemonStatus?.running ? "Running" : "Stopped"}</dd>
                </div>
                <div>
                  <dt>Endpoint</dt>
                  <dd className="mono">{daemonStatus?.url ?? "http://127.0.0.1:9700"}</dd>
                </div>
                <div>
                  <dt>Binary</dt>
                  <dd className="mono" title={daemonStatus?.cli_path ?? ""}>
                    {tidyPath(daemonStatus?.cli_path ?? "CLI not found")}
                  </dd>
                </div>
              </dl>
              <div className="settings-duo">
                <label className="field-label">
                  <span>Host</span>
                  <input value={daemonHost} onChange={(event) => setDaemonHost(event.target.value)} />
                </label>
                <label className="field-label">
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
              <label className="field-label">
                <span>Token</span>
                <input
                  type="password"
                  autoComplete="off"
                  value={daemonToken}
                  onChange={(event) => setDaemonToken(event.target.value)}
                  placeholder="auto-generated on Start"
                />
              </label>
              {daemonToken ? (
                <p className="daemon-token-hint">
                  External agents authenticate with this token (header <code>X-Grafiki-Token</code>).{" "}
                  <button className="link-button" type="button" onClick={() => copyToClipboard(daemonToken, "Daemon token")}>
                    Copy
                  </button>
                </p>
              ) : null}
              <div className="maintenance-actions">
                <button
                  className="button secondary"
                  onClick={() => void refreshDaemonStatus(undefined, { request: settingsRequestRef.current })}
                  disabled={daemonBusy !== null || !draftRoot.trim()}
                >
                  <RefreshCcw size={15} />
                  Refresh
                </button>
                <button
                  className="button primary"
                  onClick={startProjectDaemon}
                  disabled={
                    daemonBusy !== null ||
                    !draftRoot.trim() ||
                    !daemonStatus?.cli_available ||
                    Boolean(daemonStatus?.running)
                  }
                >
                  <Activity size={15} />
                  Start
                </button>
                <button
                  className="button danger-button"
                  onClick={stopProjectDaemon}
                  disabled={daemonBusy !== null || !daemonStatus?.running}
                >
                  <X size={15} />
                  Stop
                </button>
              </div>
            </div>
          </details>
        </section>
      ) : null}

      {settingsTab === "about" ? (
        <section
          className="settings-grid"
          role="tabpanel"
          id="settings-panel-about"
          aria-labelledby="settings-tab-about"
        >
          <div className="settings-editor">
            <div className="setting-row">
              <span>Appearance</span>
              <select
                aria-label="Appearance"
                value={themePref}
                onChange={(event) => changeTheme(event.target.value as ThemePref)}
              >
                <option value="light">Light</option>
                <option value="dark">Dark</option>
                <option value="system">System</option>
              </select>
            </div>
            <Setting label="Version" value="0.1.0" />
            <Setting label="License" value="MIT" />
            <Setting label="Data location" value={snapshot?.project?.db_path ?? "Unavailable"} mono />
          </div>
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
            </div>
          </div>
          <details className="settings-advanced">
            <summary>Advanced — embeddings & diagnostics</summary>
            <div className="settings-editor">
              <div className="maintenance-actions">
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
              <Setting label="Embedding provider" value={embedding?.provider ?? "Unknown"} />
              <Setting label="Vector backend" value={embedding?.vector_backend ?? "Unknown"} />
              <Setting label="Indexed records" value={`${embedding?.indexed_records ?? 0}`} />
              <Setting label="Missing or stale" value={`${embedding?.missing_or_stale_records ?? 0}`} />
            </div>
          </details>
        </section>
      ) : null}
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
    if ((needsTitle && !editTitle.trim()) || (needsBody && !editBody.trim())) {
      setActionError("Complete the required title and content fields before saving.");
      return;
    }
    const weight = Number(editWeight);
    const confidence = Number(editConfidence);
    if (isRelation && (Number.isNaN(weight) || Number.isNaN(confidence))) {
      setActionError("Relation weight and confidence must be valid numbers.");
      return;
    }

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
  const [copyFeedback, setCopyFeedback] = useState<{ text: string; error: boolean } | null>(null);

  useEffect(() => setCopyFeedback(null), [selectedId]);

  // Overlay/panel keyboard contract: focus moves in on open, Escape closes,
  // Tab is trapped within the panel, and focus returns to whatever had it
  // before — the workspace behind the narrow-width overlay is `inert`, so
  // without this the keyboard user is stranded with nothing focusable, or
  // Tab can walk them out into the inert workspace behind it.
  const closeButtonRef = useRef<HTMLButtonElement | null>(null);
  const asideRef = useRef<HTMLElement | null>(null);
  const onCloseRef = useRef(props.onClose);
  onCloseRef.current = props.onClose;
  useEffect(() => {
    const previouslyFocused = document.activeElement as HTMLElement | null;
    closeButtonRef.current?.focus();
    const focusableSelector =
      'button:not([disabled]), [href], input:not([disabled]), [tabindex]:not([tabindex="-1"])';
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        onCloseRef.current();
        return;
      }
      if (event.key !== "Tab" || !asideRef.current) return;
      const items = Array.from(
        asideRef.current.querySelectorAll<HTMLElement>(focusableSelector),
      ).filter((el) => el.offsetParent !== null);
      if (items.length === 0) return;
      const first = items[0];
      const last = items[items.length - 1];
      const active = document.activeElement;
      if (event.shiftKey && active === first) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && active === last) {
        event.preventDefault();
        first.focus();
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => {
      window.removeEventListener("keydown", onKeyDown);
      previouslyFocused?.focus?.();
    };
  }, []);

  const copySelectedId = async () => {
    if (!selectedId) return;
    try {
      if (!navigator.clipboard) throw new Error("Clipboard access is unavailable.");
      await navigator.clipboard.writeText(selectedId);
      setCopyFeedback({ text: "Record ID copied.", error: false });
    } catch (clipboardError) {
      setCopyFeedback({ text: `Could not copy the record ID: ${String(clipboardError)}`, error: true });
    }
  };

  return (
    <motion.aside
      ref={asideRef}
      className="inspector"
      role="dialog"
      aria-modal={false}
      aria-label="Inspector"
      initial={props.reduceMotion ? false : { opacity: 0, x: 18 }}
      animate={{ opacity: 1, x: 0 }}
      exit={props.reduceMotion ? undefined : { opacity: 0, x: 18 }}
      transition={transition.quick}
    >
      <header>
        <span>Inspector</span>
        <button
          ref={closeButtonRef}
          className="icon-button"
          type="button"
          title="Hide inspector (Esc)"
          aria-label="Hide inspector"
          onClick={props.onClose}
        >
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
            {selectedId ? <button onClick={() => void copySelectedId()}>Copy ID</button> : null}
          </div>
          {copyFeedback ? (
            <p role={copyFeedback.error ? "alert" : "status"} aria-live="polite">
              {copyFeedback.text}
            </p>
          ) : null}
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
        initial={props.reduceMotion ? false : { opacity: 0, y: 10 }}
        animate={{ opacity: 1, y: 0 }}
        exit={props.reduceMotion ? undefined : { opacity: 0 }}
        transition={transition.modal}
      >
        <header>
          <strong>{config.title}</strong>
          <button className="icon-button" onClick={props.onClose} title="Close" aria-label="Close">
            <X size={16} />
          </button>
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
  subtitle?: string;
  icon: typeof Activity;
  loading: boolean;
  onRefresh: () => void;
}) {
  const Icon = props.icon;
  return (
    <section className="memory-list-header">
      <div className="memory-list-header-text">
        <div className="memory-list-header-title">
          <Icon size={16} />
          <strong>{props.title}</strong>
        </div>
        {props.subtitle ? <p>{props.subtitle}</p> : null}
      </div>
      <button className="icon-button" onClick={props.onRefresh} title="Refresh records">
        <RefreshCcw size={15} className={props.loading ? "spin" : ""} />
      </button>
    </section>
  );
}

function EmptyRecordList({ text, action }: { text: string; action?: React.ReactNode }) {
  return (
    <div className="empty-record-list">
      <CircleDot size={13} />
      <span>{text}</span>
      {action}
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
      pending
        ? `${pending} pending ${pending === 1 ? "memory" : "memories"}`
        : `${group.candidates.length} ${group.candidates.length === 1 ? "memory" : "memories"}`,
      dayLabel,
      noisy ? `${noisy} low-signal` : null,
    ]
      .filter(Boolean)
      .join(" · ");
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

function confidenceTier(confidence: number): { label: string; tone: "low" | "med" | "high" } {
  if (confidence >= 0.75) return { label: "High", tone: "high" };
  if (confidence >= 0.5) return { label: "Medium", tone: "med" };
  return { label: "Low", tone: "low" };
}

// Confidence with visual weight: a Low/Medium/High label + a mini threshold bar
// (Low painted red) so a reviewer can scan the queue for weak items at a glance.
function ConfidenceChip({ confidence }: { confidence: number }) {
  const tier = confidenceTier(confidence);
  const pct = Math.round(confidence * 100);
  return (
    <span className={`confidence-chip conf-${tier.tone}`} title={`${tier.label} confidence · ${pct}%`}>
      <span className="confidence-bar">
        <span style={{ width: `${pct}%` }} />
      </span>
      {tier.label} · {pct}%
    </span>
  );
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
