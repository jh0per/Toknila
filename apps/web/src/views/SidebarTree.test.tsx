import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { ChainDetail, Project } from "../api/types";
import { initialState } from "../store/state";
import { Sidebar } from "./Sidebar";

const project = (id: string, key: string, archived = false) => ({ id, key, name: `${key} name`, archived }) as unknown as Project;
const chain = (id: string, p: string): ChainDetail =>
  ({
    chain: { id, name: id, project_id: p, autostart: true },
    tickets: [], releases: [], edges: [], release_tickets: [], order: [],
    summary: { status: "idle", phase: "idle", total: 2, done: 1, finished_at: null },
  }) as unknown as ChainDetail;

const world = (stored: Record<string, string> = {}) => {
  vi.stubGlobal("window", {
    location: { search: "" },
    localStorage: { getItem: (k: string) => stored[k] ?? null, setItem: () => {} },
  });
  return {
    ...initialState({ name: "project", project: "AAA" } as never),
    projects: [project("pa", "AAA"), project("pb", "BBB"), project("pc", "CCC", true)],
    chains: { ca: chain("ca", "pa"), cb: chain("cb", "pb") },
    chainOrder: ["ca", "cb"],
  };
};

const html = (s: ReturnType<typeof world>) => renderToStaticMarkup(<Sidebar state={s} dispatch={() => {}} />);

describe("the sidebar tree", () => {
  afterEach(() => vi.unstubAllGlobals());

  it("is a tree of treeitems, a project closed unless it is on the screen or was opened", () => {
    const h = html(world());
    expect(h).toContain('role="tree"');
    expect(h).toContain('<li role="treeitem" aria-expanded="true" data-project="AAA"');
    expect(h).toContain('<li role="treeitem" aria-expanded="false" data-project="BBB"');
    expect(h).not.toContain('data-chain-item="cb"');
    expect(h).toContain('data-chain-item="ca"');
  });
  it("lists Roadmap, Assistant, Epics, Designs and Settings under an open project, in that order", () => {
    const open = html(world()).split('data-project="AAA"')[1].split('data-project="BBB"')[0];
    const order = ["Roadmap", "Assistant", "Epics", "Designs", "Settings"].map((w) => open.indexOf(`>${w}<`));
    expect(order.every((i) => i > 0)).toBe(true);
    expect([...order].sort((a, b) => a - b)).toEqual(order);
    expect(open).toContain('data-testid="planner-link"');
    // The Assistant row opens the roadmap (with the panel), says what it is, and no longer links to the screen.
    expect(open).toContain('title="plans epics on the roadmap, stories inside an epic"');
    expect(open).not.toContain("/projects/AAA/planner");
    expect(open).toContain('data-testid="new-chain-plus"');
    expect(open).toContain('data-testid="import-plus"');
    expect(open).toContain("/projects/AAA/settings/designs");
  });
  it("opens a project the person left open, and shows its chains", () => {
    const h = html(world({ "toknila.project-open.BBB": "1" }));
    expect(h).toContain('aria-expanded="true" data-project="BBB"');
    expect(h).toContain('data-chain-item="cb"');
  });
  it("keeps the project on the screen open though it was stored closed", () => {
    const h = html(world({ "toknila.project-open.AAA": "0" }));
    expect(h).toContain('aria-expanded="true" data-project="AAA"');
  });
  it("shows a waiting badge and a running dot on a closed project", () => {
    const s = world();
    const w = {
      ...s,
      inbox: [{ kind: "question", chain: "cb" }] as never,
      agents: [{ project: "pb", limit: 1, used: 1, queued: [], runs: [{ card: null, role: "planner", since: 1 }] }] as never,
    };
    const b = html(w).split('data-project="BBB"')[1];
    expect(b).toContain("project-waiting");
    expect(b).toContain("An agent is running");
  });
  it("puts the full name in a title so a long one can end in an ellipsis", () => {
    expect(html(world())).toContain('title="BBB name"');
  });
  it("tells the kinds apart: an initials badge on a project, an icon on a node, an icon per foot row", () => {
    const h = html(world());
    const a = h.split('data-project="AAA"')[1].split('data-project="BBB"')[0];
    expect(a).toContain('class="pbadge" aria-hidden="true" data-testid="project-badge" data-initials="AA"');
    expect(h.split('data-project="BBB"')[1]).toContain('data-initials="BB"');
    for (const n of ["roadmap", "assistant", "inbox", "agents", "chains", "designs", "settings"]) expect(a).toContain(`data-icon="${n}"`);
    const chain = a.split('data-chain-item="ca"')[1].split("</li>")[0];
    expect(chain).not.toContain("data-icon");
    const foot = h.split('class="side-foot"')[1];
    for (const n of ["usage", "settings", "guide", "shortcuts"]) expect(foot).toContain(`data-icon="${n}"`);
    expect(foot).toContain("<kbd>?</kbd>");
  });
  it("lists three other projects as rows and folds the rest into one '+ N more' row", () => {
    expect(html(world())).not.toContain("sidebar-more");
    const s = world();
    const extra = ["DDD", "EEE", "FFF"].map((k) => project(`p${k}`, k));
    const h = html({ ...s, projects: [...s.projects, ...extra] });
    // AAA is on the screen, BBB, DDD and EEE are the three rows, FFF waits behind the count.
    for (const k of ["AAA", "BBB", "DDD", "EEE"]) expect(h).toContain(`data-project="${k}"`);
    expect(h).not.toContain('data-project="FFF"');
    expect(h).toContain("+ 1 more project<");
    const two = html({ ...s, projects: [...s.projects, ...["DDD", "EEE", "FFF", "GGG"].map((k) => project(`p${k}`, k))] });
    expect(two).toContain("+ 2 more projects<");
  });
  it("lists an archived project only when asked", () => {
    expect(html(world())).not.toContain('data-project="CCC"');
    const s = world();
    const h = html({ ...s, plan: { ...s.plan, showArchived: true } });
    expect(h).toContain('data-project="CCC"');
    expect(h).toContain("unarchive-CCC");
  });
});
