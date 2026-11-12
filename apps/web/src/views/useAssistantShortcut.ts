import { useEffect } from "react";
import { beside } from "../store/assistant";
import type { Intent } from "../store/msg";
import type { State } from "../store/state";

/// Ctrl+. (Cmd+. on a Mac) opens and closes the assistant panel on the roadmap, a chain and a card.
export function useAssistantShortcut(state: State, dispatch: (i: Intent) => void): void {
  const here = beside(state.route);
  useEffect(() => {
    if (!here) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "." || !(e.metaKey || e.ctrlKey) || e.altKey || e.shiftKey) return;
      e.preventDefault();
      dispatch({ type: "toggleAssistant" });
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [dispatch, here]);
}
