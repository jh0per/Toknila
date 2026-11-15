// What the window says about the assistant's turn, the same in the chat, the ask boxes outside it
// and the top bar: the daemon's turn (`working`, `step`, `since` of the planner slice), the
// `planner.turn` journal entries, and the conversation. Pure.
//
// - A message on its way, or a turn on: the waiting line, with the step beside the minutes; no
//   Nudge however long the turn takes.
// - A turn that ended after the person's last message, which has no answer: "stopped without
//   answering", and Nudge at once.
// - A message no turn started for (it did not reach the assistant): the waiting line, and Nudge
//   after `ANSWER_WAIT_MS`.

import type { ConversationMessage, EventRecord, PlannerState } from "../api/types";
import { lineFor } from "./assistant";
import { conversationEvent, msUntilOverdue, openTurn, unanswered } from "./conversation";
import type { Effect } from "./msg";
import { type PlannerIntent, applyTurn } from "./planner";
import type { State } from "./state";

/// The turn as the chat and the ask boxes show it.
export interface TurnView {
  /// The waiting line shows.
  waiting: boolean;
  /// When the waiting line counts from: the message that started the turn, else the turn's start.
  since: number;
  /// What the assistant does now, while its turn is on.
  step?: string;
  /// The turn ended after the person's last message, which has no answer.
  stopped: boolean;
  /// No turn started for the person's last message within the wait: Nudge is offered.
  late: boolean;
  /// The person's message the assistant owes an answer to.
  owed?: ConversationMessage;
}

/// What the turn looks like: `p` is the daemon's planner, `ended` when its last turn ended (see
/// `turnEndedAt`), `sending` whether a message is on its way, `now` the clock.
export function turnView(messages: ConversationMessage[], p: PlannerState | undefined, ended: number | undefined, sending: boolean, now: number): TurnView {
  const running = p?.running ?? false;
  const working = running && (p?.working ?? false);
  const owed = unanswered(messages);
  // A turn ended after the message: it reached the assistant, which stopped without an answer.
  const over = owed !== undefined && ended !== undefined && ended >= owed.at;
  return {
    waiting: running && (sending || working || (owed !== undefined && !over)),
    since: openTurn(messages).asked?.at ?? p?.since ?? now,
    step: working ? (p?.step ?? undefined) : undefined,
    stopped: running && !working && !sending && over,
    late: running && !working && !sending && !over && msUntilOverdue(messages, now) === 0,
    owed,
  };
}

const isTurn = (e: EventRecord, id: string): boolean => e.entity === "planner" && e.kind === "turn" && e.entity_id === id;

/// When the project's assistant last ended a turn, as the journal says; none while the latest
/// turn is on or none was journaled.
export function turnEndedAt(s: State, project: string): number | undefined {
  const id = s.projects.find((p) => p.key === project)?.id;
  if (!id) return undefined;
  for (let i = s.events.length - 1; i >= 0; i--) {
    const e = s.events[i];
    if (isTurn(e, id)) return (e.payload as { working?: boolean }).working ? undefined : e.at;
  }
  return undefined;
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
  const working = (e.payload as { working?: unknown }).working;
  if (e.kind !== "turn" || !key || typeof working !== "boolean") return [s, fx];
  const [next, more] = applyTurn(s, key, working, e.at);
  return [next, [...fx, ...more]];
}
