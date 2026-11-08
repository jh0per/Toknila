// How statuses are grouped for display. The finer status stays visible as small text.

import type { ChainStatus, TicketStatus } from "../api/types";

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

/// How a thing reads at a glance: nothing needed (`todo`), work going on (`prog`), a person
/// needed (`attn`), finished (`done`), set aside (`cancel`). The stylesheet colours the classes
/// `tone-*` and, per fine status, `s-<status>` (see `statusClass`).
export type Tone = "todo" | "prog" | "attn" | "done" | "cancel";

/// The class of a card for its fine status. Waiting, review, blocked and proposed share a colour
/// (a person is needed) but each has its own class.
export const statusClass = (status: TicketStatus): string => `s-${status}`;

/// The tone of a chain's computed status, for its pill in the top bar.
export function chainTone(status: ChainStatus): Tone {
  switch (status) {
    case "running":
      return "prog";
    case "blocked":
    case "needs_human":
      return "attn";
    case "done":
      return "done";
    case "empty":
    case "not_started":
    case "idle":
      return "todo";
  }
}
