import { useEffect } from "react";
import type { Intent } from "../store/msg";
import { asksForShortcuts } from "../store/shortcuts";
import type { State } from "../store/state";

/// Where a "?" is a character being typed, not a request (the terminal types into a textarea too).
const TEXT_FIELD = "input, textarea, select, [contenteditable=true]";

/// "?" outside text fields opens the list of keyboard shortcuts, and closes it again. It stays out
/// of the way of another dialog or a menu that is open.
export function useShortcutsKey(state: State, dispatch: (i: Intent) => void): void {
  const open = state.shortcutsOpen;
  const busy = Boolean(state.plan.dialog);
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const target = e.target as HTMLElement | null;
      const inText = Boolean(target?.closest(TEXT_FIELD));
      if (e.defaultPrevented || !asksForShortcuts(e, inText)) return;
      if (!open && (busy || target?.closest(".ctx-menu, [role=dialog]"))) return;
      e.preventDefault();
      dispatch({ type: "toggleShortcuts" });
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [dispatch, open, busy]);
}
