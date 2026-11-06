// Which card actions the window offers. The daemon's state machine decides what is allowed (a
// refused action comes back as an error); this only picks the buttons worth showing per status.

import type { Ticket } from "../api/types";

export type CardAction = "start" | "pause" | "resume" | "cancel" | "unblock" | "accept" | "return";

export interface ActionSpec {
  action: CardAction;
  label: string;
  /// The action takes the person's comment (a return says what is wrong).
  needsText?: boolean;
  danger?: boolean;
}

const SPEC: Record<CardAction, ActionSpec> = {
  start: { action: "start", label: "Start" },
  pause: { action: "pause", label: "Pause" },
  resume: { action: "resume", label: "Resume" },
  cancel: { action: "cancel", label: "Cancel", danger: true },
  unblock: { action: "unblock", label: "Unblock" },
  accept: { action: "accept", label: "Accept" },
  return: { action: "return", label: "Return", needsText: true },
};

/// The buttons for a card in its status, main action first.
export function actionsFor(t: Pick<Ticket, "status" | "paused">): ActionSpec[] {
  const names = (): CardAction[] => {
    switch (t.status) {
      case "backlog": return [t.paused ? "resume" : "pause", "cancel"];
      case "ready": return ["start", "pause", "cancel"];
      case "running":
      case "waiting": return ["pause", "cancel"];
      case "blocked": return ["unblock", "pause", "cancel"];
      case "review": return ["accept", "return", "cancel"];
      default: return [];
    }
  };
  return names().map((n) => SPEC[n]);
}
