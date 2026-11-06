// How each card action looks as a button. Which actions a card allows is the daemon's call
// (`TicketDetail.allowed_actions`, computed by the state machine); this only words and styles them.

import type { TicketAction } from "../api/types";

export interface ActionSpec {
  action: TicketAction;
  label: string;
  /// The action takes the person's comment (a return says what is wrong).
  needsText?: boolean;
  danger?: boolean;
}

const LABEL: Record<TicketAction, Omit<ActionSpec, "action">> = {
  start: { label: "Start" },
  pause: { label: "Pause" },
  resume: { label: "Resume" },
  block: { label: "Block" },
  unblock: { label: "Unblock" },
  accept: { label: "Accept" },
  return: { label: "Return", needsText: true },
  reopen: { label: "Reopen" },
  cancel: { label: "Cancel", danger: true },
  approve: { label: "Approve" },
  reject: { label: "Reject", danger: true },
};

/// Actions that need a reason or a text the bar has no field for; the daemon would refuse them.
const NOT_IN_THE_BAR: TicketAction[] = ["block", "reject"];

/// The buttons for the actions the daemon allows, main action first as the daemon lists them.
export function specsFor(allowed: TicketAction[]): ActionSpec[] {
  return allowed.filter((a) => !NOT_IN_THE_BAR.includes(a)).map((action) => ({ action, ...LABEL[action] }));
}
