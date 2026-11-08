// What the diagram's cards read from a card's detail: how far its subtasks are, and, for a
// proposed card, the buttons the daemon allows. The chain read model has neither, so the page
// asks for the details of the cards that need them (the ones that are proposed or at work) and
// keeps them fresh from the journal; every rule stays the daemon's.

import type { ChainDetail, TicketAction, TicketDetail, TicketStatus } from "../api/types";
import type { Effect } from "./msg";
import type { State } from "./state";

/// Statuses of cards whose subtasks and allowed actions the diagram shows without being opened.
const SHOWN: TicketStatus[] = ["proposed", "running", "waiting", "review"];

/// How many subtasks are done, or nothing for a card without any.
export function subtaskProgress(detail: TicketDetail | undefined): { done: number; total: number } | undefined {
  const all = detail?.subtasks ?? [];
  return all.length ? { done: all.filter((s) => s.done).length, total: all.length } : undefined;
}

/// Whether the daemon allows `action` on the card, going by the last detail it sent.
export const allows = (detail: TicketDetail | undefined, action: TicketAction): boolean =>
  detail?.allowed_actions.includes(action) ?? false;

/// The details to fetch for the cards of `chains` that the diagram shows more of: the ones the
/// page has not read yet, or all of them after a full reload (`again`).
export function detailsWanted(s: State, chains: ChainDetail[], again = false): Effect[] {
  return chains
    .flatMap((c) => c.tickets)
    .filter((t) => SHOWN.includes(t.status) && (again || !s.details[t.key]))
    .map((t): Effect => ({ type: "loadDetail", key: t.key }));
}

/// The key of the card with this ticket id, if the page has read its detail.
export const knownKey = (s: State, ticketId: string | undefined): string | undefined =>
  ticketId ? Object.values(s.details).find((d) => d.ticket.id === ticketId)?.ticket.key : undefined;
