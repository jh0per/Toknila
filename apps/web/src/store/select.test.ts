import { describe, expect, it } from "vitest";
import type { ChainDetail, EventRecord, Project } from "../api/types";
import { reduce } from "./reduce";
import { parseRoute, routePath } from "./route";
import {
  type State, currentCard, currentChain, currentProject, initialState, queuePosition, slotsOf, ticketEvents,
} from "./state";

const projects = [
  { id: "p1", key: "DEMO", name: "Demo" },
  { id: "p2", key: "OTHER", name: "Other" },
] as Project[];

function chain(id: string, name: string, key: string, ticketIds: string[]): ChainDetail {
  return {
    chain: { id, name, goal: "", autostart: true, project_id: "p1" },
    tickets: ticketIds.map((t, i) => ({ id: t, key: `${key}-${i + 1}`, chain_id: id, title: t, status: "backlog" })),
    releases: [], edges: [], release_tickets: [], order: [],
    summary: { status: "running", total: ticketIds.length, done: 0, released: 0 },
  } as unknown as ChainDetail;
}

function loaded(): State {
  const [s] = reduce(initialState({ name: "chains" }), {
    type: "loaded",
    projects,
    chains: [chain("c1", "demo", "DEMO", ["t1", "t2"]), chain("c2", "other", "OTH", ["t3"])],
  });
  return { ...s, lastSeq: 10 };
}

const event = (seq: number, entity: string, kind: string, payload: unknown, entity_id = ""): EventRecord =>
  ({ seq, at: 0, actor: "system", entity, entity_id, kind, payload }) as EventRecord;

describe("routes", () => {
  it("live in the URL", () => {
    const paths = [
      "/", "/chains/demo", "/chains/a%20b", "/inbox", "/projects/DEMO",
      "/projects/DEMO/chains/demo", "/projects/DEMO/chains/demo/cards/DEMO-1",
    ];
    for (const path of paths) expect(routePath(parseRoute(path))).toBe(path);
    expect(parseRoute("/nowhere/at/all")).toEqual({ name: "chains" });
    expect(parseRoute("/projects/DEMO/chains/demo/cards")).toEqual({ name: "chains" });
  });

  it("are an effect, never done by the reducer itself", () => {
    const [s, fx] = reduce(loaded(), { type: "navigate", route: { name: "chain", chain: "other" } });
    expect(fx).toEqual([{ type: "pushUrl", path: "/chains/other" }]);
    expect(currentChain(s)?.chain.id).toBe("c2");
    const [back] = reduce(s, { type: "urlChanged", route: { name: "inbox" } });
    expect(currentChain(back)).toBeUndefined();
  });
});

describe("the chain shown without a pick", () => {
  const at = (route: Parameters<typeof initialState>[0], events: EventRecord[] = []) => {
    const [s] = reduce(initialState(route), {
      type: "loaded",
      projects,
      chains: [chain("c1", "demo", "DEMO", ["t1", "t2"]), chain("c2", "other", "OTH", ["t3"])],
    });
    return { ...s, events };
  };

  it("is the project's first chain when the journal says nothing about them", () => {
    expect(currentChain(at({ name: "project", project: "DEMO" }))?.chain.id).toBe("c1");
    expect(currentChain(at({ name: "chains" }))?.chain.id).toBe("c1");
  });

  it("is the most recently active one: the chain of the newest journal entry", () => {
    const events = [
      event(1, "ticket", "status_changed", { ticket: { id: "t3", chain_id: "c2" } }),
      event(2, "comment", "added", { comment: { ticket_id: "t3" } }),
    ];
    expect(currentChain(at({ name: "project", project: "DEMO" }, events))?.chain.id).toBe("c2");
    const later = [...events, event(3, "chain", "updated", { chain: { id: "c1" } })];
    expect(currentChain(at({ name: "project", project: "DEMO" }, later))?.chain.id).toBe("c1");
  });

  it("is none for a project without chains, and a picked chain wins", () => {
    expect(currentChain(at({ name: "project", project: "OTHER" }))).toBeUndefined();
    expect(currentChain(at({ name: "projectChain", project: "DEMO", chain: "other" }))?.chain.id).toBe("c2");
  });

  it("belongs to the first project that is not archived, at home", () => {
    const [s] = reduce(initialState({ name: "chains" }), {
      type: "loaded",
      projects: [{ ...projects[0], archived: true }, projects[1]],
      chains: [chain("c1", "demo", "DEMO", ["t1"])],
    });
    expect(currentProject(s)?.key).toBe("OTHER");
    expect(currentChain(s)).toBeUndefined();
  });
});

