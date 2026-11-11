// How the page words a card's review. The rounds and whose turn it is come from the daemon
// (`TicketDetail.review`); nothing here decides what is allowed.

import type { ReviewRound, ReviewView } from "../api/types";
import { checkText, systemRoundText } from "./checkPhase";

/// Who gave a round, in words: a person, the daemon's check or the reviewer agent.
export function whoText(r: ReviewRound): string {
  if (r.reviewer === "human") return "you";
  return r.reviewer === "system" ? "the check" : "the reviewer agent";
}

/// What the round decided, in words ("passed" and "failed" for the check).
export function verdictText(r: ReviewRound): string {
  if (r.reviewer === "system") return r.verdict === "approve" ? "passed" : "failed";
  return r.verdict === "approve" ? "approved it" : "asked for changes";
}

/// The first line of a round's remarks, cut to a short list entry; the card's comments hold the whole text.
export function gistText(comment: string, max = 120): string {
  const line = comment.split("\n").find((l) => l.trim() !== "")?.trim() ?? "";
  return line.length > max ? `${line.slice(0, max - 1)}…` : line;
}

/// "Round 2: you asked for changes — handle the error case".
export function roundText(r: ReviewRound): string {
  const skipped = r.reviewer === "system" ? systemRoundText(r.verdict, r.comment) : undefined;
  if (skipped) return `Round ${r.number}: ${skipped}`;
  const gist = gistText(r.comment);
  return `Round ${r.number}: ${whoText(r)} ${verdictText(r)}${gist ? ` — ${gist}` : ""}`;
}

/// When a round was decided, as a short local time.
export function whenText(ms: number): string {
  return new Date(ms).toLocaleString([], { month: "short", day: "numeric", hour: "2-digit", minute: "2-digit" });
}

/// How many returns the loop (the check, then the reviewer agent) has used, of the project's limit.
export function loopText(review: ReviewView): string {
  return `${review.used_rounds} of ${review.max_rounds} rounds of the loop used`;
}

/// Who the card waits for in review, in words; empty outside review.
export function awaitingText(review: ReviewView, now: number = Date.now()): string {
  const turn = review.awaiting;
  if (!turn) return "";
  if (turn.by === "person") return "waiting for you";
  if (turn.by === "merging") return "merging into the main branch";
  if (turn.by === "check") return checkText(review, now).replace(/^Being checked/, "being checked");
  return turn.decides ? "waiting for the reviewer agent" : "waiting for the reviewer agent, then you";
}

/// One acceptance item of a verdict: its judgement, and whether the reviewer ran what shows it or only read.
export type ItemRow = { outcome: "PASS" | "FAIL"; item: number; text: string; basis: "ran" | "read" | "NOT_RUN" };

const ITEM = /^(PASS|FAIL) (\d+)\. (.*) \[(ran|read|NOT_RUN)\]$/;

/// The item lines of a round's remarks (the daemon writes `PASS 1. text - evidence [ran]`); other lines are left out.
export function itemRows(comment: string): ItemRow[] {
  const rows: ItemRow[] = [];
  for (const line of comment.split("\n")) {
    const m = ITEM.exec(line.trim());
    if (m) rows.push({ outcome: m[1] as ItemRow["outcome"], item: Number(m[2]), text: m[3], basis: m[4] as ItemRow["basis"] });
  }
  return rows;
}

/// The basis in words for the card: "ran" for an item the reviewer ran, "read" for one judged from the diff.
export function basisText(b: ItemRow["basis"]): string {
  return b === "ran" ? "ran" : b === "read" ? "read only" : "NOT_RUN";
}
