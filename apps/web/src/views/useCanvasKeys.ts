import { useEffect } from "react";
import type { Intent } from "../store/msg";
import type { State } from "../store/state";
import { GRID } from "./canvasGeometry";

const TYPING = "input, textarea, select, [contenteditable=true], [role=dialog], .ctx-menu, .quick-card";

const ARROWS: Record<string, [number, number]> = {
  ArrowLeft: [-1, 0], ArrowRight: [1, 0], ArrowUp: [0, -1], ArrowDown: [0, 1],
};

/// Keys of the diagram: N opens the new-card field; S starts the card in focus (the focused card,
/// else the open one) when the daemon says it may start; the arrow keys move the open card by one
/// grid step, with Shift by five (`nudge` gets the distance in canvas pixels). Enter on a focused
/// card is the link's own: it opens the card. Nothing is taken while a text field has focus or a
/// dialog is open.
export function useCanvasKeys(
  state: State,
  dispatch: (i: Intent) => void,
  openField: () => void,
  nudge: (dx: number, dy: number) => void,
): void {
  useEffect(() => {
    const key = (ev: KeyboardEvent) => {
      if (ev.defaultPrevented || ev.ctrlKey || ev.metaKey || ev.altKey || state.plan.dialog) return;
      if ((ev.target as HTMLElement | null)?.closest(TYPING)) return;
      const arrow = ARROWS[ev.key];
      if (arrow) {
        if (state.route.name !== "card") return;
        ev.preventDefault();
        const step = (ev.shiftKey ? 5 : 1) * GRID;
        nudge(arrow[0] * step, arrow[1] * step);
        return;
      }
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
