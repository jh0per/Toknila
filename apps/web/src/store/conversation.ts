// A project's conversation with its assistant, as the chat shows it: the daemon's record, oldest
// first, the last page loaded and older pages on request. A message arrives through the journal
// (`conversation.message`, and a card discussion's `thread.message_posted`). Pure: the requests
// are the effects in `conversationEffects.ts`.

import type { ConversationAbout, ConversationMessage, ConversationPage, EventRecord, PlannerState } from "../api/types";
import { ANSWER_WAIT_MS } from "./discussion";
import type { Effect } from "./msg";
import type { Route } from "./route";
import type { State } from "./state";

/// How many messages the window asks for at a time.
export const PAGE = 50;

/// One project's conversation as loaded.
export interface Talk {
  /// Oldest first, no id twice.
  messages: ConversationMessage[];
  /// The daemon has older messages than the first one here.
  more: boolean;
  /// An older page is on its way.
  older: boolean;
}

export type ConversationSlice = Record<string, Talk>;

export const initialConversation: ConversationSlice = {};

/// What the person did.
export type ConversationIntent =
  /// The chat opened: load the newest page.
  | { type: "loadConversation"; project: string }
  /// The person scrolled to the top: load the page before the first message.
  | { type: "loadOlder"; project: string };

/// What the daemon answered, or the stream brought.
export type ConversationResult =
  | { type: "conversationLoaded"; project: string; page: ConversationPage; before?: string }
  /// One message, from the journal or from the person's own send.
  | { type: "conversationMessage"; project: string; message: ConversationMessage }
  | { type: "conversationFailed"; project: string; error: string };

export type ConversationMsg = ConversationIntent | ConversationResult;

export type ConversationEffect = { type: "fetchConversation"; project: string; before?: string };

const order = (a: ConversationMessage, b: ConversationMessage) => a.at - b.at || a.id.localeCompare(b.id);

/// `have` and `more` together: one list, oldest first, no id twice (the later copy wins).
export function mergeMessages(have: ConversationMessage[], more: ConversationMessage[]): ConversationMessage[] {
  const byId = new Map(have.map((m) => [m.id, m]));
  for (const m of more) byId.set(m.id, m);
  return [...byId.values()].sort(order);
}

export const talkOf = (s: State, project: string): Talk | undefined => s.conversation[project];

export function reduceConversation(s: State, msg: ConversationMsg): [State, Effect[]] {
  const set = (project: string, talk: Talk | undefined): State => {
    const { [project]: _gone, ...rest } = s.conversation;
    return { ...s, conversation: talk ? { ...rest, [project]: talk } : rest };
  };
  switch (msg.type) {
    case "loadConversation":
      return [s, [{ type: "fetchConversation", project: msg.project }]];
    case "loadOlder": {
      const talk = s.conversation[msg.project];
      const first = talk?.messages[0];
      if (!talk || !talk.more || talk.older || !first) return [s, []];
      return [set(msg.project, { ...talk, older: true }), [{ type: "fetchConversation", project: msg.project, before: first.id }]];
    }
    case "conversationLoaded": {
      const talk = s.conversation[msg.project];
      if (msg.before !== undefined) {
        // An older page joins what is there; whether more remain is the page's say.
        const have = talk?.messages ?? [];
        return [set(msg.project, { messages: mergeMessages(have, msg.page.messages), more: msg.page.more, older: false }), []];
      }
      // The newest page: it keeps what the window already has (older pages, a message that came
      // in a moment ago), and says there is more when either did.
      const have = talk?.messages ?? [];
      const more = talk && have.length > 0 && have[0].at < (msg.page.messages[0]?.at ?? Infinity) ? talk.more : msg.page.more;
      return [set(msg.project, { messages: mergeMessages(have, msg.page.messages), more, older: false }), []];
    }
    case "conversationMessage": {
      const talk = s.conversation[msg.project];
      // A chat that is not open loads the whole newest page when it opens.
      if (!talk) return [s, []];
      return [set(msg.project, { ...talk, messages: mergeMessages(talk.messages, [msg.message]) }), []];
    }
    case "conversationFailed": {
      const talk = s.conversation[msg.project];
      return [talk ? set(msg.project, { ...talk, older: false }) : s, []];
    }
  }
}

const reload = (s: State): Effect[] => Object.keys(s.conversation).map((project) => ({ type: "fetchConversation", project }));

/// A `conversation.message` journal event: the message joins its project's chat, if the chat is
/// open. A gap in the stream reloads the newest page of every open chat instead.
export function conversationEvent(s: State, e: EventRecord, gap: boolean): [State, Effect[]] {
  const p = (e.payload ?? {}) as { project?: string; message?: ConversationMessage };
  const key = s.projects.find((x) => x.id === p.project)?.key;
  // A planner's turn started or ended: which answers close their messages depends on it.
  if (e.entity === "planner") return [s, key ? [{ type: "plannerCall", project: key, call: "load" }] : []];
  if (gap) return [s, reload(s)];
  if (!key || !p.message) return [s, []];
  return reduceConversation(s, { type: "conversationMessage", project: key, message: p.message });
}

