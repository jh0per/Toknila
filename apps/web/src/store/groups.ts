// How statuses are grouped for display. The finer status stays visible as small text.

import type { TicketStatus } from "../api/types";

export type StatusGroup = "awaiting_approval" | "todo" | "in_progress" | "blocked" | "done" | "cancelled";

/// Display order of the groups; a proposed card waits for the owner first, cancelled is shown apart, last.
export const GROUP_ORDER: StatusGroup[] = ["awaiting_approval", "todo", "in_progress", "blocked", "done", "cancelled"];

export const GROUP_LABEL: Record<StatusGroup, string> = {
  awaiting_approval: "awaiting approval",
  todo: "To do",
  in_progress: "In progress",
  blocked: "Blocked",
  done: "Done",
  cancelled: "Cancelled",
};

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

/// Items split by group, every group present, order inside a group kept.
export function groupBy<T>(items: T[], status: (item: T) => TicketStatus): Record<StatusGroup, T[]> {
  const out: Record<StatusGroup, T[]> = { awaiting_approval: [], todo: [], in_progress: [], blocked: [], done: [], cancelled: [] };
  for (const i of items) out[statusGroup(status(i))].push(i);
  return out;
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
