import type { LayoutState, PaneState } from "./types";

export const defaultPanes: PaneState[] = [
  { id: "overview", kind: "overview", title: "Overview" },
];

const STORAGE_KEY = "grafiki.desktop.layout";
const LAYOUT_VERSION = 2;

// Known pane kinds (must match PaneKind in types.ts). Used to reject panes from a
// tampered/old URL hash before they reach the renderer.
const PANE_KINDS: ReadonlySet<string> = new Set([
  "overview",
  "search",
  "chat",
  "graph",
  "candidates",
  "relations",
  "sessions",
  "state",
  "decisions",
  "context",
  "settings",
  "detail",
  "capture",
  "agent",
]);

export function createDefaultLayout(): LayoutState {
  return {
    activePaneId: defaultPanes[0].id,
    panes: defaultPanes,
  };
}

export function loadInitialLayout(): LayoutState {
  return (
    decodeLayoutFromHash(window.location.hash) ??
    decodeLayout(localStorage.getItem(STORAGE_KEY)) ??
    createDefaultLayout()
  );
}

export function persistLayout(layout: LayoutState) {
  const encoded = encodeLayout(layout);
  localStorage.setItem(STORAGE_KEY, encoded);
  const nextHash = `#/app/panes/${encoded}`;
  if (window.location.hash !== nextHash) {
    window.history.replaceState(null, "", nextHash);
  }
}

export function decodeLayoutFromHash(hash: string): LayoutState | null {
  const marker = "#/app/panes/";
  if (!hash.startsWith(marker)) return null;
  return decodeLayout(hash.slice(marker.length));
}

export function encodeLayout(layout: LayoutState): string {
  return encodeURIComponent(
    JSON.stringify({
      version: LAYOUT_VERSION,
      activePaneId: layout.activePaneId,
      panes: layout.panes,
    }),
  );
}

export function decodeLayout(value: string | null): LayoutState | null {
  if (!value) return null;

  try {
    const parsed = JSON.parse(decodeURIComponent(value)) as LayoutState & { version?: number };
    if (parsed.version !== LAYOUT_VERSION) return null;
    if (!Array.isArray(parsed.panes) || parsed.panes.length === 0) return null;
    const panes = parsed.panes
      .filter(
        (pane) =>
          typeof pane.id === "string" &&
          typeof pane.kind === "string" &&
          PANE_KINDS.has(pane.kind),
      )
      .map((pane) => ({
        ...pane,
        title: pane.title || titleForPane(pane),
      }));
    if (panes.length === 0) return null;
    const activePaneId = panes.some((pane) => pane.id === parsed.activePaneId)
      ? parsed.activePaneId
      : panes[0].id;
    return { activePaneId, panes };
  } catch {
    return null;
  }
}

export function titleForPane(pane: Pick<PaneState, "kind" | "query" | "recordId" | "captureType" | "recordType">) {
  if (pane.kind === "search") {
    const type = pane.recordType && pane.recordType !== "all" ? `${pane.recordType} ` : "";
    return pane.query ? `Search: ${type}${pane.query}` : "Search";
  }
  if (pane.kind === "detail") return pane.recordId ? `Detail: ${pane.recordId}` : "Detail";
  if (pane.kind === "capture") return pane.captureType ? `New ${pane.captureType}` : "Capture";
  if (pane.kind === "candidates") return "Memory Review";
  if (pane.kind === "agent") return "Agent Activity";
  const kind = pane.kind ?? "";
  if (!kind) return "Pane";
  return kind[0].toUpperCase() + kind.slice(1);
}

export function newPaneId(kind: string) {
  return `${kind}-${Math.random().toString(36).slice(2, 8)}`;
}
