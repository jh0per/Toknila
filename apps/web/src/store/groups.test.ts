import { describe, expect, it } from "vitest";
import type { TicketStatus } from "../api/types";
import { GROUP_LABEL, GROUP_ORDER, groupBy, statusGroup } from "./groups";

describe("statusGroup", () => {
  it("maps every status to its display group", () => {
    const expected: Record<TicketStatus, string> = {
      proposed: "awaiting_approval",
      backlog: "todo",
      ready: "todo",
      running: "in_progress",
      waiting: "in_progress",
      review: "in_progress",
      blocked: "blocked",
      merged: "done",
      released: "done",
      cancelled: "cancelled",
    };
    for (const [status, group] of Object.entries(expected)) {
      expect(statusGroup(status as TicketStatus)).toBe(group);
    }
  });

  it("keeps a proposed card out of todo, with its own label", () => {
    const g = groupBy([{ s: "proposed" }, { s: "backlog" }] as { s: TicketStatus }[], (i) => i.s);
    expect(g.awaiting_approval).toHaveLength(1);
    expect(g.todo).toHaveLength(1);
    expect(GROUP_LABEL.awaiting_approval).toBe("awaiting approval");
    expect(GROUP_ORDER[0]).toBe("awaiting_approval");
  });

  it("splits a list, keeping order and showing cancelled apart", () => {
    const items = [
      { k: 1, s: "ready" },
      { k: 2, s: "cancelled" },
      { k: 3, s: "backlog" },
      { k: 4, s: "released" },
    ] as { k: number; s: TicketStatus }[];
    const g = groupBy(items, (i) => i.s);
    expect(g.todo.map((i) => i.k)).toEqual([1, 3]);
    expect(g.cancelled.map((i) => i.k)).toEqual([2]);
    expect(g.done.map((i) => i.k)).toEqual([4]);
    expect(g.blocked).toEqual([]);
  });
});