/// What a message in a card's thread asks of the open chats: a card discussion is part of its
/// project's conversation, so they read their newest page again.
export const reloadTalks = (s: State, e: EventRecord): Effect[] => (e.kind === "message_posted" ? reload(s) : []);

/// Whether a message is the assistant at work (Toknila's line, or its own progress line) and so
/// neither a word from the person nor an answer to them.
const atWork = (m: ConversationMessage): boolean => m.author === "toknila" || m.author === "progress";

/// The message the assistant owes an answer to: the person spoke last (Toknila's lines and the
/// assistant's progress lines are the assistant at work, not a word to the person).
export function unanswered(messages: ConversationMessage[]): ConversationMessage | undefined {
  const last = [...messages].reverse().find((m) => !atWork(m));
  return last?.author === "person" ? last : undefined;
}

/// The turn the assistant is in, when the person is owed an answer: the message that started it
/// (`asked`), the ids of the messages sent after it, which wait for the next turn (`queued`), and
/// the assistant's latest progress line of this turn.
export interface OpenTurn {
  asked?: ConversationMessage;
  queued: string[];
  progress?: ConversationMessage;
  /// When the asked message's wait began: its send, or the answer before it, whichever is later.
  since?: number;
  /// The unread messages (the asked one and the queued ones) of a planner that has stopped.
  stopped: string[];
}

/// The one place that decides which messages are answered. It uses these facts only:
/// 1. the order of the record: the n-th answer of the assistant answers the n-th message of the
///    person (the planner answers one message per turn, which `say` enforces);
/// 2. `planner.working` and `planner.since` (TOKNILA-127), when the planner says its turn is on:
///    an answer posted since that turn began does not answer its message yet, a message is
///    answered only when the turn that followed it ended;
/// 3. `planner.running`: a planner that stopped will read none of the messages still unread.
/// Without a planner state (not loaded) only the first fact is used. `resent` are messages the
/// person sent again after a stop: their copy is the one awaited, so they are left out.
export function openTurn(messages: ConversationMessage[], planner?: PlannerState, resent: string[] = []): OpenTurn {
  const unread: ConversationMessage[] = [];
  let since: number | undefined;
  let progress: ConversationMessage | undefined;
  const turnOn = planner?.running && planner.working && planner.since != null ? planner.since : undefined;
  for (const m of messages) {
    if (m.author === "person") {
      if (!resent.includes(m.id)) unread.push(m);
    }
    else if (m.author === "assistant") {
      if (turnOn !== undefined && m.at >= turnOn) continue;
      if (unread.shift()) since = m.at;
    } else if (m.author === "progress") progress = m;
  }
  const asked = unread[0];
  if (!asked) return { queued: [], stopped: [] };
  const from = Math.max(asked.at, since ?? asked.at);
  // A progress line from before the wait began belongs to an older turn.
  const mine = progress && progress.at >= from && order(asked, progress) < 0 ? progress : undefined;
  const stopped = planner && !planner.running ? unread.map((m) => m.id) : [];
  return { asked, queued: unread.slice(1).map((m) => m.id), progress: mine, since: from, stopped };
}

/// Whole minutes from `at` to `now`, never below zero.
export const minutesSince = (at: number, now: number): number => Math.max(0, Math.floor((now - at) / 60_000));

/// How long the assistant may take before the waiting line stops counting and points to its terminal.
export const STILL_WORKING_MIN = 5;

/// What a queued message of the person says under it.
export const QUEUED_NOTE = "sent, the assistant reads it after this answer";

/// What a message says when the assistant stopped before reading it.
export const STOPPED_NOTE = "the assistant stopped before reading this; send it again";

/// Milliseconds until the unanswered message is overdue (0 when it is); undefined when nothing waits.
export function msUntilOverdue(messages: ConversationMessage[], now: number): number | undefined {
  const m = unanswered(messages);
  return m === undefined ? undefined : Math.max(0, m.at + ANSWER_WAIT_MS - now);
}

/// Calls `wake` when the unanswered message becomes overdue; returns what stops the timer.
export function wakeWhenLate(messages: ConversationMessage[], now: number, wake: () => void): () => void {
  const left = msUntilOverdue(messages, now);
  if (left === undefined || left === 0) return () => {};
  const id = setTimeout(wake, left);
  return () => clearTimeout(id);
}

/// Where a message's context chip or a Toknila line links to: the card's page when the store has
/// the card, else the chain's, else nowhere.
export function aboutRoute(s: State, project: string, about: ConversationAbout | null | undefined): Route | undefined {
  if (!about) return undefined;
  if (about.kind === "roadmap") return { name: "project", project };
  if (about.kind === "chain") return { name: "projectChain", project, chain: about.name };
  for (const id of s.chainOrder) {
    const chain = s.chains[id];
    if (chain?.tickets.some((t) => t.key === about.key)) return { name: "card", project, chain: chain.chain.name, key: about.key };
  }
  return undefined;
}

/// What a context chip says. A chain shows its title; `title` is the one the store has for the
/// chain's name (the slug when it has none).
export const aboutLabel = (about: ConversationAbout, title?: string): string =>
  about.kind === "roadmap" ? "roadmap" : about.kind === "chain" ? `epic ${title ?? about.name}` : about.key;
