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

describe("hidden projects in the sidebar", () => {
  afterEach(() => vi.unstubAllGlobals());

  const hidden = (id: string, key: string) => ({ ...project(id, key), hidden: true }) as Project;
  /// The sidebar's list: the tree named Projects, up to the rows under it.
  const list = (h: string) =>
    h.split('aria-label="Projects"')[1].split(/data-testid="show-(?:hidden|archived)"|class="side-foot"/)[0];
  const withHidden = (route = "AAA", extra: Project[] = []) => {
    const s = world();
    return {
      ...s,
      route: { name: "project", project: route } as never,
      projects: [...s.projects, hidden("pd", "DDD"), hidden("pe", "EEE"), ...extra],
      chains: { ...s.chains, cd: chain("cd", "pd") },
      chainOrder: [...s.chainOrder, "cd"],
    };
  };

  it("leaves a hidden project out of the list and offers 'Show hidden (N)' under it, above 'Show archived'", () => {
    const h = html(withHidden());
    expect(list(h)).toContain('data-project="BBB"');
    expect(list(h)).not.toContain('data-project="DDD"');
    expect(h).not.toContain('data-project="DDD"');
    expect(h).toContain("Show hidden (2)");
    expect(h.indexOf('data-testid="show-hidden"')).toBeGreaterThan(h.indexOf('aria-label="Projects"'));
    expect(h.indexOf('data-testid="show-hidden"')).toBeLessThan(h.indexOf('data-testid="show-archived"'));
  });

  it("does not count a hidden project in '+ N more'", () => {
    // AAA on the screen, BBB, FFF and GGG are the three rows: nothing more, though DDD and EEE exist.
    const h = html(withHidden("AAA", [project("pf", "FFF"), project("pg", "GGG")]));
    expect(h).not.toContain("sidebar-more");
    const one = html(withHidden("AAA", ["FFF", "GGG", "HHH"].map((k) => project(`p${k}`, k))));
    expect(one).toContain("+ 1 more project<");
  });

  it("lists the hidden projects as rows when asked: badge, name and Show in list, no tree", () => {
    const s = withHidden();
    const h = html({ ...s, plan: { ...s.plan, showHidden: true } });
    const group = h.split('data-testid="hidden-projects"')[1].split("</ul>")[0];
    for (const k of ["DDD", "EEE"]) {
      expect(group).toContain(`data-project="${k}"`);
      expect(group).toContain(`data-testid="show-in-list-${k}"`);
    }
    expect(group).toContain('data-initials="DD"');
    expect(group).not.toContain('role="group"');
    expect(group).not.toContain("aria-expanded");
    expect(list(h)).not.toContain('data-project="DDD"');
  });

  it("says on the closed row what waits in hidden projects and whether an agent of one runs", () => {
    const s = withHidden();
    const w = {
      ...s,
      inbox: [{ kind: "question", project: "pd" }, { kind: "review", chain: "cd" }, { kind: "question", project: "pe" }, { kind: "question", project: "pb" }] as never,
      agents: [{ project: "pe", limit: 1, used: 1, queued: [], runs: [{ card: null, role: "planner", since: 1 }] }] as never,
    };
    const row = html(w).split('data-testid="show-hidden"')[1].split("</button>")[0];
    expect(row).toContain("Show hidden (2)");
    expect(row).toContain('data-testid="hidden-waiting"');
    expect(row).toContain(">3<");
    expect(row).toContain('data-testid="hidden-running"');
    const quiet = html(withHidden()).split('data-testid="show-hidden"')[1].split("</button>")[0];
    expect(quiet).not.toContain("hidden-waiting");
    expect(quiet).not.toContain("hidden-running");
    // Opened, each row says its own.
    const open = html({ ...w, plan: { ...w.plan, showHidden: true } });
    const d = open.split('data-testid="hidden-projects"')[1].split('data-project="EEE"')[0];
    expect(d).toContain('data-testid="project-waiting">2<');
  });

  it("lists a hidden project while it is on the screen, with its tree, the hidden mark and Show in list", () => {
    const h = html(withHidden("DDD"));
    const l = list(h);
    expect(l).toContain('<li role="treeitem" aria-expanded="true" data-project="DDD"');
    expect(l).toContain('data-chain-item="cd"');
    const row = l.split('data-project="DDD"')[1].split('role="group"')[0];
    expect(row).toContain('data-testid="hidden-mark"');
    expect(row).toContain('data-testid="show-in-list-DDD"');
    // It is in the list only, not in the hidden group too.
    expect(h).toContain("Show hidden (1)");
    // Elsewhere it leaves the list again.
    expect(list(html(withHidden("AAA")))).not.toContain('data-project="DDD"');
  });
});
