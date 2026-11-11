import { describe, expect, it } from "vitest";
import type { ChainDetail, Project, TicketDetail } from "../api/types";
import type { Msg } from "./msg";
import { planError } from "./plan";
import { reduce } from "./reduce";
import { type State, activeProjects, currentProject, initialState } from "./state";

const projects = [
  { id: "p1", key: "DEMO", name: "Demo" },
  { id: "p2", key: "OTHER", name: "Other" },
] as Project[];

function chain(id: string, name: string, keys: string[], project = "p1"): ChainDetail {
  return {
    chain: { id, name, goal: "", autostart: false, project_id: project },
    tickets: keys.map((k, i) => ({ id: `t${i + 1}`, key: k, chain_id: id, title: k, status: "backlog", prompt: "" })),
    releases: [], edges: [], release_tickets: [], order: [],
    summary: { status: "not_started", total: keys.length, done: 0, released: 0 },
  } as unknown as ChainDetail;
}

const detail = (key: string, chainId = "c1") =>
  ({ ticket: { id: "t9", key, chain_id: chainId, title: key, prompt: "" }, chain: "demo", can_edit: true, can_move: true }) as unknown as TicketDetail;

function loaded(route = { name: "project", project: "DEMO" } as State["route"]): State {
  return reduce(initialState(route), { type: "loaded", projects, chains: [chain("c1", "demo", ["DEMO-1"])] })[0];
}

const run = (s: State, ...msgs: Msg[]): State => msgs.reduce((at, m) => reduce(at, m)[0], s);

describe("dialogs", () => {
  it("open and close in the store", () => {
    const [s, fx] = reduce(loaded(), { type: "openDialog", dialog: { kind: "newChain", project: "DEMO" } });
    expect(s.plan.dialog).toEqual({ kind: "newChain", project: "DEMO" });
    // The version the new chain suggests is asked for; nothing else is.
    expect(fx).toEqual([{ type: "loadNextVersion", project: "DEMO" }]);
    expect(reduce(s, { type: "closeDialog" })[0].plan.dialog).toBeUndefined();
  });

  it("the desktop menu's hash opens the add-a-project form and is then cleared", () => {
    const [s, fx] = reduce(loaded(), { type: "hashChanged", hash: "#/add-project" });
    expect(s.plan.dialog).toEqual({ kind: "addProject" });
    expect(fx).toEqual([{ type: "clearHash" }]);
    expect(reduce(loaded(), { type: "hashChanged", hash: "#/elsewhere" })[1]).toEqual([]);
  });

  it("a form opened again starts without the last refusal", () => {
    const refused = run(loaded(), { type: "planRejected", form: "chain", field: undefined, message: "chain name is empty" });
    expect(planError(refused.plan, "chain")).toBe("chain name is empty");
    const again = run(refused, { type: "openDialog", dialog: { kind: "newChain", project: "DEMO" } });
    expect(planError(again.plan, "chain")).toBeUndefined();
  });
});

