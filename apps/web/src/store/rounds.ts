// The review history of a card, read from the journal: a return is the move Review -> Running,
// and the comment the person wrote came with it as an instruction comment right beside it.

import type { EventRecord } from "../api/types";
import { ticketOfEvent } from "./state";

export interface Round {
  /// The round the card entered by this return (the first review is round 1).
  round: number;
  seq: number;
  at: number;
  comment: string;
}

/// How far from the status change (in journal entries) its comment can sit.
const NEAR = 2;

type Payload = Record<string, Record<string, unknown> | string | undefined>;

function returned(e: EventRecord, ticket: string): boolean {
  const p = (e.payload ?? {}) as Payload;
  return e.entity === "ticket" && e.kind === "status_changed" && p.from === "review" && p.to === "running"
    && ticketOfEvent(e) === ticket;
}

function instruction(e: EventRecord, ticket: string): string | undefined {
  if (e.entity !== "comment" || ticketOfEvent(e) !== ticket) return undefined;
  const c = ((e.payload ?? {}) as Payload).comment as { kind?: string; body?: { gist?: string } } | undefined;
  return c?.kind === "instruction" ? c.body?.gist : undefined;
}

/// The returns of one ticket, oldest first.
export function reviewRounds(events: EventRecord[], ticket: string): Round[] {
  const rounds: Round[] = [];
  for (const e of events) {
    if (!returned(e, ticket)) continue;
    const near = events
      .filter((c) => Math.abs(c.seq - e.seq) <= NEAR && instruction(c, ticket) !== undefined)
      .sort((a, b) => Math.abs(a.seq - e.seq) - Math.abs(b.seq - e.seq))[0];
    rounds.push({
      round: rounds.length + 2,
      seq: e.seq,
      at: e.at,
      comment: near ? (instruction(near, ticket) ?? "") : "",
    });
  }
  return rounds;
}
