import { describe, expect, it } from "vitest";
import type { ChainDeletion, ChainDetail, EventRecord, Project, ProjectDeletion } from "../api/types";
import type { Msg } from "./msg";
import { reduce } from "./reduce";
import { folderText, keyConfirmed } from "./removal";
import { type State, initialState } from "./state";

const projects = [
  { id: "p1", key: "APP", name: "App", repo_path: "/work/app" },
  { id: "p2", key: "OTHER", name: "Other" },
] as Project[];

function chain(id: string, name: string, project: string, ticket: string): ChainDetail {
  return {
    chain: { id, name, goal: "", autostart: false, project_id: project },
    tickets: [{ id: `${id}-t`, key: ticket, chain_id: id, title: "t", status: "backlog" }],
    releases: [], edges: [], release_tickets: [], order: [],
    summary: { status: "not_started", total: 1, done: 0, released: 0 },
  } as unknown as ChainDetail;
}

const event = (seq: number, entity: string, kind: string, entity_id: string, payload: unknown): EventRecord =>
  ({ seq, at: 0, actor: "human", entity, entity_id, kind, payload }) as EventRecord;

function loaded(route: State["route"]): State {
  const [s] = reduce(initialState(route), {
    type: "loaded",
    projects,
    chains: [chain("c1", "plan", "p1", "APP-1"), chain("c2", "other", "p2", "OTHER-1")],
  });
  const events = [event(1, "ticket", "created", "c1-t", { ticket: { id: "c1-t", chain_id: "c1" } })];
  return { ...s, events, lastSeq: 1 };
}

const deletion = (over: Partial<ProjectDeletion> = {}): ProjectDeletion => ({
  project: projects[0], chains: ["plan"], tickets: 1, worktrees_removed: [], folder: "kept", warnings: [], ...over,
});

const run = (s: State, ...msgs: Msg[]): State => msgs.reduce((at, m) => reduce(at, m)[0], s);

describe("deleting a project", () => {
  it("moves the folder to the Trash only once the key is typed", () => {
    expect(keyConfirmed(projects[0], "")).toBe(false);
    expect(keyConfirmed(projects[0], "app")).toBe(false);
    expect(keyConfirmed(projects[0], " APP ")).toBe(true);
  });

  it("says what became of the folder, plainly when the Trash did not take it", () => {
    expect(folderText(deletion({ folder: "trashed" }), true)).toBe("The folder /work/app is in the Trash.");
    expect(folderText(deletion(), false)).toBe("The folder /work/app stays where it is, with the branches Toknila made for its cards.");
    expect(folderText(deletion({ folder_error: "the Trash is full" }), true))
      .toBe("The folder stayed where it was: the Trash is full");
    expect(folderText(deletion(), true)).toBe("The folder stayed where it was: the Trash did not take it");
    expect(folderText(deletion({ folder: "none" }), true)).toBe("There was no folder to move to the Trash.");
  });

  it("asks the daemon, and a refusal while agents work says so", () => {
    const s0 = loaded({ name: "settings", project: "APP" });
    const [busy, fx] = reduce(s0, { type: "deleteProject", project: "APP", trash_folder: true });
    expect(fx).toEqual([{ type: "deleteProject", project: "APP", trash_folder: true }]);
    expect(busy.removal).toMatchObject({ busy: true, trashAsked: true });
    const message = "APP has 1 agent(s) working; stop them before deleting the project";
    const s = run(busy, { type: "deleteRefused", message, agentsWorking: true });
    expect(s.removal.error).toEqual({ message, agentsWorking: true });
    expect(s.projects).toHaveLength(2);
  });

  it("drops the project and its chains, keeps the answer and goes home", () => {
    const s0 = run(loaded({ name: "settings", project: "APP" }), { type: "openDialog", dialog: { kind: "deleteProject", project: "APP" } });
    const [s, fx] = reduce(s0, { type: "projectDeleted", deletion: deletion() });
    expect(s.projects.map((p) => p.key)).toEqual(["OTHER"]);
    expect(s.chainOrder).toEqual(["c2"]);
    expect(s.events).toEqual([]);
    expect(s.removal.result?.project.key).toBe("APP");
    expect(s.plan.dialog).toEqual({ kind: "deleteProject", project: "APP" });
    expect(s.route).toEqual({ name: "chains" });
    expect(fx).toContainEqual({ type: "pushUrl", path: "/" });
    expect(run(s, { type: "closeDialog" }).removal.result).toBeUndefined();
  });
});

