import { describe, expect, it } from "vitest";
import type { TicketStatus } from "../api/types";
import { GROUP_LABEL, GROUP_ORDER, chainTone, groupBy, statusClass, statusGroup, statusText } from "./groups";

describe("chainTone and statusClass", () => {
  it("reads a chain that needs a person as attention and a running one as progress", () => {
    expect(chainTone("needs_human")).toBe("attn");
    expect(chainTone("blocked")).toBe("attn");
    expect(chainTone("running")).toBe("prog");
    expect(chainTone("done")).toBe("done");
    expect(chainTone("idle")).toBe("todo");
    expect(chainTone("not_started")).toBe("todo");
    expect(chainTone("empty")).toBe("todo");
  });

  it("gives every fine status its own class", () => {
    const all = ["proposed", "backlog", "ready", "running", "waiting", "blocked", "review", "merged", "released", "cancelled"] as const;
    expect(new Set(all.map(statusClass)).size).toBe(all.length);
    expect(statusClass("waiting")).toBe("s-waiting");
  });
});

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

describe("statusText", () => {
  it("shows a waiting card as in progress, never as blocked", () => {
    expect(statusText("waiting")).toBe("in progress · waiting");
    expect(statusText("running")).toBe("in progress · running");
    expect(statusText("blocked")).toBe("blocked");
    expect(statusText("backlog")).toBe("todo · backlog");
    expect(statusText("merged")).toBe("done · merged");
  });
});
