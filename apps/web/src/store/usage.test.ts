import { describe, expect, it } from "vitest";
import type { UsageView } from "../api/types";
import { reduce } from "./reduce";
import { initialState } from "./state";
import { breakdownRows, maxOf } from "./usage";

const view = (days: number): UsageView =>
  ({ summary: { days, series: [], totals: {} }, machine_scan: true }) as unknown as UsageView;

const start = () => initialState({ name: "chains" });

describe("the usage page", () => {
  it("loads the default window when it opens", () => {
    const [s, fx] = reduce(start(), { type: "navigate", route: { name: "usage" } });
    expect(s.route).toEqual({ name: "usage" });
    expect(fx).toContainEqual({ type: "loadUsage", days: 7 });
    expect(fx).toContainEqual({ type: "pushUrl", path: "/usage" });
  });

  it("switching the window reloads it and the answer is kept", () => {
    const [s, fx] = reduce(start(), { type: "setUsageDays", days: 30 });
    expect(s.usage.days).toBe(30);
    expect(fx).toEqual([{ type: "loadUsage", days: 30 }]);
    const [loaded] = reduce(s, { type: "usageLoaded", view: view(30) });
    expect(loaded.usage.view?.summary.days).toBe(30);
  });

  it("asks again after an answer while the page is open, and not after it was left", () => {
    const [open] = reduce(start(), { type: "navigate", route: { name: "usage" } });
    const [, fx] = reduce(open, { type: "usageLoaded", view: view(7) });
    expect(fx).toEqual([{ type: "pollUsage", days: 7 }]);
    const [, failedFx] = reduce(open, { type: "usageFailed", error: "x" });
    expect(failedFx).toEqual([{ type: "pollUsage", days: 7 }]);
    const [, away] = reduce(start(), { type: "usageLoaded", view: view(7) });
    expect(away).toEqual([]);
  });

  it("ignores an answer for a window the person has left", () => {
    const [s] = reduce(start(), { type: "setUsageDays", days: 90 });
    const [late] = reduce(s, { type: "usageLoaded", view: view(7) });
    expect(late.usage.view).toBeUndefined();
  });

  it("the cost and tokens toggle changes the metric and loads nothing", () => {
    const [s, fx] = reduce(start(), { type: "setUsageMetric", metric: "tokens" });
    expect(s.usage.metric).toBe("tokens");
    expect(fx).toEqual([]);
  });

  it("the machine-wide switch is saved by the daemon, which then answers with the page", () => {
    const [, fx] = reduce(start(), { type: "setMachineScan", on: false });
    expect(fx).toEqual([{ type: "saveUsageSettings", machine_scan: false, days: 7 }]);
  });

  it("a failed load is shown", () => {
    const [s] = reduce(start(), { type: "usageFailed", error: "boom" });
    expect(s.usage.error).toBe("boom");
  });

  it("a usage event while the page is open reloads it", () => {
    const [s] = reduce(start(), { type: "navigate", route: { name: "usage" } });
    const [, fx] = reduce({ ...s, lastSeq: 1 }, {
      type: "event",
      event: { seq: 2, at: 0, actor: "system", entity: "project", entity_id: "x", kind: "k", payload: {} },
    });
    expect(fx).toContainEqual({ type: "loadUsage", days: 7 });
  });
});

describe("breakdown rows", () => {
  const item = (label: string, cost: number, processed: number) =>
    ({ label, figures: { cost_usd: cost, processed } }) as never;
  it("pick the metric and give each row its share of the largest", () => {
    const items = [item("a", 4, 100), item("b", 1, 400)];
    expect(breakdownRows(items, "cost").map((r) => [r.label, r.value, r.share])).toEqual([
      ["a", 4, 1], ["b", 1, 0.25],
    ]);
    expect(breakdownRows(items, "tokens").map((r) => r.share)).toEqual([0.25, 1]);
  });
  it("an empty or all-zero list has no division by zero", () => {
    expect(breakdownRows([], "cost")).toEqual([]);
    expect(breakdownRows([item("z", 0, 0)], "cost")[0].share).toBe(0);
    expect(maxOf([0, 0])).toBe(0);
  });
});
