// The entries of the diagram's right-click menus. What a card allows is the daemon's call
// (`allowed_actions`, `can_move` of its detail), worded by `specsFor`; nothing is derived here.

import type { TicketAction, TicketDetail } from "../api/types";
import type { MenuEntry } from "./ContextMenu";
import { specsFor } from "./actionLabels";

/// The card actions the diagram's menu offers, in this order, when the daemon allows them.
const MENU_ACTIONS: TicketAction[] = ["start", "pause", "resume"];

export interface CardMenuHandlers {
  open: () => void;
  act: (action: TicketAction) => void;
  addAfter: () => void;
  addBefore: () => void;
  /// Cancelling asks first: this opens the question.
  askCancel: () => void;
}

/// The menu of a card. `detail` is the daemon's latest word on it; before it is known the menu
/// offers only what needs no state.
export function cardEntries(detail: TicketDetail | undefined, h: CardMenuHandlers): MenuEntry[] {
  const specs = specsFor(detail?.allowed_actions ?? []);
  const entries: MenuEntry[] = [{ label: "Open", testId: "menu-card-open", run: h.open }];
  for (const a of MENU_ACTIONS) {
    const spec = specs.find((s) => s.action === a);
    if (spec) entries.push({ label: spec.label, testId: `menu-card-${a}`, run: () => h.act(a) });
  }
  entries.push({ label: "Add card after", testId: "menu-card-add-after", run: h.addAfter });
  if (detail?.can_move) entries.push({ label: "Add card before", testId: "menu-card-add-before", run: h.addBefore });
  const cancel = specs.find((s) => s.action === "cancel");
  if (cancel) entries.push({ label: cancel.label, testId: "menu-card-cancel", danger: true, run: h.askCancel });
  return entries;
}

export interface CanvasMenuHandlers {
  newCard: () => void;
  newChain: () => void;
  importPlan: () => void;
  askPlanner: () => void;
}

/// The menu of empty canvas.
export function canvasEntries(h: CanvasMenuHandlers): MenuEntry[] {
  return [
    { label: "New card", testId: "menu-canvas-new-card", run: h.newCard },
    { label: "New chain", testId: "menu-canvas-new-chain", run: h.newChain },
    { label: "Import a plan", testId: "menu-canvas-import", run: h.importPlan },
    { label: "Ask the planner", testId: "menu-canvas-planner", run: h.askPlanner },
  ];
}
