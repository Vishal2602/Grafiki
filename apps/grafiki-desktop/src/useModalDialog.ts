import { useEffect, useRef } from "react";

const FOCUSABLE =
  'a[href], button:not([disabled]), textarea:not([disabled]), input:not([disabled]), select:not([disabled]), [tabindex]:not([tabindex="-1"])';

/**
 * Focus management for a modal dialog (ARIA APG dialog pattern): move focus into
 * the panel on open, trap Tab/Shift+Tab within it, close on Escape, and restore
 * focus to the triggering element on unmount. Returns a ref to attach to the
 * dialog panel element (which should also carry role="dialog", aria-modal="true",
 * an aria-label, and tabIndex={-1}). No dependencies.
 */
export function useModalDialog<T extends HTMLElement = HTMLElement>(
  onClose: () => void,
) {
  const ref = useRef<T>(null);
  const onCloseRef = useRef(onClose);
  onCloseRef.current = onClose;

  // Capture the triggering element during RENDER (before React commits the modal
  // DOM and before any autoFocus runs in the commit phase) so focus can be
  // restored to it on close. Reading document.activeElement here is a one-time
  // snapshot, guarded so it only happens on the first render.
  const triggerRef = useRef<HTMLElement | null>(null);
  if (triggerRef.current === null) {
    triggerRef.current = document.activeElement as HTMLElement | null;
  }

  // Run once on mount: focus into the panel and bind the trap.
  // eslint-disable-next-line react-hooks/exhaustive-deps
  useEffect(() => {
    const panel = ref.current;
    const trigger = triggerRef.current;

    const focusables = (): HTMLElement[] =>
      panel
        ? Array.from(panel.querySelectorAll<HTMLElement>(FOCUSABLE)).filter(
            (el) => el.offsetParent !== null,
          )
        : [];

    // Respect an autoFocus'd field already inside the panel; otherwise move
    // focus to the first focusable (falling back to the panel itself).
    if (!panel || !panel.contains(document.activeElement)) {
      const first = focusables()[0];
      if (first) first.focus();
      else panel?.focus();
    }

    function onKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") {
        event.preventDefault();
        onCloseRef.current();
        return;
      }
      if (event.key !== "Tab" || !panel) return;
      const items = focusables();
      if (items.length === 0) {
        event.preventDefault();
        panel.focus();
        return;
      }
      const firstEl = items[0];
      const lastEl = items[items.length - 1];
      const active = document.activeElement;
      if (event.shiftKey && active === firstEl) {
        event.preventDefault();
        lastEl.focus();
      } else if (!event.shiftKey && active === lastEl) {
        event.preventDefault();
        firstEl.focus();
      }
    }

    document.addEventListener("keydown", onKeyDown, true);
    return () => {
      document.removeEventListener("keydown", onKeyDown, true);
      trigger?.focus?.();
    };
  }, []);

  return ref;
}
