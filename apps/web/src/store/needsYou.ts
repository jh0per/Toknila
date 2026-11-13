// What a person must do by hand on a story in Review: the PERSON items of the agent's hand-in and
// of the reviewer's verdict. The words come from the daemon (the hand-in's blocks, the verdict's
// `PERSON 2. text [basis]` lines); this only picks them out.

import type { Comment, ReviewView } from "../api/types";
import { itemRows } from "./review";

/// Said when no item is for a person.
export const NOTHING_BY_HAND = "Nothing to do by hand; Accept when the hand-in convinces you";
/// The line under each button.
export const ACCEPT_LINE = "Accept: the story merges";
export const RETURN_LINE = "Return: the agent fixes what you found";
/// The two sentences of the first-time hint above the buttons.
export const REVIEW_HINT = "Accept merges the story. Return sends your words to the agent, which fixes and hands in again.";

const PERSON_LINE = /^[-*]?\s*PERSON\b[\s:.\-–—]*(.+)$/;

function handInItems(report: Comment | undefined): string[] {
  const b = report?.body;
  if (!b) return [];
  const text = [b.gist, b.what, b.how_to_verify, b.consequences].filter(Boolean).join("\n");
  const items: string[] = [];
  for (const line of text.split("\n")) {
    const m = PERSON_LINE.exec(line.trim());
    if (m) items.push(m[1].trim());
  }
  return items;
}

/// The reviewer's PERSON items from the newest round that has any item lines; an older round's
/// items were answered by the work done since.
function verdictItems(review: ReviewView | undefined): string[] {
  for (const r of [...(review?.rounds ?? [])].reverse()) {
    const rows = itemRows(r.comment);
    if (rows.length > 0) return rows.filter((x) => x.outcome === "PERSON").map((x) => x.text.replace(/\s*\[(ran|read|NOT_RUN)\]$/, ""));
  }
  return [];
}

/// What the person must do by hand: the hand-in's PERSON items, then the verdict's, each once.
export function needsYou(report: Comment | undefined, review: ReviewView | undefined): string[] {
  const seen = new Set<string>();
  return [...handInItems(report), ...verdictItems(review)].filter((t) => t && !seen.has(t.toLowerCase()) && seen.add(t.toLowerCase()));
}

const KEY = "toknila.review-hint-dismissed.";

/// Whether this viewer dismissed the Accept-or-Return hint in the project; shown when nothing is
/// stored or storage cannot be read.
export function hintDismissed(project: string, storage?: Storage): boolean {
  try {
    return (storage ?? window.localStorage).getItem(KEY + project) === "1";
  } catch {
    return false;
  }
}

/// Remembers that the hint was dismissed for this viewer and project.
export function storeHintDismissed(project: string, storage?: Storage): void {
  try {
    (storage ?? window.localStorage).setItem(KEY + project, "1");
  } catch {
    // The hint is gone for this visit.
  }
}
