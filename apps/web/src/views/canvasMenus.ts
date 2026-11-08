// The entries of the diagram's right-click menus, in groups. What a card allows is the daemon's
// call (`allowed_actions`, `can_edit`, `can_move` of its detail), worded by `specsFor`; nothing is
// derived here. At most one entry is red, and it is last.

import type { TicketAction, TicketDetail } from "../api/types";
import type { MenuEntry } from "./ContextMenu";
import { specsFor } from "./actionLabels";
import { menuGroups } from "./menuGroups";

/// Actions that never appear as plain entries: Cancel is the last group's, Block needs a reason
/// the menu has no field for.
const SKIPPED: TicketAction[] = ["cancel", "block"];

/// Shortcuts of the card actions that have one on the diagram (S starts the card in focus).
const KEYS: Partial<Record<TicketAction, string>> = { start: "S" };

export interface CardMenuHandlers {
  open: () => void;
  openTerminal: () => void;
  edit: () => void;
  act: (action: TicketAction) => void;
  /// Rejecting and returning ask for the person's words first: this opens the question.
  ask: (action: "reject" | "return") => void;
  addAfter: () => void;
  addBefore: () => void;
  /// Pick the card this one should wait for.
  addDependency: () => void;
  /// Cancelling asks first: this opens the question.
  askCancel: () => void;
}

/// The menu of a card: Open, Edit, Open terminal | what the daemon allows | Add card before, Add
/// card after, Add dependency | Cancel. `detail` is the daemon's latest word on the card; before
/// it is known the menu offers only what needs no state.
export function cardEntries(detail: TicketDetail | undefined, h: CardMenuHandlers): MenuEntry[] {
  const allowed = detail?.allowed_actions ?? [];
  const specs = specsFor(allowed);
  const open: MenuEntry[] = [{ label: "Open", testId: "menu-card-open", keys: "Enter", run: h.open }];
  if (detail?.can_edit) open.push({ label: "Edit", testId: "menu-card-edit", run: h.edit });
  open.push({ label: "Open terminal", testId: "menu-card-terminal", run: h.openTerminal });

  const actions: MenuEntry[] = [];
  for (const action of allowed) {
    if (SKIPPED.includes(action)) continue;
    const testId = `menu-card-${action}`;
    if (action === "reject") {
      actions.push({ label: "Reject…", testId, run: () => h.ask("reject") });
      continue;
    }
    const spec = specs.find((s) => s.action === action);
    if (!spec) continue;
    const entry = { label: spec.label, testId, keys: KEYS[action] };
    actions.push(
      spec.needsText
        ? { ...entry, label: `${spec.label}…`, run: () => h.ask(action as "return") }
        : { ...entry, run: () => h.act(action) },
    );
  }

  const around: MenuEntry[] = [];
  if (detail?.can_move) around.push({ label: "Add card before", testId: "menu-card-add-before", run: h.addBefore });
  around.push({ label: "Add card after", testId: "menu-card-add-after", run: h.addAfter });
  if (detail?.can_move) around.push({ label: "Add dependency…", testId: "menu-card-add-dependency", run: h.addDependency });

  const cancel = specs.find((s) => s.action === "cancel");
  const last: MenuEntry[] = cancel ? [{ label: cancel.label, testId: "menu-card-cancel", danger: true, run: h.askCancel }] : [];
  return menuGroups(open, actions, around, last);
}

export interface CanvasMenuHandlers {
  newCard: () => void;
  newChain: () => void;
  importPlan: () => void;
  askPlanner: () => void;
}

/// The menu of empty canvas: New card | New chain, Import a plan | Plan with the assistant.
export function canvasEntries(h: CanvasMenuHandlers): MenuEntry[] {
  return menuGroups(
    [{ label: "New card", testId: "menu-canvas-new-card", keys: "N", run: h.newCard }],
    [
      { label: "New chain", testId: "menu-canvas-new-chain", run: h.newChain },
      { label: "Import a plan", testId: "menu-canvas-import", run: h.importPlan },
    ],
    [{ label: "Plan with the assistant", testId: "menu-canvas-planner", run: h.askPlanner }],
  );
}

/// The menu of a line between two cards, titled with what it joins ("TOK-1 → TOK-2").
export function edgeMenu(from: string, to: string, remove: () => void): { title: string; entries: MenuEntry[] } {
  return {
    title: `${from} → ${to}`,
    entries: [{ label: "Remove dependency", testId: "menu-remove-edge", keys: "Delete", danger: true, run: remove }],
  };
}
