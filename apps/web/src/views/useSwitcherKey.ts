import { useEffect } from "react";
import type { Intent } from "../store/msg";
import { asksForSwitcher } from "../store/switcher";

/// Cmd/Ctrl+K opens the project switcher from anywhere, and closes it again. It works inside a
/// text field too (the key is a chord, not a character), but stays out of another dialog's way.
export function useSwitcherKey(dispatch: (i: Intent) => void, open: boolean, busy: boolean): void {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.defaultPrevented || !asksForSwitcher(e)) return;
      if (!open && (busy || (e.target as HTMLElement | null)?.closest(".ctx-menu, [role=dialog]"))) return;
      e.preventDefault();
      dispatch({ type: "toggleSwitcher" });
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [dispatch, open, busy]);
}
