// Browser test of plans in the window and of deleting a project, against a real daemon (P1-29).
// Usage: node e2e/plans.mjs <path to tok binary>   (cargo xtask e2e-web runs it)
//
// A repository holds two spec folders and a chain file. Adding it as a project offers the three
// plans; one is unticked and the other two are imported, and their chains appear in the sidebar.
// A new scan shows those two as already imported. The import dialog imports a spec folder and a
// chain file by path, and refuses a bad path inline. Deleting the project is refused while a
// (simulated) agent works; once it stops, the folder goes to the Trash after the key is typed.
// The Trash here is a temporary folder (TOKNILA_TRASH_DIR), never the system's. A second project
// is then removed from Toknila only, and its folder stays.

import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { checks, cleanup, launch } from "./helpers.mjs";

const tokBin = process.argv[2];
if (!tokBin) {
  console.error("usage: node e2e/plans.mjs <tok binary>");
  process.exit(2);
}
const home = mkdtempSync(join(tmpdir(), "toknila-plans-"));
// Repositories live outside the data home: the daemon never trashes a folder inside it.
const work = mkdtempSync(join(tmpdir(), "toknila-plans-work-"));
const bin = mkdtempSync(join(tmpdir(), "toknila-plans-trash-"));
const env = { ...process.env, TOKNILA_HOME: home, TOKNILA_TRASH_DIR: bin };
const tok = (...args) => execFileSync(tokBin, args, { env, encoding: "utf8" });
const git = (dir, ...args) =>
  execFileSync("git", ["-C", dir, "-c", "user.name=t", "-c", "user.email=t@example.com", "-c", "commit.gpgsign=false", ...args], { encoding: "utf8" });

const write = (path, text) => {
  mkdirSync(join(path, ".."), { recursive: true });
  writeFileSync(path, text);
};
const tasks = (title) =>
  `# Tasks: ${title}\n\n## Phase 1: Setup\n\n- [ ] T001 Prepare ${title}\n\n## Phase 2: Build\n\n- [ ] T002 Build ${title}\n`;
const chainFile = (name) =>
  `[chain]\nname = "${name}"\ngoal = "Plan ${name}"\nautostart = false\n\n` +
  `[[ticket]]\nid = "a"\ntitle = "${name} first"\nprofile = "sim"\n\n` +
  `[[ticket]]\nid = "b"\ntitle = "${name} second"\nafter = ["a"]\nprofile = "sim"\n`;

/// A repository on develop (the form's default base branch) holding `files`.
function repo(name, files) {
  const dir = join(work, name);
  for (const [rel, text] of Object.entries(files)) write(join(dir, rel), text);
  mkdirSync(dir, { recursive: true });
  git(dir, "init", "--quiet");
  git(dir, "symbolic-ref", "HEAD", "refs/heads/develop");
  git(dir, "add", "-A");
  git(dir, "commit", "--quiet", "--allow-empty", "-m", "init");
  return dir;
}
const planner = repo("planner", {
  "specs/001-a/tasks.md": tasks("Feature A"),
  "specs/002-b/tasks.md": tasks("Feature B"),
  "plans/x.toml": chainFile("x"),
  "extra/y.toml": chainFile("y"),
});
const keeper = repo("keeper", { "README.md": "kept\n" });

