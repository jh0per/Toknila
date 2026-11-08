// How statuses are grouped for display. The finer status stays visible as small text.

import type { TicketStatus } from "../api/types";

export type StatusGroup = "awaiting_approval" | "todo" | "in_progress" | "blocked" | "done" | "cancelled";

/// The group a status is shown in.
export function statusGroup(status: TicketStatus): StatusGroup {
  switch (status) {
    case "proposed":
      return "awaiting_approval";
    case "backlog":
    case "ready":
      return "todo";
    case "running":
    case "waiting":
    case "review":
      return "in_progress";
    case "blocked":
      return "blocked";
    case "merged":
    case "released":
      return "done";
    case "cancelled":
      return "cancelled";
  }
}

const GROUP_WORD: Record<StatusGroup, string> = {
  awaiting_approval: "awaiting approval",
  todo: "todo",
  in_progress: "in progress",
  blocked: "blocked",
  done: "done",
  cancelled: "cancelled",
};

/// The one line a card shows for its status: the group, then the finer status when it adds
/// something. A waiting card is in progress, not blocked: it is waiting for a person.
export function statusText(status: TicketStatus): string {
  const group = GROUP_WORD[statusGroup(status)];
  return group === status ? group : `${group} · ${status}`;
}
