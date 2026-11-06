// What a thread looks like to the person: derived from what the daemon sends, never stored.
// The daemon owns the rules (an open thread blocks hand-in); this only names them for display.

import type { ThreadDetail, ThreadVersion } from "../api/types";

export type ThreadStatus = "open" | "changes_requested" | "approved";

/// Approved, or open; open with a request for changes nobody answered with a newer version yet.
export function threadStatus(t: ThreadDetail): ThreadStatus {
  if (t.thread.state === "approved") return "approved";
  const asked = [...t.messages].reverse().find((m) => m.kind === "changes_requested");
  if (!asked) return "open";
  const answered = t.versions.some((v) => v.created_at > asked.created_at);
  return answered ? "open" : "changes_requested";
}

/// How many threads still keep the card from being handed in.
export function blocksHandIn(threads: ThreadDetail[]): number {
  return threads.filter((t) => t.thread.state === "open").length;
}

/// The version on screen: the one picked if it exists, else the latest.
export function shownVersion(t: ThreadDetail, picked: number | undefined): ThreadVersion | undefined {
  return t.versions.find((v) => v.number === picked) ?? t.versions[t.versions.length - 1];
}
