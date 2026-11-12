import { describe, expect, it } from "vitest";
import type { UsageItem } from "../api/types";
import { shipOf, workspaceRows } from "./usageGroups";

const item = (label: string, processed: number, cost: number, read = 0, input = 0): UsageItem =>
  ({
    label,
    figures: { processed, cost_usd: cost, uncached_input: input, tokens: { cache_read: read, input, cache_write: 0, output: 0 } },
  }) as unknown as UsageItem;

const ship = (repo: string, card: string) => `/Users/x/.toknila-ships/${repo}/phase-6/${card}`;

describe("workspace grouping", () => {
  it("knows a ship's repository and short name, and nothing about other folders", () => {
    expect(shipOf(ship("toknila", "toknila-77"))).toEqual({ repo: "toknila", short: "phase-6/toknila-77" });
    expect(shipOf("/work/demo")).toBeUndefined();
    expect(shipOf("/Users/x/.toknila-ships/toknila")).toBeUndefined();
  });

  it("puts worktrees under their repository with sums, largest first", () => {
    const rows = workspaceRows(
      [item(ship("toknila", "a"), 100, 1, 50, 50), item("/work/other", 500, 5), item(ship("toknila", "b"), 300, 3, 0, 300)],
      "tokens",
    );
    expect(rows.map((r) => [r.kind, r.short, r.inGroup])).toEqual([
      ["folder", "other", false],
      ["group", "toknila", false],
      ["folder", "phase-6/b", true],
      ["folder", "phase-6/a", true],
    ]);
    const group = rows[1];
    expect(group).toMatchObject({ processed: 400, cost: 4, value: 400 });
    expect(group.cachedShare).toBeCloseTo(50 / 400);
    expect(rows[0].share).toBe(1);
    expect(group.share).toBeCloseTo(0.8);
  });

  it("keeps the full path as the label of a folder and orders by cost when cost is shown", () => {
    const rows = workspaceRows([item("/work/a", 900, 1), item(ship("r", "c"), 10, 7)], "cost");
    expect(rows.map((r) => r.label)).toEqual(["r", ship("r", "c"), "/work/a"]);
  });

  it("names two folders with the same last folder by their parent too", () => {
    const rows = workspaceRows([item("/a/app", 2, 0), item("/b/app", 1, 0), item("/c/solo", 0, 0)], "tokens");
    expect(rows.map((r) => r.short)).toEqual(["a/app", "b/app", "solo"]);
  });
});

describe("grouping by the repository the daemon found", () => {
  const repo = "/Users/h0per/Work/web/imagegen-backend";
  const inRepo = (label: string, processed: number): UsageItem =>
    ({ ...item(label, processed, 0), repo, name: "imagegen-backend" }) as UsageItem;

  it("puts worktrees of other tools under one repository row, with a count", () => {
    const rows = workspaceRows(
      [
        inRepo(`${repo}/app`, 100),
        inRepo("/Users/h0per/.paradev/worktrees/imagegen-backend/feat-a", 300),
        inRepo("/Users/h0per/.superset/worktrees/imagegen-backend/feat-b", 200),
        item("/Users/h0per/scratch", 50, 0),
      ],
      "tokens",
    );
    expect(rows.map((r) => [r.kind, r.short, r.label, r.count, r.inGroup])).toEqual([
      ["group", "imagegen-backend", repo, 3, false],
      ["folder", "feat-a", "/Users/h0per/.paradev/worktrees/imagegen-backend/feat-a", 0, true],
      ["folder", "feat-b", "/Users/h0per/.superset/worktrees/imagegen-backend/feat-b", 0, true],
      ["folder", "app", `${repo}/app`, 0, true],
      ["folder", "scratch", "/Users/h0per/scratch", 0, false],
    ]);
    expect(rows[0].processed).toBe(600);
  });

  it("leaves a folder with no repository as a row of its own", () => {
    const rows = workspaceRows([item("/tmp/loose", 5, 0)], "tokens");
    expect(rows.map((r) => [r.kind, r.short, r.count])).toEqual([["folder", "loose", 0]]);
  });
});