describe("selection", () => {
  it("selects a project; the first project is the default", () => {
    expect(currentProject(loaded())?.key).toBe("DEMO");
    const [s, fx] = reduce(loaded(), { type: "selectProject", project: "OTHER" });
    expect(fx).toEqual([{ type: "pushUrl", path: "/projects/OTHER" }]);
    expect(currentProject(s)?.id).toBe("p2");
    expect(currentChain(s)).toBeUndefined();
  });

  it("selects a chain", () => {
    const [s, fx] = reduce(loaded(), { type: "selectChain", project: "DEMO", chain: "other" });
    expect(fx).toEqual([{ type: "pushUrl", path: "/projects/DEMO/chains/other" }]);
    expect(currentChain(s)?.chain.id).toBe("c2");
    expect(currentCard(s)).toBeUndefined();
  });

  it("selects a card, fetching its detail", () => {
    const [s, fx] = reduce(loaded(), { type: "selectCard", project: "DEMO", chain: "demo", key: "DEMO-2" });
    expect(fx).toEqual([
      { type: "pushUrl", path: "/projects/DEMO/chains/demo/cards/DEMO-2" },
      { type: "loadDetail", key: "DEMO-2" },
      { type: "loadThreads", key: "DEMO-2" },
    ]);
    expect(currentCard(s)?.id).toBe("t2");
  });

  it("fetches the detail of a card opened by a deep link once the data is there", () => {
    const route = { name: "card", project: "DEMO", chain: "demo", key: "DEMO-1" } as const;
    const [, fx] = reduce(initialState(route), {
      type: "loaded", projects, chains: [chain("c1", "demo", "DEMO", ["t1"])],
    });
    expect(fx).toEqual([
      { type: "loadDetail", key: "DEMO-1" },
      { type: "loadThreads", key: "DEMO-1" },
    ]);
  });

  it("switches the bottom tab without touching the route", () => {
    const s0 = loaded();
    expect(s0.tab).toBe("details");
    const [s, fx] = reduce(s0, { type: "selectTab", tab: "events" });
    expect([s.tab, s.route, fx]).toEqual(["events", s0.route, []]);
  });
});

describe("stream events", () => {
  it("refresh only the touched chain, plus the open card when it is the one touched", () => {
    const [open] = reduce(loaded(), { type: "selectCard", project: "DEMO", chain: "demo", key: "DEMO-1" });
    const detail = { ticket: { id: "t1", key: "DEMO-1" }, subtasks: [], comments: [] } as never;
    const [s] = reduce(open, { type: "detailLoaded", detail });
    const other = event(11, "ticket", "updated", { ticket: { id: "t3", chain_id: "c2" } });
    expect(reduce(s, { type: "event", event: other })[1]).toEqual([
      { type: "loadChain", id: "c2" }, { type: "loadInbox" }, { type: "loadAgents" },
    ]);
    const mine = event(11, "ticket", "status_changed", { ticket: { id: "t1", chain_id: "c1" } });
    expect(reduce(s, { type: "event", event: mine })[1]).toEqual([
      { type: "loadChain", id: "c1" }, { type: "loadInbox" }, { type: "loadAgents" },
      { type: "loadDetail", key: "DEMO-1" },
    ]);
  });

  it("build a ticket's journal live", () => {
    const first = event(11, "ticket", "status_changed", { ticket: { id: "t1", chain_id: "c1" } }, "t1");
    const noise = event(12, "ticket", "updated", { ticket: { id: "t3", chain_id: "c2" } }, "t3");
    let s = loaded();
    for (const e of [first, noise]) s = reduce(s, { type: "event", event: e })[0];
    expect(ticketEvents(s, "t1").map((e) => e.seq)).toEqual([11]);
    expect(ticketEvents(s, "t3").map((e) => e.seq)).toEqual([12]);
  });

  it("keep the inbox the daemon computed, as it came", () => {
    const items = [{ kind: "question", ticket: "DEMO-1" }, { kind: "blocked", ticket: "DEMO-2" }] as never;
    const [s, fx] = reduce(loaded(), { type: "inboxLoaded", items });
    expect(s.inbox).toBe(items);
    expect(fx).toEqual([]);
  });

  it("a leftovers decision is an effect, not a state change", () => {
    const intent = { type: "leftovers", key: "DEMO-1", action: "move" } as const;
    const s = loaded();
    expect(reduce(s, intent)).toEqual([s, [intent]]);
  });
});

describe("agent slots", () => {
  const agents = [
    { project: "p1", limit: 2, used: 2, queued: [{ ticket: "t2", position: 1 }, { ticket: "t3", position: 2 }] },
    { project: "p2", limit: 4, used: 0, queued: [] },
  ];

  it("show the slots of the project in focus and the place of each waiting card", () => {
    const [s] = reduce(loaded(), { type: "agentsLoaded", agents });
    expect(slotsOf(s)).toEqual({ used: 2, limit: 2 });
    const [inDemo] = reduce(s, { type: "selectProject", project: "DEMO" });
    expect(slotsOf(inDemo)).toEqual({ used: 2, limit: 2 });
    const [inOther] = reduce(s, { type: "selectProject", project: "OTHER" });
    expect(slotsOf(inOther)).toEqual({ used: 0, limit: 4 });
    expect([queuePosition(s, "t2"), queuePosition(s, "t3"), queuePosition(s, "t1")]).toEqual([1, 2, undefined]);
  });

  it("are refreshed by a journal event and by a project change", () => {
    const project = event(11, "project", "updated", { project: { id: "p1" } });
    expect(reduce(loaded(), { type: "event", event: project })[1]).toEqual([
      { type: "loadProjects" }, { type: "loadAgents" },
    ]);
  });
});
