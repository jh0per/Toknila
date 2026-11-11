import { describe, expect, it } from "vitest";
import type { ChainDetail, EventRecord, Project } from "../api/types";
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
    summary: { status: "running", total: ticketIds.length, done: 0, released: 0, releasable: false },
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
  const [s] = reduce(initialState({ name: "chains" }), {
    type: "loaded",
    projects,
    chains: [chain("c1", "demo", ["t1", "t2"]), chain("c2", "other", ["t3"])],
  });
  return { ...s, lastSeq: 10 };
}

describe("connection", () => {
  it("reads the journal tail, then connects after it", () => {
    const s0 = initialState({ name: "chains" });
    expect(reduce(s0, { type: "started" })[1]).toEqual([{ type: "fetchTail" }, { type: "watchUpdates" }]);
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
    expect(fx).toEqual([{ type: "loadChain", id: "c2" }, { type: "loadInbox" }, { type: "loadAgents" }]);
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
    expect([s.lastSeq, fx]).toEqual([11, [{ type: "loadProjects" }, { type: "loadAgents" }]]);
  });

  it("add a chain the page has not seen at the end", () => {
    const [s] = reduce(loaded(), { type: "chainLoaded", chain: chain("c3", "new", []) });
    expect(s.chainOrder).toEqual(["c1", "c2", "c3"]);
  });
});

describe("threads", () => {
  const card: State = { ...loaded(), route: { name: "card", project: "DEMO", chain: "demo", key: "DEMO-1" } };
  const threads = { ticket: "DEMO-1", phase: "design", threads: [] } as never;

  it("opening a card loads its details and its threads", () => {
    const [, fx] = reduce(loaded(), { type: "selectCard", project: "DEMO", chain: "demo", key: "DEMO-1" });
    expect(fx).toContainEqual({ type: "loadThreads", key: "DEMO-1" });
  });

  it("keeps the answer by ticket key", () => {
    const [s] = reduce(card, { type: "threadsLoaded", threads });
    expect(s.threads["DEMO-1"]).toBe(threads);
  });

  it("a thread event of the open card reloads its threads", () => {
    const e = event(11, "thread", "opened", { ticket: "DEMO-1" });
    const [, fx] = reduce(card, { type: "event", event: e });
    expect(fx).toContainEqual({ type: "loadThreads", key: "DEMO-1" });
  });

  it("a thread action is an effect, not a state change", () => {
    const intent = { type: "threadAction", key: "DEMO-1", thread: "th1", action: "approve" } as const;
    const [s, fx] = reduce(card, intent);
    expect(s).toBe(card);
    expect(fx).toEqual([{ type: "threadAction", key: "DEMO-1", thread: "th1", action: "approve" }]);
  });

  it("a card action and a comment are effects, not state changes", () => {
    const action = { type: "ticketAction", key: "DEMO-1", action: "return", text: "fix it" } as const;
    expect(reduce(card, action)).toEqual([card, [action]]);
    const comment = { type: "addComment", key: "DEMO-1", kind: "instruction", text: "go on" } as const;
    expect(reduce(card, comment)).toEqual([card, [comment]]);
  });

  it("an answer to an agent is an effect carrying the role", () => {
    const answer = { type: "answer", key: "DEMO-1", role: "reviewer", answer: "deny", text: "no" } as const;
    expect(reduce(card, answer)).toEqual([card, [answer]]);
  });

  it("picking a version is remembered per thread", () => {
    const [s] = reduce(card, { type: "pickVersion", thread: "th1", number: 1 });
    expect(s.versionPick.th1).toBe(1);
  });
});

