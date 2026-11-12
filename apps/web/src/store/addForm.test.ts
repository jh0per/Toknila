import { describe, expect, it } from "vitest";
import type { Project, RepoCheck } from "../api/types";
import type { Msg } from "./msg";
import { reduce } from "./reduce";
import { addBusy } from "./settings";
import { type State, initialState } from "./state";

const A = "/work/Toknila";
const B = "/work/other-repo";

const check = (
  key: string | null, name: string | null, branch: string | null = "develop", command: string | null = null,
  merge: RepoCheck["suggested_merge_mode"] = "local", setup: string | null = null,
): RepoCheck => ({
  problems: [], branches: ["develop"], suggested_key: key, suggested_name: name, suggested_branch: branch,
  suggested_check_command: command, suggested_setup_command: setup, suggested_merge_mode: merge,
});

const path = (value: string): Msg => ({ type: "editDraft", field: "repo_path", value });
const checked = (repo_path: string, key: string | null, name: string | null, branch: string | null = "develop"): Msg =>
  ({ type: "repoChecked", repo_path, check: check(key, name, branch) });

function blank(): State {
  const projects = [{ id: "p1", key: "DEMO", name: "Demo" }] as Project[];
  return reduce(initialState({ name: "chains" }), { type: "loaded", projects, chains: [] })[0];
}

const run = (s: State, ...msgs: Msg[]): State => msgs.reduce((at, m) => reduce(at, m)[0], s);

