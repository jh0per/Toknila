import { describe, expect, it } from "vitest";
import type { ChainDetail, EventRecord, Project, TicketItem } from "../api/types";
import { reduce } from "./reduce";
import { type State, initialState } from "./state";

function chain(id: string, name: string, ticketIds: string[]): ChainDetail {
  return {
    chain: { id, name, goal: "", autostart: true, project_id: "p1" },
    tickets: ticketIds.map((t, i) => ({
      id: t,
      key: `${name === "demo" ? "DEMO" : "OTHER"}-${i + 1}`,
      chain_id: id,
      title: t,
      prompt: "",
      acceptance: [],
      needs_secrets: false,
      paused: false,
      status: "backlog",
      status_since: 0,
      tokens_used: 0,
    })),
    releases: [],
    edges: [],
    release_tickets: [],
    order: [],
    summary: { status: "running", total: ticketIds.length, done: 0, released: 0 },
  } as ChainDetail;
}

function event(seq: number, entity: string, kind: string, payload: unknown): EventRecord {
  return { seq, at: 0, actor: "system", entity, entity_id: "", kind, payload };
}

const projects = [
  { id: "p1", key: "DEMO", name: "Demo" },
  { id: "p2", key: "OTHER", name: "Other" },
] as Project[];

function loaded(): State {
  const tickets = [{ ticket: { id: "t1" }, needs_human: true }] as TicketItem[];
  const [s] = reduce(initialState({ name: "chains" }), {
    type: "loaded",
    projects,
    chains: [chain("c1", "demo", ["t1", "t2"]), chain("c2", "other", ["t3"])],
    tickets,
  });
  return { ...s, lastSeq: 10 };
}

describe("connection", () => {
  it("reads the journal tail, then connects after it", () => {
    const s0 = initialState({ name: "chains" });
    expect(reduce(s0, { type: "started" })[1]).toEqual([{ type: "fetchTail" }]);
    const [s1, fx] = reduce(s0, { type: "journalTail", seq: 42, events: [] });
    expect(s1.lastSeq).toBe(42);
    expect(fx).toEqual([{ type: "connect", since: 42, delayMs: 0 }]);
  });

  it("loads everything when the stream opens", () => {
    const [s, fx] = reduce(initialState({ name: "chains" }), { type: "linkOpened" });
    expect(s.link).toBe("live");
    expect(fx).toEqual([{ type: "loadAll" }]);
  });

  it("reconnects with doubling backoff up to ten seconds, resuming after the last seq", () => {
    let s = { ...initialState({ name: "chains" }), lastSeq: 7 };
    const delays: number[] = [];
    for (let i = 0; i < 7; i++) {
      const [next, fx] = reduce(s, { type: "linkClosed" });
      expect(next.link).toBe("offline");
      const connect = fx[0];
      if (connect.type !== "connect") throw new Error("expected connect");
      expect(connect.since).toBe(7);
      delays.push(connect.delayMs);
      s = next;
    }
    expect(delays).toEqual([500, 1000, 2000, 4000, 8000, 10000, 10000]);
    expect(reduce(s, { type: "linkOpened" })[0].retryMs).toBe(500);
  });
});

describe("journal events", () => {
  it("refresh only the chain they touch and the inbox", () => {
    const e = event(11, "ticket", "status_changed", { ticket: { chain_id: "c2" } });
    const [s, fx] = reduce(loaded(), { type: "event", event: e });
    expect(s.lastSeq).toBe(11);
    expect(fx).toEqual([{ type: "loadChain", id: "c2" }, { type: "loadTickets" }]);
  });

  it("place comments and subtasks by their ticket", () => {
    const comment = event(11, "comment", "added", { ticket: "DEMO-2", comment: { ticket_id: "t2" } });
    expect(reduce(loaded(), { type: "event", event: comment })[1][0]).toEqual({ type: "loadChain", id: "c1" });
    const sub = event(11, "subtask", "updated", { ticket: "DEMO-3", subtask: { ticket_id: "t3" } });
    expect(reduce(loaded(), { type: "event", event: sub })[1][0]).toEqual({ type: "loadChain", id: "c2" });
  });

  it("reload everything after a gap or an event the page cannot place", () => {
    const gap = event(13, "ticket", "updated", { ticket: { chain_id: "c1" } });
    const [s, fx] = reduce(loaded(), { type: "event", event: gap });
    expect([s.lastSeq, fx]).toEqual([13, [{ type: "loadAll" }]]);
    const unknown = event(11, "runner", "started", {});
    expect(reduce(loaded(), { type: "event", event: unknown })[1]).toEqual([{ type: "loadAll" }]);
  });

  it("ignore what was already applied; a project event reloads the projects", () => {
    expect(reduce(loaded(), { type: "event", event: event(9, "ticket", "updated", {}) })).toEqual([loaded(), []]);
    const [s, fx] = reduce(loaded(), { type: "event", event: event(11, "project", "created", {}) });
    expect([s.lastSeq, fx]).toEqual([11, [{ type: "loadProjects" }]]);
  });

  it("add a chain the page has not seen at the end", () => {
    const [s] = reduce(loaded(), { type: "chainLoaded", chain: chain("c3", "new", []) });
    expect(s.chainOrder).toEqual(["c1", "c2", "c3"]);
  });
});

