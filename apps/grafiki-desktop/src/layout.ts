import type { LayoutState, PaneState } from "./types";

export const defaultPanes: PaneState[] = [
  { id: "home", kind: "home", title: "Home" },
];

const STORAGE_KEY = "grafiki.desktop.layout";
const LAYOUT_VERSION = 3;

// Known pane kinds (must match PaneKind in types.ts). Used to reject panes from a
// tampered/old URL hash before they reach the renderer.
const PANE_KINDS: ReadonlySet<string> = new Set([
  "home",
  "session",
  "chat",
  "terminal",
  "candidates",
  "settings",
  "detail",
]);

export function createDefaultLayout(): LayoutState {
  return {
    activePaneId: defaultPanes[0].id,
    panes: defaultPanes,
  };
}

export function loadInitialLayout(): LayoutState {
  // Always start at Home on a fresh launch — the ledger is the app's front
  // door (docs/UX_REDESIGN.md §4). A resumable/live session is offered there
  // via the Resume banner and live-session card, never as a silent
  // full-screen takeover. Before this fix the app reopened directly into
  // whatever pane (often the live terminal) was last active, so launch and
  // the "Sessions" nav item showed the identical screen with no explanation
  // (2026-07-04 don-norman-design-critic: "no idea what to do and where to
  // look"). Mid-session navigation still persists via persistLayout below —
  // this only overrides what a fresh process boot opens to.
  return createDefaultLayout();
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

export function titleForPane(pane: Pick<PaneState, "kind" | "recordId">) {
  if (pane.kind === "detail") return pane.recordId ? `Detail: ${pane.recordId}` : "Detail";
  if (pane.kind === "candidates") return "Review";
  if (pane.kind === "session") return "Session";
  if (pane.kind === "chat") return "Memory";
  if (pane.kind === "terminal") return "Session";
  const kind = pane.kind ?? "";
  if (!kind) return "Pane";
  return kind[0].toUpperCase() + kind.slice(1);
}

export function newPaneId(kind: string) {
  return `${kind}-${Math.random().toString(36).slice(2, 8)}`;
}