describe("the add-a-project form", () => {
  it("asks the daemon about a new folder, once per folder and never for an empty one", () => {
    const [s, fx] = reduce(blank(), path(A));
    expect(fx).toEqual([{ type: "checkRepo", repo_path: A, work_branch: "" }]);
    expect(reduce(s, path(`${A} `))[1]).toEqual([]);
    expect(reduce(s, path("  "))[1]).toEqual([]);
  });

  it("fills the key and the name from the folder's suggestion", () => {
    const s = run(blank(), path(A), checked(A, "TOKNILA", "Toknila"));
    expect(s.settings.add.draft).toMatchObject({ key: "TOKNILA", name: "Toknila", repo_path: A });
  });

  it("prefills the base branch with the repository's own and keeps one the person typed", () => {
    const s = run(blank(), path(A), checked(A, "TOKNILA", "Toknila", "main"));
    expect(s.settings.add.draft.work_branch).toBe("main");
    const followed = run(s, path(B), checked(B, "OTHERREPO", "other-repo", "trunk"));
    expect(followed.settings.add.draft.work_branch).toBe("trunk");
    const mine = run(followed, { type: "editDraft", field: "work_branch", value: "release" }, path(A), checked(A, "TOKNILA", "Toknila", "main"));
    expect(mine.settings.add.draft.work_branch).toBe("release");
    // A repository that suggests none leaves the field empty for the daemon to judge.
    expect(run(blank(), path(A), checked(A, "TOKNILA", "Toknila", null)).settings.add.draft.work_branch).toBe("");
  });

  it("suggests the setup command of a repository with an npm lock file, as a value, and keeps one the person typed", () => {
    const npm = "npm ci --prefix apps/web";
    const web = (to: string, setup: string | null): Msg =>
      ({ type: "repoChecked", repo_path: to, check: check("WEB", "web", "main", null, "local", setup) });
    const s = run(blank(), path(A), web(A, npm));
    expect(s.settings.add.draft.setup_command).toBe(npm);
    expect(run(s, path(B), web(B, null)).settings.add.draft.setup_command).toBe("");
    const mine = run(s, { type: "editDraft", field: "setup_command", value: "make deps" }, path(B), web(B, npm));
    expect(mine.settings.add.draft.setup_command).toBe("make deps");
  });

  it("prefills the check command and the merge mode from the repository and keeps what the person changed", () => {
    const hub = (to: string, command: string | null, merge: RepoCheck["suggested_merge_mode"]): Msg =>
      ({ type: "repoChecked", repo_path: to, check: check("HUB", "hub", "main", command, merge) });
    const s = run(blank(), path(A), hub(A, "cargo xtask verify", "github"));
    expect(s.settings.add.draft).toMatchObject({ check_command: "cargo xtask verify", merge_mode: "github" });
    // Another folder replaces what was prefilled, including with nothing.
    const other = run(s, path(B), hub(B, null, "local"));
    expect(other.settings.add.draft).toMatchObject({ check_command: "", merge_mode: "local" });
    // What the person typed stays, whatever the folder says.
    const mine = run(
      s, { type: "editDraft", field: "check_command", value: "make check" },
      { type: "editDraft", field: "merge_mode", value: "local" }, path(B), hub(B, "npm test", "github"),
    );
    expect(mine.settings.add.draft).toMatchObject({ check_command: "make check", setup_command: "", merge_mode: "local" });
    // And they are what goes out.
    const [, fx] = reduce(mine, { type: "submitProject" });
    expect(fx).toContainEqual({
      type: "addProject",
      draft: { key: "HUB", name: "hub", repo_path: B, work_branch: "main", check_command: "make check", setup_command: "", merge_mode: "local" },
    });
  });

  it("an emptied folder clears the prefilled check command", () => {
    const s = run(blank(), path(A), { type: "repoChecked", repo_path: A, check: check("T", "t", "main", "cargo test", "local") }, path(""));
    expect(s.settings.add.draft.check_command).toBe("");
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
    expect(fx).toEqual([{ type: "addProject", draft: { key: "TOKNILA", name: "Toknila", repo_path: A, work_branch: "develop", check_command: "", setup_command: "", merge_mode: "local" } }]);
  });

  it("sent before the suggestion came, waits for the folder check and then goes out with it", () => {
    const [waiting, fx] = reduce(run(blank(), path(` ${A} `)), { type: "submitProject" });
    expect(fx).toEqual([{ type: "checkRepo", repo_path: A, work_branch: "" }]);
    expect(waiting.settings.busy).toBe(true);
    const [sent, out] = reduce(waiting, checked(A, "TOKNILA", "Toknila"));
    expect(out).toEqual([{ type: "addProject", draft: { key: "TOKNILA", name: "Toknila", repo_path: A, work_branch: "develop", check_command: "", setup_command: "", merge_mode: "local" } }]);
    expect(sent.settings.add.waiting).toBe(false);
    // A later answer for the same folder does not send it twice.
    expect(reduce(sent, checked(A, "TOKNILA", "Toknila"))[1]).toEqual([]);
  });

  it("a folder that suggests no key still goes out, and the daemon judges the key", () => {
    const s = run(blank(), path("/work/1"), checked("/work/1", null, "1"));
    const [, fx] = reduce(s, { type: "submitProject" });
    expect(fx).toEqual([{ type: "addProject", draft: { key: "", name: "1", repo_path: "/work/1", work_branch: "develop", check_command: "", setup_command: "", merge_mode: "local" } }]);
  });

  it("a refusal ends the wait, so the next answer does not send the form by itself", () => {
    const [waiting] = reduce(run(blank(), path(A)), { type: "submitProject" });
    const refused = run(waiting, { type: "settingsRejected", field: "repo_path", message: "gone" });
    expect(reduce(refused, checked(A, "TOKNILA", "Toknila"))[1]).toEqual([]);
  });
});

const missing = (p: string): RepoCheck => ({
  ...check("X", "x", null), branches: [],
  problems: [{ field: "repo_path", code: "path_missing", message: `${p} does not exist.`, fix: "create_folder" }],
});
const notRepo = (p: string): RepoCheck => ({
  ...missing(p),
  problems: [{ field: "repo_path", code: "not_a_git_repo", message: `${p} is not a git repository.`, fix: "init_repository" }],
});

