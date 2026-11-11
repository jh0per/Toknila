import type { ChainDetail, ChainSummary } from "../api/types";
import type { Tone } from "./groups";

export type Phase = ChainSummary["phase"];

/// The colour family of a phase: active reads as work going on, finished as done.
export function phaseTone(phase: Phase): Tone {
  switch (phase) {
    case "active":
      return "prog";
    case "waiting":
      return "attn";
    case "finished":
      return "done";
    case "idle":
    case "empty":
      return "todo";
  }
}

const RANK: Record<Phase, number> = { active: 0, waiting: 1, idle: 2, empty: 3, finished: 4 };

/// The chains that are not finished (active and waiting first, the order kept inside a phase) and
/// the finished ones.
export function splitChains(chains: ChainDetail[]): { open: ChainDetail[]; finished: ChainDetail[] } {
  const rank = (c: ChainDetail) => RANK[c.summary.phase];
  const open = chains.filter((c) => c.summary.phase !== "finished");
  open.sort((a, b) => rank(a) - rank(b));
  return { open, finished: chains.filter((c) => c.summary.phase === "finished") };
}

/// `12 Nov`: the day of a timestamp in milliseconds.
export function dayText(ms: number): string {
  return new Date(ms).toLocaleDateString("en-GB", { day: "numeric", month: "short" });
}

/// What the Finish node says: `Finished · 12 Nov`, or how many cards are left.
export function finishText(s: Pick<ChainSummary, "phase" | "finished_at" | "total" | "done">): string {
  if (s.phase === "finished") return s.finished_at == null ? "Finished" : `Finished · ${dayText(s.finished_at)}`;
  return `${Math.max(0, s.total - s.done)} left`;
}

const KEY = "toknila.finished-open.";

/// Whether the project's Finished group was left open; closed when nothing was stored.
export function readFinishedOpen(project: string): boolean {
  try {
    return window.localStorage.getItem(KEY + project) === "1";
  } catch {
    return false;
  }
}

/// Remembers the state of the Finished group for this viewer and project.
export function storeFinishedOpen(project: string, open: boolean): void {
  try {
    window.localStorage.setItem(KEY + project, open ? "1" : "0");
  } catch {
    // The group still opens and closes for this visit.
  }
}
