import { describe, expect, it } from "vitest";
import type { ChainDetail, FoundPlan, ImportPathResult, PlanScan, Project } from "../api/types";
import type { Msg } from "./msg";
import { reduce } from "./reduce";
import { type State, initialState } from "./state";

const project = { id: "p1", key: "APP", name: "App", repo_path: "/work/app" } as Project;

function chain(id: string, name: string): ChainDetail {
  return {
    chain: { id, name, goal: "", autostart: false, project_id: "p1" },
    tickets: [{ id: `${id}-t`, key: "APP-1", chain_id: id, title: "t", status: "backlog" }],
    releases: [], edges: [], release_tickets: [], order: [],
    summary: { status: "not_started", total: 1, done: 0, released: 0 },
  } as unknown as ChainDetail;
}

const plan = (relative: string, imported = false): FoundPlan =>
  ({ kind: relative.endsWith(".toml") ? "chain_file" : "spec_folder", name: relative, path: `/work/app/${relative}`, relative, cards: 2, imported });

const scan: PlanScan = { plans: [plan("plans/x.toml"), plan("specs/001-a"), plan("specs/002-b", true)], specify_dir: true };

const imported = (...names: string[]): ImportPathResult => ({
  chains: names.map((n) => ({ folder: n, path: `/work/app/${n}`, cards: 1, tasks: 3, done_cards: 0, out_of_order: [], chain: chain(`c-${n}`, n) })),
  skipped: [],
  failed: [],
});

function loaded(route: State["route"] = { name: "project", project: "APP" }): State {
  return reduce(initialState(route), { type: "loaded", projects: [project], chains: [] })[0];
}

const run = (s: State, ...msgs: Msg[]): State => msgs.reduce((at, m) => reduce(at, m)[0], s);
const scanned = (reason: "added" | "asked", found = scan): Msg => ({ type: "plansScanned", project: "APP", reason, scan: found });

