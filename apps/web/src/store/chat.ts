// A card's Chat: what the person and the card's agent have said to each other, built from what
// the API already gives: the comments (instructions, reports, decisions; an agent's progress
// note is a report), the questions the agent asked and the answers the person gave (the journal),
// each as one bubble with who said it and when.

import type { Actor, Comment, EventRecord, Ticket, TicketDetail } from "../api/types";
import { answerWords } from "./eventWords";
import { isPermissionReport } from "./permissionReports";

export type ChatSide = "you" | "agent" | "system";
export type ChatKind = "instruction" | "report" | "decision" | "question" | "answer" | "note";

export interface ChatItem {
  key: string;
  /// Milliseconds since the epoch.
  at: number;
  side: ChatSide;
  /// Who said it, in words.
  who: string;
  kind: ChatKind;
  /// The line itself.
  text: string;
  /// Further lines of a comment (what, why, how to verify...).
  more: string[];
  /// An instruction the agent has not received yet.
  pending: boolean;
}

const SIDE: Record<Actor, ChatSide> = { human: "you", agent: "agent", system: "system" };
const WHO: Record<ChatSide, string> = { you: "You", agent: "Agent", system: "Toknila" };

/// How far apart a question and the report that repeats it may be written.
const TWIN_MS = 5000;

function commentItem(c: Comment): ChatItem {
  const side = SIDE[c.author];
  const b = c.body;
  const more = [b.what, b.why, b.how_to_verify, b.alternatives, b.consequences].filter((x): x is string => Boolean(x));
  return {
    key: `c${c.id}`, at: c.created_at, side, who: WHO[side], kind: c.kind, text: b.gist, more,
    pending: c.kind === "instruction" && c.author === "human" && !c.delivered_at,
  };
}

type Fields = Record<string, any>;

/// A journal entry of the card that belongs in the conversation, if it does: the agent asks, the
/// person answers.
function eventItem(e: EventRecord): ChatItem | undefined {
  const p = (e.payload ?? {}) as Fields;
  const base = { key: `e${e.seq}`, at: e.at, more: [], pending: false };
  const agent = (who: string, kind: ChatKind, text: string): ChatItem => ({ ...base, side: "agent", who, kind, text });
  const who = p.role === "reviewer" ? "Reviewer" : "Agent";
  if (e.entity !== "ticket") return undefined;
  if (e.kind === "status_changed" && p.reason?.status === "waiting") {
    const r = p.reason as Fields;
    if (r.kind === "silent") return agent("Agent", "note", "The agent stopped responding.");
    return agent("Agent", "question", r.kind === "permission" ? `Asks for permission: ${r.text}` : String(r.text ?? ""));
  }
  if (e.kind === "question_raised" && p.kind !== "question") {
    return agent(who, "question", p.kind === "permission" ? "Asks for permission." : "Is waiting for you on its screen.");
  }
  if (e.kind === "question_answered") {
    const side = SIDE[e.actor];
    return { ...base, side, who: WHO[side], kind: "answer", text: answerWords(p.answer) };
  }
  return undefined;
}

/// The conversation of a card, oldest first. While the card waits on a question that the answer
/// box shows (`open`), that question is not repeated as a bubble.
export function chatItems(detail: TicketDetail | undefined, events: EventRecord[], open: boolean): ChatItem[] {
  const comments = (detail?.comments ?? []).filter((c) => !isPermissionReport(c)).map(commentItem);
  const asked = events.map(eventItem).filter((i): i is ChatItem => i !== undefined);
  // An agent's question is also written down as a report of the same words, a moment apart; the
  // question stays, as the question.
  const twin = (c: ChatItem) =>
    c.kind === "report" && c.side === "agent" &&
    asked.some((q) => q.kind === "question" && q.text === c.text && Math.abs(q.at - c.at) <= TWIN_MS);
  const items = [...comments.filter((c) => !twin(c)), ...asked].sort((a, b) => a.at - b.at || a.key.localeCompare(b.key));
  if (open) {
    const last = items.map((i) => i.kind).lastIndexOf("question");
    if (last >= 0) items.splice(last, 1);
  }
  return items;
}

const MONTHS = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
const two = (n: number) => String(n).padStart(2, "0");

/// When a bubble was said: the time today, else the day and the time.
export function whenText(at: number, now: number = Date.now()): string {
  const d = new Date(at);
  const n = new Date(now);
  const time = `${two(d.getHours())}:${two(d.getMinutes())}`;
  const today = d.getFullYear() === n.getFullYear() && d.getMonth() === n.getMonth() && d.getDate() === n.getDate();
  return today ? time : `${MONTHS[d.getMonth()]} ${d.getDate()}, ${time}`;
}

/// Whether anyone can still be told something on this card: not once it is finished or cancelled.
export const canTalk = (card: Pick<Ticket, "status">): boolean =>
  card.status !== "merged" && card.status !== "released" && card.status !== "cancelled";
