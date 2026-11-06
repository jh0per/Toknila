// What a thread looks like to the person: derived from what the daemon sends, never stored.
// The daemon owns the rules (an open thread blocks hand-in); this only names them for display.

import type { ThreadDetail, ThreadVersion } from "../api/types";

export type ThreadStatus = "open" | "changes_requested" | "approved";

/// Approved, changes requested (the daemon's `changes_requested`), or plain open.
export function threadStatus(t: ThreadDetail): ThreadStatus {
  if (t.thread.state === "approved") return "approved";
  return t.changes_requested ? "changes_requested" : "open";
}

/// How many threads still keep the card from being handed in.
export function blocksHandIn(threads: ThreadDetail[]): number {
  return threads.filter((t) => t.thread.state === "open").length;
}

/// The version on screen: the one picked if it exists, else the latest.
export function shownVersion(t: ThreadDetail, picked: number | undefined): ThreadVersion | undefined {
  return t.versions.find((v) => v.number === picked) ?? t.versions[t.versions.length - 1];
}
