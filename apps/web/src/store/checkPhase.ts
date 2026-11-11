// How the page words where a card's check stands. The phase comes from the daemon
// (`ReviewView.check`); nothing here decides it.

import type { ReviewView } from "../api/types";

/// How long a check has run: seconds under a minute, else whole minutes.
export function elapsedText(since: number, now: number): string {
  const secs = Math.max(0, Math.floor((now - since) / 1000));
  return secs < 60 ? `${secs} s` : `${Math.floor(secs / 60)} min`;
}

/// "queued, next in line", "syncing with main", "cargo xtask verify, 3 min", or "starting" before the checker reports.
export function checkWhat(review: ReviewView, now: number): string {
  const c = review.check;
  if (!c) return review.check_queue ? queued(review.check_queue) : "starting";
  if (c.phase === "queued") return queued(c.place);
  if (c.phase === "syncing") return "syncing with main";
  return `${c.command}, ${elapsedText(c.since, now)}`;
}

function queued(place: number): string {
  return place === 1 ? "queued, next in line" : `queued, #${place} in line`;
}

/// "Being checked: cargo xtask verify, 3 min".
export function checkText(review: ReviewView, now: number): string {
  return `Being checked: ${checkWhat(review, now)}`;
}

/// What a round of the daemon's own pass says in the round list: the check's verdict, or, for a
/// project with no check command, that the ship was only brought up to main.
export function systemRoundText(verdict: "approve" | "request_changes", comment: string): string | undefined {
  if (verdict === "approve" && comment.startsWith("Brought up to main")) return "brought up to main; the project has no check command";
  return undefined;
}
