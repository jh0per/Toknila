// What the window says about the assistant's turn, the same in the chat, the ask boxes outside it
// and the top bar: the daemon's turn (`working`, `step`, `since` of the planner slice), the
// `planner.turn` journal entries, and the conversation read as `openTurn` reads it (the message
// awaited is the oldest one without an answer). Pure.
//
// - A message on its way, or a turn on: the waiting line, with the step beside the minutes; no
//   Nudge however long the turn takes.
// - A turn that started after the awaited message was sent and ended without answering it:
//   "stopped without answering", and Nudge at once.
// - A message no turn started for (it did not reach the assistant): the waiting line, and Nudge
//   after `ANSWER_WAIT_MS`.

import type { ConversationMessage, EventRecord, PlannerState } from "../api/types";
import { lineFor } from "./assistant";
import { conversationEvent, msUntilOverdue, openTurn, unanswered } from "./conversation";
import type { Effect } from "./msg";
import { type PlannerIntent, applyTurn } from "./planner";
import type { State } from "./state";

/// The assistant's last turn, once it ended: when it started (when the journal has it) and ended.
export interface EndedTurn {
  start?: number;
  end: number;
}

/// The turn as the chat and the ask boxes show it.
export interface TurnView {
  /// The waiting line shows.
  waiting: boolean;
  /// When the waiting line counts from: when the awaited message's wait began, else the turn's start.
  since: number;
  /// What the assistant does now, while its turn is on.
  step?: string;
  /// The awaited message's turn ended without an answer to it.
  ended: boolean;
  /// No turn started for the person's last message within the wait: Nudge is offered.
  late: boolean;
  /// The message Nudge sends again.
  owed?: ConversationMessage;
}

/// What the turn looks like: `p` is the daemon's planner, `last` its last turn when it ended (see
/// `lastTurnEnded`), `sending` whether a message is on its way, `now` the clock, `resent` the
/// messages the person sent again (`openTurn`).
export function turnView(
  messages: ConversationMessage[], p: PlannerState | undefined, last: EndedTurn | undefined, sending: boolean, now: number, resent: string[] = [],
): TurnView {
  const running = p?.running ?? false;
  const working = running && (p?.working ?? false);
  const turn = openTurn(messages, p, resent);
  const asked = turn.asked;
  // The ended turn was the awaited message's: it started after the message was sent (a turn that
  // was on when it was sent was another message's, and its own turn is still to come).
  const over = asked !== undefined && last !== undefined && (last.start ?? last.end) >= asked.at;
  return {
    waiting: running && (sending || working || (asked !== undefined && !over)),
    since: turn.since ?? p?.since ?? now,
    step: working ? (p?.step ?? undefined) : undefined,
    ended: running && !working && !sending && over,
    late: running && !working && !sending && !over && msUntilOverdue(messages, now) === 0,
    owed: over ? asked : unanswered(messages),
  };
}

const isTurn = (e: EventRecord, id: string): boolean => e.entity === "planner" && e.kind === "turn" && e.entity_id === id;
const working = (e: EventRecord): boolean => Boolean((e.payload as { working?: boolean }).working);

/// The project's assistant's last turn as the journal has it, when it ended; none while the latest
/// turn is on or none was journaled.
export function lastTurnEnded(s: State, project: string): EndedTurn | undefined {
  const id = s.projects.find((p) => p.key === project)?.id;
  if (!id) return undefined;
  let end: number | undefined;
  for (let i = s.events.length - 1; i >= 0; i--) {
    const e = s.events[i];
    if (!isTurn(e, id)) continue;
    if (end === undefined) {
      if (working(e)) return undefined;
      end = e.at;
    } else if (working(e)) return { start: e.at, end };
  }
  return end === undefined ? undefined : { end };
}

/// The words of the Assistant button's and the sidebar row's title while the assistant's turn is
/// on; none otherwise.
export const workingTitle = (p: PlannerState | undefined): string | undefined =>
  p?.running && p.working ? (p.step ? `The assistant is working: ${p.step}` : "The assistant is working") : undefined;

/// What Nudge sends: the unanswered message again, through the same line it was first typed as.
export function nudgeFor(project: string, owed: ConversationMessage): PlannerIntent {
  const about = owed.about;
  if (about?.kind === "card" && owed.thread) return { type: "nudgeCard", project, key: about.key, thread: owed.thread, text: owed.text };
  return { type: "nudgeChat", project, text: about?.kind === "chain" || about?.kind === "roadmap" ? lineFor(about, owed.text) : owed.text };
}

/// A journal entry of the assistant's: a message of its conversation, or its turn starting or
/// ending (`planner.turn`, which changes `working` at once). A gap in `seq` reloads.
export function assistantEvent(s: State, e: EventRecord, gap: boolean): [State, Effect[]] {
  if (e.entity !== "planner") return conversationEvent(s, e, gap);
  const fx: Effect[] = gap ? [{ type: "loadAll" }] : [];
  const key = s.projects.find((p) => p.id === e.entity_id)?.key;
  const on = (e.payload as { working?: unknown }).working;
  if (e.kind !== "turn" || !key || typeof on !== "boolean") return [s, fx];
  const [next, more] = applyTurn(s, key, on, e.at);
  return [next, [...fx, ...more]];
}
