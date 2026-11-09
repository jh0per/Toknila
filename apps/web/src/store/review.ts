// How the page words a card's review. The rounds and whose turn it is come from the daemon
// (`TicketDetail.review`); nothing here decides what is allowed.

import type { ReviewRound, ReviewView } from "../api/types";

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
export function awaitingText(review: ReviewView): string {
  const turn = review.awaiting;
  if (!turn) return "";
  if (turn.by === "person") return "waiting for you";
  if (turn.by === "merging") return "merging into the main branch";
  if (turn.by === "check") {
    const place = review.check_queue ?? 0;
    return place > 0 ? `the check waits for another check to end (${place === 1 ? "next in line" : `#${place} in line`})` : "the check is running";
  }
  return turn.decides ? "waiting for the reviewer agent" : "waiting for the reviewer agent, then you";
}
