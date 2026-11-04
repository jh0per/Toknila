// How statuses are grouped for display. The finer status stays visible as small text.

import type { TicketStatus } from "../api/types";

export type StatusGroup = "todo" | "in_progress" | "blocked" | "done" | "cancelled";

/// Display order of the groups; cancelled is shown apart, last.
export const GROUP_ORDER: StatusGroup[] = ["todo", "in_progress", "blocked", "done", "cancelled"];

export const GROUP_LABEL: Record<StatusGroup, string> = {
  todo: "To do",
  in_progress: "In progress",
  blocked: "Blocked",
  done: "Done",
  cancelled: "Cancelled",
};

/// The group a status is shown in.
export function statusGroup(status: TicketStatus): StatusGroup {
  switch (status) {
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
  const out: Record<StatusGroup, T[]> = { todo: [], in_progress: [], blocked: [], done: [], cancelled: [] };
  for (const i of items) out[statusGroup(status(i))].push(i);
  return out;
}