describe("deleting a chain", () => {
  const chainDeletion = (): ChainDeletion => ({
    chain: { id: "c1", name: "plan", goal: "", autostart: false, project_id: "p1" } as ChainDeletion["chain"],
    project: "APP", tickets: 1, worktrees_removed: [], warnings: [],
  });

  it("asks the daemon and shows a refusal that names the cards", () => {
    const s0 = loaded({ name: "projectChain", project: "APP", chain: "plan" });
    const [busy, fx] = reduce(s0, { type: "deleteChain", chain: "c1" });
    expect(fx).toEqual([{ type: "deleteChain", chain: "c1" }]);
    expect(busy.removal.busy).toBe(true);
    const message = "chain plan has agents working on APP-1; stop them before deleting the chain";
    const s = run(busy, { type: "deleteRefused", message, agentsWorking: true });
    expect(s.removal.error).toEqual({ message, agentsWorking: true });
    expect(s.chainOrder).toEqual(["c1", "c2"]);
  });

  it("drops only that chain, keeps the answer for the dialog and sends the page to its project", () => {
    const s0 = run(
      loaded({ name: "card", project: "APP", chain: "plan", key: "APP-1" }),
      { type: "openDialog", dialog: { kind: "deleteChain", chain: "c1" } },
    );
    const [s, fx] = reduce(s0, { type: "chainDeleted", deletion: chainDeletion() });
    expect(s.chainOrder).toEqual(["c2"]);
    expect(s.projects).toHaveLength(2);
    expect(s.removal.chainResult?.chain.name).toBe("plan");
    expect(s.plan.dialog).toEqual({ kind: "deleteChain", chain: "c1" });
    expect(s.route).toEqual({ name: "project", project: "APP" });
    expect(fx).toContainEqual({ type: "loadInbox" });
    expect(run(s, { type: "closeDialog" }).removal.chainResult).toBeUndefined();
  });

  it("leaves a page about another chain where it is", () => {
    const at = loaded({ name: "projectChain", project: "OTHER", chain: "other" });
    const s = run(at, { type: "chainDeleted", deletion: chainDeletion() });
    expect(s.route).toEqual({ name: "projectChain", project: "OTHER", chain: "other" });
  });
});

describe("journal entries of a deletion", () => {
  const deleted = (seq: number, entity: "chain" | "project", id: string) =>
    ({ type: "event", event: event(seq, entity, "deleted", id, { [entity]: { id } }) }) as Msg;

  it("drop the chain and the project, and a page that showed them goes home", () => {
    const at = loaded({ name: "card", project: "APP", chain: "plan", key: "APP-1" });
    const [noChain, fx] = reduce(at, deleted(2, "chain", "c1"));
    expect(noChain.chainOrder).toEqual(["c2"]);
    expect(noChain.route).toEqual({ name: "chains" });
    expect(fx).toContainEqual({ type: "loadInbox" });
    expect(fx).not.toContainEqual({ type: "loadChain", id: "c1" });
    const [noProject] = reduce(noChain, deleted(3, "project", "p1"));
    expect(noProject.projects.map((p) => p.key)).toEqual(["OTHER"]);
  });

  it("leave a page about something else where it is", () => {
    const at = loaded({ name: "projectChain", project: "OTHER", chain: "other" });
    const s = run(at, deleted(2, "chain", "c1"), deleted(3, "project", "p1"));
    expect(s.route).toEqual({ name: "projectChain", project: "OTHER", chain: "other" });
    expect(s.projects.map((p) => p.key)).toEqual(["OTHER"]);
  });
});
