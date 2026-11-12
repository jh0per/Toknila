// What the top bar and the Agents node say about today's spend. The daemon sums; this only words
// the figures it sent.

import type { ProjectRun, QuotaWindow, TodayUsage } from "../api/types";
import { costText } from "../format";
import type { State } from "./state";

/// How often the window asks again (the daemon keeps an answer for a minute).
export const TODAY_POLL_MS = 60_000;

/// Today's answer for `project`, when the one held is for it.
export function todayOf(s: Pick<State, "usage">, project: string | undefined): TodayUsage | undefined {
  const t = s.usage.today;
  return project !== undefined && t?.project === project ? t.view : undefined;
}

/// "est. $23.77 today".
export const todayText = (usd: number): string => `est. ${costText(usd)} today`;

/// The clock time a window resets, in the person's time zone: 15:30.
export function resetClock(resetsAtSeconds: number): string {
  const d = new Date(resetsAtSeconds * 1000);
  return `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
}

/// One window of the plan: "5h 48% used · resets 15:30" for the five-hour window, "7d 8%" for
/// the week's. Other windows are not shown.
export function windowText(w: QuotaWindow): string | undefined {
  const pct = Math.round(w.used_percentage);
  if (w.name === "five_hour") return `5h ${pct}% used · resets ${resetClock(w.resets_at)}`;
  if (w.name === "seven_day") return `7d ${pct}%`;
  return undefined;
}

/// The windows to show, the five-hour one first.
export function windowTexts(today: TodayUsage): string[] {
  const order = ["five_hour", "seven_day"];
  const windows = [...(today.quota?.windows ?? [])].sort((a, b) => order.indexOf(a.name) - order.indexOf(b.name));
  return windows.map(windowText).filter((t): t is string => t !== undefined);
}

/// The title of the line: the machine-wide figure and the estimate note's reason.
export const machineTitle = (today: TodayUsage, note: string): string =>
  `all projects: ${costText(today.machine.cost_usd)} today. ${note}`;

/// What the agents running now spent today: the cards among `runs`, each once (a worker and its
/// reviewer share a card). Undefined when no figure has arrived or no card runs.
export function runningSpend(today: TodayUsage | undefined, runs: ProjectRun[]): number | undefined {
  if (!today) return undefined;
  const keys = new Set(runs.flatMap((r) => (r.card ? [r.card.key] : [])));
  if (keys.size === 0) return undefined;
  let usd = 0;
  for (const key of keys) usd += today.cards?.[key]?.cost_usd ?? 0;
  return usd;
}
