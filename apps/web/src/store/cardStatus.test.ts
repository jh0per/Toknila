import { describe, expect, it } from "vitest";
import type { StatusReason, Ticket, TicketDetail, TicketStatus } from "../api/types";
import { type Pace, statusLine } from "./cardStatus";

const card = (status: TicketStatus, extra: Partial<Ticket> = {}) =>
  ({ key: "T-1", status, paused: false, ...extra }) as Ticket;
const detail = (allowed: string[], awaiting?: TicketDetail["review"]["awaiting"]) =>
  ({ allowed_actions: allowed, review: { rounds: [], awaiting } }) as unknown as TicketDetail;
const pace = (over: Partial<Pace> = {}): Pace => ({ queue: undefined, autostart: true, ...over });

describe("the status sentence", () => {
  it("says a waiting card waits for your answer, with its question as the next step", () => {
    const reason = { status: "waiting", kind: "question", text: "Which database?" } as StatusReason;
    const line = statusLine(card("waiting", { status_reason: reason }), detail([]), pace());
    expect(line).toEqual({ tone: "needs", text: "Waiting for your answer", next: "Which database?" });
  });

  it("says a card waits for you to approve its design, with a link to Threads", () => {
    const d = { ...detail([]), design_waiting: true, phase: "design" } as TicketDetail;
    const line = statusLine(card("running", { design_first: true }), d, pace());
    expect(line.tone).toBe("needs");
    expect(line.text).toBe("Waiting for you to approve the design");
    expect(line.next).toContain("approve it or ask for changes");
    expect(line.tab).toEqual({ tab: "threads", label: "Open the design" });
    const silent = { status: "waiting", kind: "silent" } as StatusReason;
    expect(statusLine(card("waiting", { status_reason: silent }), d, pace()).text).toBe("Waiting for you to approve the design");
  });

  it("keeps a question as the sentence of a waiting card, even with a design on the table", () => {
    const d = { ...detail([]), design_waiting: true, phase: "design" } as TicketDetail;
    const reason = { status: "waiting", kind: "question", text: "Which colour?" } as StatusReason;
    expect(statusLine(card("waiting", { status_reason: reason }), d, pace()).text).toBe("Waiting for your answer");
  });

  it("says the agent works on the design while the thread is open and nothing waits for you", () => {
    const d = { ...detail([]), design_waiting: false, phase: "design" } as TicketDetail;
    const line = statusLine(card("running"), d, pace());
    expect(line.text).toBe("An agent is working on the design");
    expect(line.tab).toBeUndefined();
    expect(statusLine(card("running"), { ...d, phase: "implement" } as TicketDetail, pace()).text).toBe("An agent is working on it");
  });

  it("says a ready card starts when a slot is free, with its place in line", () => {
    expect(statusLine(card("ready"), detail(["start"]), pace()).text).toBe("Ready: starts when a slot is free");
    expect(statusLine(card("ready"), detail(["start"]), pace({ queue: 3 })).text).toBe("Ready: starts when a slot is free (#3 in line)");
    expect(statusLine(card("ready"), detail(["start"]), pace({ queue: 1 })).text).toBe("Ready: starts when a slot is free (next in line)");
  });

  it("asks for Start when the chain does not start cards by itself", () => {
    const line = statusLine(card("ready"), detail(["start"]), pace({ autostart: false }));
    expect(line.tone).toBe("needs");
    expect(line.text).toBe("Ready: start it when you want");
  });

  it("points a card in review at the report, and says who reviews now", () => {
    expect(statusLine(card("review"), detail(["accept"], { by: "person" }), pace()).text)
      .toBe("In review: read the agent's report below, then Accept or Return");
    expect(statusLine(card("review"), detail([], { by: "agent", decides: true }), pace()).text)
      .toBe("In review: the reviewer agent is checking the work");
    expect(statusLine(card("review"), detail([], { by: "check" }), pace()).text)
      .toBe("In review: bringing the ship up to main");
    const waiting = (place: number) => {
      const d = detail([], { by: "check" });
      d.review = { ...d.review, check_queue: place };
      return statusLine(card("review"), d, pace()).text;
    };
    expect(waiting(0)).toBe("In review: bringing the ship up to main");
    expect(waiting(1)).toBe("In review: waits for another check to end (next in line)");
    expect(waiting(3)).toBe("In review: waits for another check to end (#3 in line)");
    expect(statusLine(card("review"), detail([], { by: "merging" }), pace()).text)
      .toBe("Approved: merging into the main branch");
  });

  it("says done, and cancelled with its reason", () => {
    expect(statusLine(card("merged"), detail([]), pace())).toEqual({ tone: "done", text: "Done" });
    const reason = { status: "cancelled", text: "not needed any more" } as StatusReason;
    expect(statusLine(card("cancelled", { status_reason: reason }), detail([]), pace()).text)
      .toBe("Cancelled: not needed any more");
  });

  it("words the other statuses too", () => {
    expect(statusLine(card("proposed"), detail(["approve", "reject"]), pace()).text).toBe("Proposed: waiting for your approval");
    expect(statusLine(card("backlog", { paused: true }), detail(["resume"]), pace()).text)
      .toBe("Paused: it will not start until you resume it");
    expect(statusLine(card("backlog"), detail(["start"]), pace()).next).toBe("You can also start it now.");
    expect(statusLine(card("running"), detail([]), pace()).text).toBe("An agent is working on it");
    const blocked = { status: "blocked", reason: { kind: "manual", text: "waiting for keys" }, resume_to: "running" } as StatusReason;
    const line = statusLine(card("blocked", { status_reason: blocked }), detail(["unblock"]), pace());
    expect(line.text).toBe("Blocked: waiting for keys");
    expect(line.next).toBe("Fix what it says, then Unblock.");
  });
});
