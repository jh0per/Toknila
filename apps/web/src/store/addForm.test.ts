import { describe, expect, it } from "vitest";
import type { Project, RepoCheck } from "../api/types";
import type { Msg } from "./msg";
import { reduce } from "./reduce";
import { type State, initialState } from "./state";

const A = "/work/Toknila";
const B = "/work/other-repo";

const check = (key: string | null, name: string | null): RepoCheck =>
  ({ problems: [], branches: ["develop"], suggested_key: key, suggested_name: name });

const path = (value: string): Msg => ({ type: "editDraft", field: "repo_path", value });
const checked = (repo_path: string, key: string | null, name: string | null): Msg =>
  ({ type: "repoChecked", repo_path, check: check(key, name) });

function blank(): State {
  const projects = [{ id: "p1", key: "DEMO", name: "Demo" }] as Project[];
  return reduce(initialState({ name: "chains" }), { type: "loaded", projects, chains: [] })[0];
}

const run = (s: State, ...msgs: Msg[]): State => msgs.reduce((at, m) => reduce(at, m)[0], s);

describe("the add-a-project form", () => {
  it("asks the daemon about a new folder, once per folder and never for an empty one", () => {
    const [s, fx] = reduce(blank(), path(A));
    expect(fx).toEqual([{ type: "checkRepo", repo_path: A, work_branch: "develop" }]);
    expect(reduce(s, path(`${A} `))[1]).toEqual([]);
    expect(reduce(s, path("  "))[1]).toEqual([]);
  });

  it("fills the key and the name from the folder's suggestion", () => {
    const s = run(blank(), path(A), checked(A, "TOKNILA", "Toknila"));
    expect(s.settings.add.draft).toMatchObject({ key: "TOKNILA", name: "Toknila", repo_path: A });
  });

  it("follows a new folder until the person types into the field", () => {
    const followed = run(blank(), path(A), checked(A, "TOKNILA", "Toknila"), path(B), checked(B, "OTHERREPO", "other-repo"));
    expect(followed.settings.add.draft).toMatchObject({ key: "OTHERREPO", name: "other-repo" });
    const mine = run(followed, { type: "editDraft", field: "key", value: "MINE" }, path(A), checked(A, "TOKNILA", "Toknila"));
    expect(mine.settings.add.draft).toMatchObject({ key: "MINE", name: "Toknila" });
    const named = run(mine, { type: "editDraft", field: "name", value: "Mine" }, path(B), checked(B, "OTHERREPO", "other-repo"));
    expect(named.settings.add.draft).toMatchObject({ key: "MINE", name: "Mine" });
  });

  it("drops an answer about a folder the form no longer shows", () => {
    const s = run(blank(), path(A), path(B), checked(A, "TOKNILA", "Toknila"));
    expect(s.settings.add.draft).toMatchObject({ key: "", name: "", repo_path: B });
    expect(s.settings.add.suggested).toBeUndefined();
  });

  it("an emptied folder clears what followed it, not what the person typed", () => {
    const s = run(blank(), path(A), checked(A, "TOKNILA", "Toknila"), { type: "editDraft", field: "name", value: "Mine" }, path(""));
    expect(s.settings.add.draft).toMatchObject({ key: "", name: "Mine", repo_path: "" });
  });

  it("sends the suggestion for a key the person left empty", () => {
    const emptied = run(blank(), path(A), checked(A, "TOKNILA", "Toknila"), { type: "editDraft", field: "key", value: "" });
    expect(emptied.settings.add.draft.key).toBe("");
    const [, fx] = reduce(emptied, { type: "submitProject" });
    expect(fx).toEqual([{ type: "addProject", draft: { key: "TOKNILA", name: "Toknila", repo_path: A, work_branch: "develop" } }]);
  });

  it("sent before the suggestion came, waits for the folder check and then goes out with it", () => {
    const [waiting, fx] = reduce(run(blank(), path(` ${A} `)), { type: "submitProject" });
    expect(fx).toEqual([{ type: "checkRepo", repo_path: A, work_branch: "develop" }]);
    expect(waiting.settings.busy).toBe(true);
    const [sent, out] = reduce(waiting, checked(A, "TOKNILA", "Toknila"));
    expect(out).toEqual([{ type: "addProject", draft: { key: "TOKNILA", name: "Toknila", repo_path: A, work_branch: "develop" } }]);
    expect(sent.settings.add.waiting).toBe(false);
    // A later answer for the same folder does not send it twice.
    expect(reduce(sent, checked(A, "TOKNILA", "Toknila"))[1]).toEqual([]);
  });

  it("a folder that suggests no key still goes out, and the daemon judges the key", () => {
    const s = run(blank(), path("/work/1"), checked("/work/1", null, "1"));
    const [, fx] = reduce(s, { type: "submitProject" });
    expect(fx).toEqual([{ type: "addProject", draft: { key: "", name: "1", repo_path: "/work/1", work_branch: "develop" } }]);
  });

  it("a refusal ends the wait, so the next answer does not send the form by itself", () => {
    const [waiting] = reduce(run(blank(), path(A)), { type: "submitProject" });
    const refused = run(waiting, { type: "settingsRejected", field: "repo_path", message: "gone" });
    expect(reduce(refused, checked(A, "TOKNILA", "Toknila"))[1]).toEqual([]);
  });
});
