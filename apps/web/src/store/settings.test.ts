import { describe, expect, it } from "vitest";
import type { Project, RepoProblem } from "../api/types";
import { reduce } from "./reduce";
import { type State, currentProject, initialState } from "./state";
import { fieldError, formError } from "./settings";

const project = (over: Partial<Project> = {}) =>
  ({ id: "p1", key: "DEMO", name: "Demo", max_parallel: 2, ...over }) as Project;

const draft = { key: "DEMO", name: "Demo", repo_path: "/work/demo", work_branch: "develop", check_command: "", merge_mode: "local" as const };

function ready(projects: Project[]): State {
  const [s] = reduce(initialState({ name: "chains" }), { type: "loaded", projects, chains: [] });
  return s;
}

/// The form with every field typed in by the person.
function typed(s: State): State {
  const fields = ["repo_path", "key", "name", "work_branch", "check_command", "merge_mode"] as const;
  return fields.reduce((at, field) => reduce(at, { type: "editDraft", field, value: draft[field] })[0], s);
}

describe("first run", () => {
  it("loads the checks and the profiles when there is no project", () => {
    const [, fx] = reduce(initialState({ name: "chains" }), { type: "loaded", projects: [], chains: [] });
    expect(fx).toEqual([{ type: "loadChecks" }, { type: "loadProfiles" }, { type: "loadPlaces" }]);
  });

  it("loads nothing extra at home once there are projects", () => {
    const [, fx] = reduce(initialState({ name: "chains" }), { type: "loaded", projects: [project()], chains: [] });
    expect(fx).toEqual([]);
  });

  it("loads them when the settings page opens", () => {
    const [s, fx] = reduce(ready([project()]), { type: "navigate", route: { name: "settings", project: "DEMO" } });
    expect(fx).toContainEqual({ type: "loadChecks" });
    expect(fx).toContainEqual({ type: "loadProfiles" });
    expect(currentProject(s)?.key).toBe("DEMO");
  });

  it("settings without a project in the address is about the first project", () => {
    const s = ready([project(), project({ id: "p2", key: "OTHER" })]);
    const [at] = reduce(s, { type: "navigate", route: { name: "settings" } });
    expect(currentProject(at)?.key).toBe("DEMO");
  });
});

describe("adding a project", () => {
  it("asks the daemon and marks the form busy", () => {
    const [s, fx] = reduce(typed(ready([])), { type: "submitProject" });
    expect(s.settings.busy).toBe(true);
    expect(fx).toEqual([{ type: "addProject", draft }]);
  });

  it("keeps each problem for its own field, and the branches to pick", () => {
    const problems = [{ field: "work_branch", code: "branch_missing", message: "no branch" }] as RepoProblem[];
    const [busy] = reduce(typed(ready([])), { type: "submitProject" });
    const [s] = reduce(busy, { type: "addRejected", problems, branches: ["main", "trunk"] });
    expect(fieldError(s.settings, "work_branch")).toBe("no branch");
    expect(fieldError(s.settings, "repo_path")).toBeUndefined();
    expect(s.settings.branches).toEqual(["main", "trunk"]);
    expect(s.settings.busy).toBe(false);
  });

  it("a new attempt clears the old problems", () => {
    const problems = [{ field: "repo_path", code: "path_missing", message: "gone" }] as RepoProblem[];
    const [rejected] = reduce(typed(ready([])), { type: "addRejected", problems, branches: [] });
    const [again] = reduce(rejected, { type: "submitProject" });
    expect(again.settings.problems).toEqual([]);
  });

  it("an added project empties the form and closes its dialog", () => {
    const [open] = reduce(typed(ready([project({ id: "p0", key: "OLD" })])), {
      type: "openDialog", dialog: { kind: "addProject" },
    });
    const [s] = reduce(open, { type: "projectAdded", project: project() });
    expect(s.settings.add.draft).toEqual({ key: "", name: "", repo_path: "", work_branch: "", check_command: "", merge_mode: "local" });
    expect(s.plan.dialog).toBeUndefined();
  });

  it("an added project joins the list, the page moves to its settings and the agents reload", () => {
    const [s, fx] = reduce(ready([]), { type: "projectAdded", project: project() });
    expect(s.projects.map((p) => p.key)).toEqual(["DEMO"]);
    expect(s.route).toEqual({ name: "settings", project: "DEMO" });
    expect(fx).toContainEqual({ type: "pushUrl", path: "/projects/DEMO/settings" });
    expect(fx).toContainEqual({ type: "loadAgents" });
  });
});

describe("saving settings", () => {
  it("replaces the project with the saved one and says so", () => {
    const s0 = ready([project()]);
    const [s, fx] = reduce(s0, { type: "projectSaved", project: project({ max_parallel: 4 }) });
    expect(s.projects[0].max_parallel).toBe(4);
    expect(s.settings.saved).toBe(true);
    expect(fx).toContainEqual({ type: "loadAgents" });
  });

  it("a refusal names its field, and one without a field is the form's", () => {
    const [named] = reduce(ready([project()]), { type: "settingsRejected", field: "work_branch", message: "no such branch" });
    expect(fieldError(named.settings, "work_branch")).toBe("no such branch");
    expect(formError(named.settings)).toBeUndefined();
    const [plain] = reduce(ready([project()]), { type: "settingsRejected", field: undefined, message: "limit is 1 to 64" });
    expect(formError(plain.settings)).toBe("limit is 1 to 64");
  });

  it("loads the tok command status with the page and installs it on request", () => {
    const [, fx] = reduce(ready([project()]), { type: "navigate", route: { name: "settings", project: "DEMO" } });
    expect(fx).toContainEqual({ type: "loadCli" });
    const [asked, run] = reduce(ready([project()]), { type: "installCli" });
    expect(run).toEqual([{ type: "installCli" }]);
    const cli = { version: "0.2.0", bundled: "/app/tok", link: "/h/.local/bin/tok", installed: true, on_path: false };
    const [done] = reduce(asked, { type: "cliLoaded", cli });
    expect(done.settings.cli).toEqual(cli);
    const [failed] = reduce(done, { type: "cliRejected", message: "exists and is not a link" });
    expect(failed.settings.cliError).toBe("exists and is not a link");
  });
});
