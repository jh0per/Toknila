import { useEffect } from "react";
import type { Intent } from "../store/msg";
import type { State } from "../store/state";

const TYPING = "input, textarea, select, [contenteditable=true], [role=dialog], .ctx-menu, .quick-card";

/// Keys of the diagram: N opens the new-card field; S starts the card in focus (the focused card,
/// else the open one) when the daemon says it may start. Enter on a focused card is the link's
/// own: it opens the card. Nothing is taken while a text field has focus or a dialog is open.
export function useCanvasKeys(state: State, dispatch: (i: Intent) => void, openField: () => void): void {
  useEffect(() => {
    const key = (ev: KeyboardEvent) => {
      if (ev.defaultPrevented || ev.ctrlKey || ev.metaKey || ev.altKey || state.plan.dialog) return;
      if ((ev.target as HTMLElement | null)?.closest(TYPING)) return;
      const k = ev.key.toLowerCase();
      if (k === "n") {
        ev.preventDefault();
        openField();
      } else if (k === "s") {
        const focused = (document.activeElement as HTMLElement | null)?.closest<HTMLElement>("a.node")?.dataset.key;
        const target = focused ?? (state.route.name === "card" ? state.route.key : undefined);
        if (!target || !state.details[target]?.allowed_actions.includes("start")) return;
        ev.preventDefault();
        dispatch({ type: "ticketAction", key: target, action: "start" });
      }
    };
    window.addEventListener("keydown", key);
    return () => window.removeEventListener("keydown", key);
  });
}
