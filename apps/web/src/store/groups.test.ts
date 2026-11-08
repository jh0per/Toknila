import { describe, expect, it } from "vitest";
import type { TicketStatus } from "../api/types";
import { statusGroup, statusText } from "./groups";

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
