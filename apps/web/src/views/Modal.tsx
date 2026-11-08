import { type ReactNode, useEffect, useRef } from "react";

const FOCUSABLE = [
  "a[href]", "button:not([disabled])", "input:not([disabled]):not([type=hidden])", "select:not([disabled])",
  "textarea:not([disabled])", "summary", "[tabindex]:not([tabindex='-1'])",
].join(",");

/// What inside `root` can take the focus and is on screen, in tab order.
function focusables(root: HTMLElement): HTMLElement[] {
  return [...root.querySelectorAll<HTMLElement>(FOCUSABLE)].filter((el) => el.getClientRects().length > 0);
}

/// A dialog over the window. Escape, the close button and a click beside it close it; the form
/// inside brings its own heading. Focus moves into the dialog when it opens (to the field the form
/// asked for, else the first control), Tab and Shift+Tab stay inside it, and closing it returns
/// the focus to what had it before.
export function Modal({ label, testId, onClose, children }: {
  label: string;
  testId: string;
  onClose: () => void;
  children: ReactNode;
}) {
  const box = useRef<HTMLDivElement>(null);
  // What had the focus before the dialog: read while rendering, since a field with `autoFocus`
  // takes the focus when the dialog is put on the page, before any effect runs.
  const before = useRef<HTMLElement | null | undefined>(undefined);
  if (before.current === undefined) before.current = document.activeElement as HTMLElement | null;
  useEffect(() => {
    const back = before.current;
    const el = box.current;
    // A field that took the focus itself (autoFocus) keeps it; otherwise the first control gets it.
    if (el && !el.contains(document.activeElement)) (focusables(el).find((f) => f.dataset.testid !== "modal-close") ?? el).focus();
    return () => {
      if (back?.isConnected) back.focus();
    };
  }, []);
  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        onClose();
        return;
      }
      const el = box.current;
      if (e.key !== "Tab" || !el) return;
      const all = focusables(el);
      if (all.length === 0) {
        e.preventDefault();
        return;
      }
      const first = all[0];
      const last = all[all.length - 1];
      const at = document.activeElement;
      if (!el.contains(at)) {
        e.preventDefault();
        (e.shiftKey ? last : first).focus();
      } else if (e.shiftKey && at === first) {
        e.preventDefault();
        last.focus();
      } else if (!e.shiftKey && at === last) {
        e.preventDefault();
        first.focus();
      }
    };
    window.addEventListener("keydown", key);
    return () => window.removeEventListener("keydown", key);
  }, [onClose]);
  return (
    <div className="modal-back" onMouseDown={(e) => { if (e.target === e.currentTarget) onClose(); }}>
      <div className="modal" role="dialog" aria-modal="true" aria-label={label} data-testid={testId} ref={box} tabIndex={-1}>
        <button type="button" className="modal-close" aria-label="Close" data-testid="modal-close" onClick={onClose}>×</button>
        {children}
      </div>
    </div>
  );
}