let browser;
try {
  const { url } = JSON.parse(tok("open", "--json"));
  browser = await launch();
  const page = await browser.newPage();
  let pageError = "";
  page.on("pageerror", (e) => {
    pageError = e.message;
  });
  const { expectText, expectPath } = checks(page);
  const t = (id) => page.locator(`[data-testid=${id}]`);
  const plan = (rel) => page.locator(`[data-testid=found-plans] li[data-plan="${rel}"]`);
  const chainLink = (name) => page.locator(`[data-testid=chains] a[href*="/chains/${name}?"]`);
  const row = (key) => page.locator(`[data-testid=projects] > ul > li[data-project=${key}]`);
  const enabled = async (id, want, what) => {
    if ((await t(id).isEnabled()) !== want) throw new Error(`${what}: expected ${want ? "enabled" : "disabled"}`);
    console.log(`PASS ${what}`);
  };
  const addProject = async (dir, key) => {
    await t("repo-path").fill(dir);
    await page.waitForFunction((k) => document.querySelector("[data-testid=project-key]")?.value === k, key, { timeout: 10_000 });
    await t("add-project-submit").click();
    await expectPath(`/projects/${key}/settings`, `${key} is added`);
  };

  // 1. Adding the project offers the plans its repository holds; two of three are imported.
  await page.goto(url);
  await t("onboarding").waitFor({ timeout: 10_000 });
  await addProject(planner, "PLANNER");
  await expectText(t("found-plans-dialog"), "Found 3 plans in this repository", "the new project's plans are offered");
  await plan("specs/002-b").locator("input[type=checkbox]").uncheck();
  await t("import-selected").click();
  await expectText(plan("plans/x.toml"), "imported as x", "the chain file is imported");
  await expectText(plan("specs/001-a"), "imported as 001-a", "the spec folder is imported");
  if ((await plan("specs/002-b").locator("[data-testid=plan-outcome]").count()) !== 0) throw new Error("the unticked plan was imported");
  console.log("PASS the unticked plan is left alone");
  await expectPath("/projects/PLANNER/chains/x", "the first imported chain opens");
  await chainLink("x").waitFor();
  await chainLink("001-a").waitFor();
  console.log("PASS both chains are in the sidebar");
  await t("skip-plans").click();
  await t("found-plans-dialog").waitFor({ state: "detached" });

  // 2. A new scan lists the imported plans as such.
  await t("import-plus").click();
  await t("import-dialog").locator("[data-testid=scan-plans]").click();
  await expectText(t("found-count"), "Found 3 plans", "a new scan lists the three plans");
  const already = await page.locator("[data-testid=found-plans] li", { hasText: "already imported" }).count();
  if (already !== 2) throw new Error(`a new scan should mark 2 plans as already imported, marked ${already}`);
  console.log("PASS a new scan marks the two imported plans");

  // 3. Import by path: a spec folder, a chain file, and a bad path refused inline.
  await t("import-path").fill(join(work, "nowhere"));
  await t("import-path-submit").click();
  await expectText(t("error-import-path"), "nowhere", "a bad path is refused inline");
  await t("import-path").fill(join(planner, "specs", "002-b"));
  await t("import-path-submit").click();
  await expectText(t("import-result"), "Imported 002-b", "a spec folder imports by path");
  await t("import-path").fill(join(planner, "extra", "y.toml"));
  await t("import-path-submit").click();
  await expectText(t("import-result"), "Imported y", "a chain file imports by path");
  await page.keyboard.press("Escape");
  await chainLink("002-b").waitFor();
  await chainLink("y").waitFor();
  console.log("PASS the chains imported by path are in the sidebar");

  // 4. Deleting is refused while an agent works; the key guards the Trash.
  tok("start", "PLANNER-1");
  await row("PLANNER").click({ button: "right" });
  await t("menu-delete-project").click();
  await expectText(t("delete-folder"), "planner", "the dialog shows the folder");
  await enabled("delete-trash", false, "the Trash button waits for the key");
  await t("delete-confirm").fill("planner");
  await enabled("delete-trash", false, "a wrong key keeps it disabled");
  await t("delete-confirm").fill("PLANNER");
  await enabled("delete-trash", true, "the typed key enables it");
  await t("delete-remove").click();
  await expectText(t("delete-error"), "agent", "deleting is refused while an agent works");
  await expectText(t("delete-error"), "Stop its agents first", "the refusal says to stop them first");
  tok("cancel", "PLANNER-1", "stopped for the test");
  await t("delete-trash").click();
  await expectText(t("delete-result"), "PLANNER was removed", "the project is deleted");
  await expectText(t("delete-folder-note"), "is in the Trash", "the folder went to the Trash");
  await expectPath("/", "the page goes home");
  if (existsSync(planner) || !existsSync(join(bin, "planner", "specs"))) throw new Error("the folder should be in the test Trash");
  console.log("PASS the folder moved to the test Trash, not the system's");
  await t("delete-done").click();
  if (await row("PLANNER").count()) throw new Error("the deleted project is still in the sidebar");
  console.log("PASS the deleted project left the sidebar");

  // 5. Remove from Toknila only: the folder stays.
  await t("onboarding").waitFor({ timeout: 10_000 });
  await addProject(keeper, "KEEPER");
  await t("settings-nav-advanced").click();
  await t("danger-zone").locator("summary").click();
  await t("delete-project-button").click();
  await t("delete-remove").click();
  await expectText(t("delete-folder-note"), "stays where it is", "removing from Toknila keeps the folder");
  if (!existsSync(join(keeper, "README.md"))) throw new Error("the kept folder is gone");
  console.log("PASS the kept folder is untouched");
  if (pageError) throw new Error(`page error: ${pageError}`);
  console.log("web plans: PASS");
} catch (e) {
  console.error(`web plans: FAIL: ${e.message}`);
  process.exitCode = 1;
} finally {
  await browser?.close();
  await cleanup(tok, home);
  for (const dir of [work, bin]) rmSync(dir, { recursive: true, force: true, maxRetries: 5, retryDelay: 200 });
}