describe("chains and cards from the window", () => {
  it("a new chain is sent, joins the store and opens", () => {
    const input = { name: "plan", goal: null, autostart: false, review_mode: null };
    const [busy, fx] = reduce(loaded(), { type: "createChain", project: "DEMO", input });
    expect(busy.plan.busy).toBe(true);
    expect(fx).toEqual([{ type: "createChain", project: "DEMO", input }]);
    const [s, after] = reduce(busy, { type: "chainCreated", project: "DEMO", chain: chain("c2", "plan", []) });
    expect(s.chainOrder).toEqual(["c1", "c2"]);
    expect(s.plan).toMatchObject({ busy: false, dialog: undefined });
    expect(s.route).toEqual({ name: "projectChain", project: "DEMO", chain: "plan" });
    expect(after).toContainEqual({ type: "pushUrl", path: "/projects/DEMO/chains/plan" });
  });

  it("a refusal stays with its form and field", () => {
    const s = run(loaded(), { type: "planRejected", form: "card", field: "title", message: "ticket title is empty" });
    expect(planError(s.plan, "card", "title")).toBe("ticket title is empty");
    expect(planError(s.plan, "card")).toBeUndefined();
    expect(planError(s.plan, "edit", "title")).toBeUndefined();
    expect(s.plan.busy).toBe(false);
  });

  it("a new card opens, and its chain reloads to show it", () => {
    const [s, fx] = reduce(loaded(), { type: "cardCreated", detail: detail("DEMO-2") });
    expect(s.details["DEMO-2"]).toBeDefined();
    expect(s.route).toEqual({ name: "card", project: "DEMO", chain: "demo", key: "DEMO-2" });
    expect(fx).toContainEqual({ type: "loadChain", id: "c1" });
    expect(fx).toContainEqual({ type: "pushUrl", path: "/projects/DEMO/chains/demo/cards/DEMO-2" });
  });

  it("an edit opens for one card, and a save closes it and refreshes the chain", () => {
    const editing = run(loaded(), { type: "editCard", key: "DEMO-1" });
    expect(editing.plan.editing).toBe("DEMO-1");
    expect(run(editing, { type: "pickPlace", key: "DEMO-1" }).plan).toMatchObject({ editing: undefined, moving: "DEMO-1" });
    const [saved, fx] = reduce(editing, { type: "cardSaved", detail: detail("DEMO-1") });
    expect(saved.plan.editing).toBeUndefined();
    expect(fx).toEqual([{ type: "loadChain", id: "c1" }]);
    const away = run(editing, { type: "selectCard", project: "DEMO", chain: "demo", key: "DEMO-2" });
    expect(away.plan.editing).toBeUndefined();
  });

  it("a move answers with the chain in its new order", () => {
    const moving = run(loaded(), { type: "pickPlace", key: "DEMO-1" });
    const intent = { type: "moveCard", chain: "c1", key: "DEMO-1", anchor: "DEMO-2", place: "before" } as const;
    expect(reduce(moving, intent)[1]).toEqual([intent]);
    const moved = chain("c1", "demo", ["DEMO-2", "DEMO-1"]);
    const [s, fx] = reduce(moving, { type: "cardMoved", key: "DEMO-1", chain: moved });
    expect(s.chains.c1).toBe(moved);
    expect(s.plan.moving).toBeUndefined();
    expect(fx).toEqual([{ type: "loadDetail", key: "DEMO-1" }]);
  });
});

describe("archived projects", () => {
  const archived = { ...projects[0], archived: true } as Project;

  it("leave the sidebar, and the page leaves them when they were in focus", () => {
    const [s, fx] = reduce(loaded(), { type: "projectArchived", project: archived });
    expect(activeProjects(s).map((p) => p.key)).toEqual(["OTHER"]);
    expect(s.route).toEqual({ name: "chains" });
    expect(currentProject(s)?.key).toBe("OTHER");
    expect(fx).toContainEqual({ type: "pushUrl", path: "/" });
  });

  it("archiving another project keeps the page where it is", () => {
    const [s] = reduce(loaded(), { type: "projectArchived", project: { ...projects[1], archived: true } as Project });
    expect(s.route).toEqual({ name: "project", project: "DEMO" });
  });

  it("a refusal (agents still working) is kept for the archive form", () => {
    const message = "DEMO has 1 agent(s) working; stop them before archiving the project";
    const [busy, fx] = reduce(loaded(), { type: "archiveProject", project: "DEMO", archived: true });
    expect(fx).toEqual([{ type: "archiveProject", project: "DEMO", archived: true }]);
    const s = run(busy, { type: "planRejected", form: "archive", field: undefined, message });
    expect(planError(s.plan, "archive")).toBe(message);
    expect(activeProjects(s)).toHaveLength(2);
  });

  it("are listed on request, and come back", () => {
    const gone = run(loaded(), { type: "projectArchived", project: archived });
    expect(run(gone, { type: "showArchived", on: true }).plan.showArchived).toBe(true);
    const back = run(gone, { type: "projectArchived", project: { ...archived, archived: false } as Project });
    expect(activeProjects(back).map((p) => p.key)).toEqual(["DEMO", "OTHER"]);
  });

  it("an archived home counts as a first run for the checks", () => {
    const [, fx] = reduce(initialState({ name: "chains" }), { type: "loaded", projects: [archived], chains: [] });
    expect(fx).toContainEqual({ type: "loadChecks" });
  });
});
