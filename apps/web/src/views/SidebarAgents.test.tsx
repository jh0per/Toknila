import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ChainDetail, Project, ProjectAgents } from "../api/types";
import { reduce } from "../store/reduce";
import { initialState } from "../store/state";
import { agentsOf } from "../store/tree";
import { Sidebar } from "./Sidebar";

const project = (id: string, key: string) => ({ id, key, name: key, archived: false }) as unknown as Project;
const chain = (id: string, p: string, ticket: string): ChainDetail =>
  ({
    chain: { id, name: id, project_id: p, autostart: true },
    tickets: [{ id: ticket, key: `${p}-1`, title: "Card", status: "running" }],
    releases: [], edges: [], release_tickets: [], order: [],
    summary: { status: "running", total: 1, done: 0, released: 0 },
  }) as unknown as ChainDetail;
const agents = (p: string, runs: ProjectAgents["runs"]): ProjectAgents => ({ project: p, limit: 2, used: runs.length, queued: [], runs });
const run = (id: string, key: string, role: "worker" | "reviewer") => ({ card: { id, key, title: `Card ${key}` }, role, since: 1 });

const world = () => {
  const s = initialState({ name: "chain", project: "AAA", chain: "ca" } as never);
  return {
    ...s,
    projects: [project("pa", "AAA"), project("pb", "BBB")],
    chains: { ca: chain("ca", "AAA", "ta"), cb: chain("cb", "BBB", "tb") },
    chainOrder: ["ca", "cb"],
    agents: [
      agents("pa", [{ card: null, role: "planner", since: 1 }, run("ta", "AAA-1", "worker")]),
      agents("pb", [run("tb", "BBB-1", "reviewer")]),
    ],
  };
};

describe("the sidebar's Agents section", () => {
  beforeEach(() => vi.stubGlobal("window", { location: { search: "" } }));
  afterEach(() => vi.unstubAllGlobals());

  it("lists the assistant first, then the cards with their role, and nothing of another project", () => {
    const s = world();
    expect(agentsOf(s, { id: "pa" }).runs.map((r) => r.card?.key ?? "assistant")).toEqual(["assistant", "AAA-1"]);
    const html = renderToStaticMarkup(<Sidebar state={s} dispatch={() => {}} />);
    const section = html.slice(html.indexOf('data-testid="running"'));
    expect(section).toContain("Assistant");
    expect(section.indexOf("Assistant")).toBeLessThan(section.indexOf("AAA-1"));
    expect(section).toContain("worker");
    expect(section).toContain('href="/projects/AAA/planner"');
    expect(section).not.toContain("BBB-1");
  });

  it("shows nothing running for a project without runs", () => {
    const s = { ...world(), agents: [agents("pb", [run("tb", "BBB-1", "reviewer")])] };
    const html = renderToStaticMarkup(<Sidebar state={s} dispatch={() => {}} />);
    expect(html).toContain("None running.");
    expect(html).not.toContain("BBB-1");
  });
});

describe("the Agents and Inbox nodes of a project", () => {
  beforeEach(() => vi.stubGlobal("window", { location: { search: "" }, localStorage: { getItem: () => "1", setItem: () => {} } }));
  afterEach(() => vi.unstubAllGlobals());

  it("each Agents row says running out of slots; a card shows only under its own project, once that opens", () => {
    const onB = { ...world(), route: { name: "project", project: "BBB" } } as never as ReturnType<typeof world>;
    const html = renderToStaticMarkup(<Sidebar state={onB} dispatch={() => {}} />);
    expect(html).toContain(">2 / 2<");
    expect(html).toContain(">1 / 2<");
    const b = html.indexOf('data-project="BBB"');
    expect(html.indexOf("BBB-1")).toBeGreaterThan(b);
    expect(html.slice(0, b)).not.toContain("BBB-1");
    expect(html.slice(b)).not.toContain("AAA-1");
  });

  it("a waiting question shows under its own project's Inbox only, and the total stays", () => {
    const s = { ...world(), inbox: [{ kind: "question", ticket: "BBB-1", chain: "cb", project: "pb" }] } as never as ReturnType<typeof world>;
    const html = renderToStaticMarkup(<Sidebar state={s} dispatch={() => {}} />);
    const a = html.indexOf('data-project="AAA"');
    const b = html.indexOf('data-project="BBB"');
    expect(html.slice(a, b)).toContain('data-testid="project-inbox-count">0<');
    expect(html.slice(b)).toContain('data-testid="project-inbox-count">1<');
    expect(html).toContain('data-testid="inbox-count">1<');
    expect(html).toContain('href="/projects/BBB/inbox"');
  });
});

describe("the assistant starting or stopping", () => {
  it("reloads what runs, once, when the answer changes whether it runs", () => {
    const s0 = initialState({ name: "chains" });
    const on = { running: true, profile: "p" } as never;
    const [s1, fx1] = reduce(s0, { type: "plannerLoaded", project: "AAA", state: on });
    expect(fx1).toEqual([{ type: "loadAgents" }]);
    expect(reduce(s1, { type: "plannerLoaded", project: "AAA", state: on })[1]).toEqual([]);
    const off = { running: false, profile: "p" } as never;
    expect(reduce(s1, { type: "plannerLoaded", project: "AAA", state: off })[1]).toEqual([{ type: "loadAgents" }]);
  });
});
