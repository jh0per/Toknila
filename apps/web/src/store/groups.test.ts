import { describe, expect, it } from "vitest";
import type { TicketStatus } from "../api/types";
import { chainTone, statusClass, statusGroup, statusText } from "./groups";

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