describe("the folder's problems and their fixes", () => {
  it("shows the problem as soon as the check answers, without Add project", () => {
    const s = run(blank(), path(A), { type: "repoChecked", repo_path: A, check: missing(A) });
    expect(s.settings.problems).toEqual(missing(A).problems);
    expect(s.settings.busy).toBe(false);
  });

  it("drops an answer about an older text: no problem, no branch, no suggestion", () => {
    const s = run(blank(), path(A), path(B), { type: "repoChecked", repo_path: A, check: missing(A) });
    expect(s.settings.problems).toEqual([]);
    expect(s.settings.add.suggested).toBeUndefined();
    const ok = run(blank(), path(A), path(B), checked(A, "TOKNILA", "Toknila", "develop"));
    expect(ok.settings.add.draft.work_branch).toBe("");
  });

  it("asks at once after Browse or a paste, and sends only a branch the person typed", () => {
    const [s, fx] = reduce(blank(), { type: "editDraft", field: "repo_path", value: A, now: true });
    expect(fx).toEqual([{ type: "checkRepo", repo_path: A, work_branch: "", now: true }]);
    const typed = run(s, checked(A, "TOKNILA", "Toknila", "develop"), { type: "editDraft", field: "work_branch", value: "next" });
    expect(reduce(typed, path(`${A}2`))[1]).toEqual([{ type: "checkRepo", repo_path: `${A}2`, work_branch: "next" }]);
  });

  it("fills the main branch from the check unless the person typed their own", () => {
    const s = run(blank(), path(A), checked(A, "TOKNILA", "Toknila", "develop"));
    expect(s.settings.add.draft.work_branch).toBe("develop");
    const own = run(blank(), path(A), { type: "editDraft", field: "work_branch", value: "next" }, checked(A, "TOKNILA", "Toknila", "develop"));
    expect(own.settings.add.draft.work_branch).toBe("next");
  });

  it("never sends an empty main branch: the suggestion stands in for it", () => {
    const s = run(blank(), path(A), checked(A, "TOKNILA", "Toknila", "develop"), { type: "editDraft", field: "work_branch", value: "  " });
    const [, fx] = reduce(s, { type: "submitProject" });
    expect(fx).toMatchObject([{ type: "addProject", draft: { work_branch: "develop" } }]);
  });

  it("a fix requested calls prepare and holds the form until it answers", () => {
    const s = run(blank(), path(A), { type: "repoChecked", repo_path: A, check: missing(A) });
    const [fixing, fx] = reduce(s, { type: "fixFolder", fix: "create_folder" });
    expect(fx).toEqual([{ type: "prepareRepo", repo_path: A, fix: "create_folder" }]);
    expect(fixing.settings.add.fixing).toEqual({ repo_path: A, fix: "create_folder" });
    expect(addBusy(fixing.settings)).toBe(true);
    // A second press while it works does nothing.
    expect(reduce(fixing, { type: "fixFolder", fix: "create_folder" })[1]).toEqual([]);
  });

  it("a fix answered with the next problem shows it with its own button", () => {
    const s = run(blank(), path(A), { type: "repoChecked", repo_path: A, check: missing(A) }, { type: "fixFolder", fix: "create_folder" });
    const next = run(s, { type: "repoPrepared", repo_path: A, check: notRepo(A) });
    expect(next.settings.problems[0]).toMatchObject({ code: "not_a_git_repo", fix: "init_repository" });
    expect(next.settings.add.fixing).toBeUndefined();
    expect(addBusy(next.settings)).toBe(false);
    const done = run(next, { type: "fixFolder", fix: "init_repository" }, { type: "repoPrepared", repo_path: A, check: check("TOKNILA", "Toknila", "main") });
    expect(done.settings.problems).toEqual([]);
    expect(done.settings.add.draft.work_branch).toBe("main");
  });

  it("a fix refused keeps the problem and shows the daemon's sentence", () => {
    const s = run(blank(), path(A), { type: "repoChecked", repo_path: A, check: notRepo(A) }, { type: "fixFolder", fix: "init_repository" });
    const refused = run(s, { type: "fixRefused", repo_path: A, message: "git has no user name." });
    expect(refused.settings.add.fixError).toEqual({ repo_path: A, message: "git has no user name." });
    expect(refused.settings.add.fixing).toBeUndefined();
    expect(refused.settings.problems[0]).toMatchObject({ fix: "init_repository" });
    // Pressing again clears the old sentence while it works.
    expect(run(refused, { type: "fixFolder", fix: "init_repository" }).settings.add.fixError).toBeUndefined();
  });

  it("an answer or a refusal for an older text leaves nothing on the new one", () => {
    const s = run(blank(), path(A), { type: "repoChecked", repo_path: A, check: missing(A) }, { type: "fixFolder", fix: "create_folder" }, path(B));
    const answered = run(s, { type: "repoPrepared", repo_path: A, check: notRepo(A) });
    expect(answered.settings.problems).toEqual([]);
    expect(answered.settings.add.fixing).toBeUndefined();
    const refused = run(s, { type: "fixRefused", repo_path: A, message: "no" });
    expect(refused.settings.add.fixError).toBeUndefined();
  });
});
