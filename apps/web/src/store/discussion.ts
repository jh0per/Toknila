// A card's talk with the assistant, read from its discussion thread: what was said, what the
// assistant proposes for the card's prompt and acceptance (as a diff), and what became of it. The
// daemon owns the rules; this only shapes them for the screen.

import type { ThreadDetail, ThreadMessage, TicketThreads } from "../api/types";

/// What the assistant proposes to change: each field as it was and as it would be.
export interface Proposal {
  prompt?: { from: string; to: string };
  acceptance?: { from: string[]; to: string[] };
}

export type EditState = "pending" | "accepted" | "rejected";

export type Entry =
  | { type: "say"; id: string; side: "you" | "assistant"; text: string }
  | { type: "edit"; id: string; proposal: Proposal; state: EditState };

/// The card's discussion thread, if anyone has said something.
export function discussionOf(threads: TicketThreads | undefined): ThreadDetail | undefined {
  return threads?.threads.find((t) => t.thread.kind === "discussion");
}

function proposalOf(m: ThreadMessage): Proposal | undefined {
  try {
    return JSON.parse(m.text) as Proposal;
  } catch {
    return undefined;
  }
}

/// The conversation, oldest first: messages, and each proposal with its state.
export function entries(d: ThreadDetail | undefined): Entry[] {
  if (!d) return [];
  const decided = (m: ThreadMessage, kind: ThreadMessage["kind"]) =>
    d.messages.some((x) => x.kind === kind && x.text === m.id);
  const out: Entry[] = [];
  for (const m of d.messages) {
    if (m.kind === "edit_proposal") {
      const proposal = proposalOf(m);
      if (!proposal) continue;
      const state: EditState = decided(m, "edit_accepted") ? "accepted" : decided(m, "edit_rejected") ? "rejected" : "pending";
      out.push({ type: "edit", id: m.id, proposal, state });
    } else if (m.kind === "message" || m.kind === "changes_requested") {
      out.push({ type: "say", id: m.id, side: m.author === "human" ? "you" : "assistant", text: m.text });
    }
  }
  return out;
}

/// The person has spoken last and the assistant has not answered yet: the box waits.
export function awaitingAnswer(d: ThreadDetail | undefined): boolean {
  const last = [...(d?.messages ?? [])].reverse().find((m) => m.kind === "message" || m.kind === "edit_proposal");
  return last !== undefined && last.author === "human";
}

/// The place a card's draft is kept in the store's drafts.
export const draftKey = (card: string): string => `card:${card}`;

/// The line the planner's terminal takes for a message about a card (one line: a line break would
/// submit it half-way). The planner's instructions say what to do with it.
export function askLine(card: string, thread: number, text: string): string {
  const one = text.trim().replace(/\s*\n\s*/g, " ");
  return `Discussion on card ${card} (thread ${thread}): ${one}`;
}

export type DiffOp = "same" | "add" | "del";
export interface DiffLine {
  op: DiffOp;
  text: string;
}

/// A line diff of two lists of lines (longest common subsequence): lines in both are `same`, the
/// others `del` (only in `from`) or `add` (only in `to`), in reading order.
export function diffLines(from: string[], to: string[]): DiffLine[] {
  const n = from.length;
  const m = to.length;
  const len: number[][] = Array.from({ length: n + 1 }, () => new Array<number>(m + 1).fill(0));
  for (let i = n - 1; i >= 0; i--) {
    for (let j = m - 1; j >= 0; j--) {
      len[i][j] = from[i] === to[j] ? len[i + 1][j + 1] + 1 : Math.max(len[i + 1][j], len[i][j + 1]);
    }
  }
  const out: DiffLine[] = [];
  let i = 0;
  let j = 0;
  while (i < n && j < m) {
    if (from[i] === to[j]) out.push({ op: "same", text: from[i++] }), j++;
    else if (len[i + 1][j] >= len[i][j + 1]) out.push({ op: "del", text: from[i++] });
    else out.push({ op: "add", text: to[j++] });
  }
  while (i < n) out.push({ op: "del", text: from[i++] });
  while (j < m) out.push({ op: "add", text: to[j++] });
  return out;
}

/// The diff of a prompt: by lines of its text.
export const diffText = (from: string, to: string): DiffLine[] => diffLines(from.split("\n"), to.split("\n"));