describe("plans found in a repository", () => {
  it("are looked for right after a project is added", () => {
    const [, fx] = reduce(loaded(), { type: "projectAdded", project });
    expect(fx).toContainEqual({ type: "loadPlans", project: "APP", reason: "added" });
  });

  it("found right after adding open a list with all ticked but the imported ones", () => {
    const s = run(loaded(), scanned("added"));
    expect(s.plan.dialog).toEqual({ kind: "foundPlans" });
    expect(s.imports.found?.selected).toEqual(["/work/app/plans/x.toml", "/work/app/specs/001-a"]);
    expect(s.imports.found?.specify_dir).toBe(true);
  });

  it("none found right after adding: the page goes straight on", () => {
    const s = run(loaded(), scanned("added", { plans: [], specify_dir: false }));
    expect(s.plan.dialog).toBeUndefined();
    expect(s.imports.found).toBeUndefined();
  });

  it("a scan the person asked for shows in place, even when it finds none", () => {
    const [asked, fx] = reduce(loaded(), { type: "scanPlans", project: "APP" });
    expect(fx).toEqual([{ type: "loadPlans", project: "APP", reason: "asked" }]);
    expect(asked.imports.scanning).toBe("APP");
    const s = run(asked, scanned("asked", { plans: [], specify_dir: false }));
    expect(s.imports.found?.plans).toEqual([]);
    expect(s.imports.scanning).toBeUndefined();
    expect(s.plan.dialog).toBeUndefined();
  });

  it("import the ticked ones in list order, one request each", () => {
    const s = run(loaded(), scanned("asked"), { type: "togglePlan", path: "/work/app/plans/x.toml", on: false },
      { type: "togglePlan", path: "/work/app/specs/002-b", on: true });
    const [busy, fx] = reduce(s, { type: "importSelected" });
    expect(fx).toEqual([{ type: "importPlans", project: "APP", paths: ["/work/app/specs/001-a", "/work/app/specs/002-b"] }]);
    expect(busy.imports.found?.outcomes["/work/app/specs/001-a"]).toEqual({ state: "running" });
    expect(reduce(busy, { type: "importSelected" })[1]).toEqual([]);
    const none = run(loaded(), scanned("asked"), { type: "togglePlan", path: "/work/app/plans/x.toml", on: false },
      { type: "togglePlan", path: "/work/app/specs/001-a", on: false });
    expect(reduce(none, { type: "importSelected" })[1]).toEqual([]);
  });

  it("keep each plan's outcome; an imported one is marked and its chain joins the store", () => {
    const s = run(loaded(), scanned("asked"), { type: "importSelected" },
      { type: "planImported", project: "APP", path: "/work/app/plans/x.toml", result: imported("x") },
      { type: "planImported", project: "APP", path: "/work/app/specs/001-a", message: "specs/001-a/tasks.md:3: no tasks" });
    const f = s.imports.found;
    expect(f?.outcomes["/work/app/plans/x.toml"]).toEqual({ state: "done", chains: ["x"] });
    expect(f?.outcomes["/work/app/specs/001-a"]).toEqual({ state: "failed", message: "specs/001-a/tasks.md:3: no tasks" });
    expect(f?.plans.find((p) => p.relative === "plans/x.toml")?.imported).toBe(true);
    expect(f?.selected).toEqual(["/work/app/specs/001-a"]);
    expect(s.chainOrder).toEqual(["c-x"]);
  });

  it("an answer that imported nothing is a failure with the daemon's reasons", () => {
    const empty: ImportPathResult = { chains: [], skipped: [], failed: [{ folder: "001-a", path: "/p", reason: "bad line 3" }] };
    const s = run(loaded(), scanned("asked"), { type: "importSelected" },
      { type: "planImported", project: "APP", path: "/work/app/specs/001-a", result: empty });
    expect(s.imports.found?.outcomes["/work/app/specs/001-a"]).toEqual({ state: "failed", message: "001-a: bad line 3" });
  });

  it("once all answered, the first imported chain opens and the outcomes stay in sight", () => {
    const s = run(loaded(), scanned("asked"), { type: "importSelected" },
      { type: "planImported", project: "APP", path: "/work/app/plans/x.toml", result: imported("x") },
      { type: "planImported", project: "APP", path: "/work/app/specs/001-a", result: imported("001-a") });
    const [done, fx] = reduce(s, { type: "plansImported", project: "APP" });
    expect(done.route).toEqual({ name: "projectChain", project: "APP", chain: "x" });
    expect(fx).toContainEqual({ type: "pushUrl", path: "/projects/APP/chains/x" });
    // Picked in the empty project's page: the list's own dialog shows the outcomes.
    expect(done.plan.dialog).toEqual({ kind: "foundPlans" });
    expect(done.imports.busy).toBe(false);
    const inDialog = run(loaded(), { type: "openDialog", dialog: { kind: "import", project: "APP" } }, scanned("asked"),
      { type: "importSelected" }, { type: "plansImported", project: "APP" });
    expect(inDialog.plan.dialog).toEqual({ kind: "import", project: "APP" });
  });

  it("skipping drops the list and closes its dialog", () => {
    const s = run(loaded(), scanned("added"), { type: "skipPlans" });
    expect(s.imports.found).toBeUndefined();
    expect(s.plan.dialog).toBeUndefined();
  });
});

describe("the import dialog", () => {
  it("imports a path and opens the first chain it made, keeping the summary", () => {
    const [busy, fx] = reduce(loaded(), { type: "importPath", project: "APP", path: "/work/app/specs" });
    expect(fx).toEqual([{ type: "importPath", project: "APP", path: "/work/app/specs" }]);
    expect(busy.imports.busy).toBe(true);
    const [s, after] = reduce(busy, { type: "importAnswered", project: "APP", result: imported("001-a", "002-b") });
    expect(s.imports.result?.chains).toHaveLength(2);
    expect(s.chainOrder).toEqual(["c-001-a", "c-002-b"]);
    expect(s.route).toEqual({ name: "projectChain", project: "APP", chain: "001-a" });
    expect(after).toContainEqual({ type: "pushUrl", path: "/projects/APP/chains/001-a" });
  });

  it("imports pasted text as one chain", () => {
    const s = run(loaded(), { type: "importText", project: "APP", toml: "[chain]" }, { type: "textImported", project: "APP", chain: chain("c9", "pasted") });
    expect(s.imports.result?.chains.map((c) => c.chain.chain.name)).toEqual(["pasted"]);
    expect(s.route).toEqual({ name: "projectChain", project: "APP", chain: "pasted" });
  });

  it("keeps a refusal for the input it came from, and a new dialog starts clean", () => {
    const s = run(loaded(), { type: "importPath", project: "APP", path: "/nowhere" },
      { type: "importRefused", source: "path", message: "/nowhere does not exist" });
    expect(s.imports.error).toEqual({ source: "path", message: "/nowhere does not exist" });
    expect(s.imports.busy).toBe(false);
    const again = run(s, { type: "openDialog", dialog: { kind: "import", project: "APP" } });
    expect(again.imports.error).toBeUndefined();
  });
});
