// The Usage page: the window, the metric shown and what the daemon answered. The daemon does
// every sum; this keeps its answer and picks what to show.

import type { UsageItem, UsageView } from "../api/types";
import type { Effect } from "./msg";
import type { State } from "./state";

export const WINDOWS = [7, 30, 90] as const;
export type Days = (typeof WINDOWS)[number];
export type Metric = "cost" | "tokens";

export interface UsageState {
  days: Days;
  metric: Metric;
  view: UsageView | undefined;
  error: string;
}

export const initialUsage: UsageState = { days: 7, metric: "cost", view: undefined, error: "" };

/// What the person did on the page.
export type UsageIntent =
  | { type: "setUsageDays"; days: Days }
  | { type: "setUsageMetric"; metric: Metric }
  | { type: "setMachineScan"; on: boolean };

/// What the daemon answered.
export type UsageResult = { type: "usageLoaded"; view: UsageView } | { type: "usageFailed"; error: string };

export type UsageMsg = UsageIntent | UsageResult;

export type UsageEffect =
  | { type: "loadUsage"; days: Days }
  | { type: "saveUsageSettings"; machine_scan: boolean; days: Days }
  | { type: "pollUsage"; days: Days };

export const loadUsage = (s: State): Effect => ({ type: "loadUsage", days: s.usage.days });

/// The collector saves new rows without a journal event, so the open page asks again after each
/// answer (`POLL_MS` in the effect runner), and stops once the person leaves it.
const again = (s: State): Effect[] => (s.route.name === "usage" ? [{ type: "pollUsage", days: s.usage.days }] : []);

export function reduceUsage(s: State, msg: UsageMsg): [State, Effect[]] {
  const set = (usage: Partial<UsageState>): State => ({ ...s, usage: { ...s.usage, ...usage } });
  switch (msg.type) {
    case "setUsageDays":
      return [set({ days: msg.days }), [{ type: "loadUsage", days: msg.days }]];
    case "setUsageMetric":
      return [set({ metric: msg.metric }), []];
    case "setMachineScan":
      return [s, [{ type: "saveUsageSettings", machine_scan: msg.on, days: s.usage.days }]];
    case "usageLoaded":
      // An answer for a window the person has since left is stale.
      if (msg.view.summary.days !== s.usage.days) return [s, []];
      return [set({ view: msg.view, error: "" }), again(s)];
    case "usageFailed":
      return [set({ error: msg.error }), again(s)];
  }
}

/// The largest of some values (0 for none).
export const maxOf = (values: number[]): number => values.reduce((a, b) => Math.max(a, b), 0);

export interface BreakdownRow {
  label: string;
  value: number;
  /// Of the largest row, 0 to 1: the length of its bar.
  share: number;
  item: UsageItem;
}

/// The rows of a breakdown table for the metric shown, each with its bar length.
export function breakdownRows(items: UsageItem[], metric: Metric): BreakdownRow[] {
  const value = (i: UsageItem) => (metric === "cost" ? i.figures.cost_usd : i.figures.processed);
  const top = maxOf(items.map(value));
  return items.map((item) => ({
    label: item.label,
    value: value(item),
    share: top === 0 ? 0 : value(item) / top,
    item,
  }));
}
