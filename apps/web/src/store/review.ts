// How the page words a card's review. The rounds and whose turn it is come from the daemon
// (`TicketDetail.review`); nothing here decides what is allowed.

import type { ReviewRound, ReviewView } from "../api/types";

/// "Round 2: you asked for changes — handle the error case".
export function roundText(r: ReviewRound): string {
  const who = r.reviewer === "human" ? "you" : "the reviewer agent";
  const did = r.verdict === "approve" ? "approved it" : "asked for changes";
  return `Round ${r.number}: ${who} ${did}${r.comment ? ` — ${r.comment}` : ""}`;
}

/// Who the card waits for in review, in words; empty outside review.
export function awaitingText(review: ReviewView): string {
  const turn = review.awaiting;
  if (!turn) return "";
  if (turn.by === "person") return "waiting for you";
  if (turn.by === "merging") return "merging into the base branch";
  return turn.decides ? "waiting for the reviewer agent" : "waiting for the reviewer agent, then you";
}
