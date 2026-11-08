// Browser test of onboarding and settings against a real daemon (P1-19).
// Usage: node e2e/settings.mjs <path to tok binary>   (cargo xtask e2e-web runs it)
//
// An empty home opens onboarding: the git, claude and gh checks, then a form that adds a project
// from a temporary repository. Wrong folders and a missing base branch are refused next to their
// field (the repository's branches are offered). Then a chain file is imported, the parallel limit
// the review mode and the merge settings (github mode, title template, squash) are changed, the top
// bar shows the new limit, and all of them are still there after the daemon restarts. Settings are
// sections with an address each (App, Profiles, and the project's General, Agents, Review and
// merge, Plans, Danger zone). The form prefills the check command the repository's files suggest,
// and the agent model is saved with the project.

import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { checks, cleanup, launch } from "./helpers.mjs";

const tokBin = process.argv[2];
if (!tokBin) {
  console.error("usage: node e2e/settings.mjs <tok binary>");
  process.exit(2);
}
const home = mkdtempSync(join(tmpdir(), "toknila-settings-"));
const demo = resolve(import.meta.dirname, "../../../examples/demo.toml");
const tok = (...args) =>
  execFileSync(tokBin, args, { env: { ...process.env, TOKNILA_HOME: home }, encoding: "utf8" });
const git = (dir, ...args) =>
  execFileSync("git", ["-C", dir, "-c", "user.name=t", "-c", "user.email=t@example.com", "-c", "commit.gpgsign=false", ...args], { encoding: "utf8" });

// A repository whose only branch is `main`: the form prefills it, and typing develop misses it.
const repo = join(home, "repo");
mkdirSync(repo);
git(repo, "init", "--quiet");
git(repo, "symbolic-ref", "HEAD", "refs/heads/main");
// An xtask with a verify command: the form suggests `cargo xtask verify` as the check command.
mkdirSync(join(repo, "xtask", "src"), { recursive: true });
writeFileSync(join(repo, "Cargo.toml"), "[workspace]\nmembers = [\"xtask\"]\n");
writeFileSync(join(repo, "xtask", "src", "main.rs"), 'fn main() { match "verify" { "verify" => {}, _ => {} } }\n');
git(repo, "add", "-A");
git(repo, "commit", "--quiet", "-m", "init");

let browser;
try {
  const { url } = JSON.parse(tok("open", "--json"));
  const at = (path) => url.replace("/?", `${path}?`);
  browser = await launch();
  const page = await browser.newPage();
  let pageError = "";
  page.on("pageerror", (e) => {
    pageError = e.message;
  });
  const { expectText, expectPath } = checks(page);
  const t = (id) => page.locator(`[data-testid=${id}]`);

  await page.goto(url);
  await t("onboarding").waitFor({ timeout: 10_000 });
  console.log("PASS an empty home opens onboarding");
  for (const tool of ["git", "claude", "gh"]) {
    await page.locator(`[data-testid=checks] li[data-tool=${tool}]`).waitFor({ timeout: 20_000 });
  }
  console.log("PASS the git, claude and gh checks are listed");
  // The checks reflect this machine; whatever is not fine says how to fix it.
  const bad = await page.locator("[data-testid=checks] li:not([data-status=ok])").count();
  const hints = await page.locator("[data-testid=checks] .hint").count();
  if (bad !== hints) throw new Error(`${bad} checks failed but ${hints} hints are shown`);
  console.log(`PASS every failing check has a fix hint (${bad} failing here)`);

  // A folder that does not exist is refused next to the path field.
  await t("project-key").fill("DEMO");
  await t("repo-path").fill(join(home, "no-such-folder"));
  await t("add-project-submit").click();
  await expectText(t("error-repo_path"), "does not exist", "missing folder is refused at the path field");
  // A folder that is not a repository.
  await t("repo-path").fill(home);
  await t("add-project-submit").click();
  await expectText(t("error-repo_path"), "not a git repository", "plain folder is refused at the path field");
  // A repository without the base branch: the error is at the branch field and the branches are offered.
  await t("repo-path").fill(repo);
  await page.waitForFunction(() => document.querySelector("[data-testid=work-branch]")?.value === "main", null, { timeout: 10_000 });
  console.log("PASS the base branch is prefilled with the branch the repository is on");
  // The check command comes from the repository's files and the merge mode from its remote.
  await page.waitForFunction(() => document.querySelector("[data-testid=new-check-command]")?.value === "cargo xtask verify", null, { timeout: 10_000 });
  if ((await t("new-merge-mode").inputValue()) !== "local") throw new Error("a repository without a remote should suggest local");
  console.log("PASS the check command is prefilled from the repository, the merge mode is local without a remote");
  await t("work-branch").fill("develop");
  await t("add-project-submit").click();
  await expectText(t("error-work_branch"), "develop", "missing base branch is refused at the branch field");
  await expectText(t("branches"), "main", "the repository's branches are offered");
  await page.locator("[data-branch=main]").click();
  await t("add-project-submit").click();
  await expectPath("/projects/DEMO/settings", "the new project opens its settings");

  // The new project opens its General section; the section list leads to the others.
  await expectText(t("project-general"), "General", "the new project opens its General section");
  await expectText(t("project-name-shown"), "repo", "General shows the project's name");
  await expectText(t("settings-nav-project"), "Project DEMO", "the section list names the project");

  // Import a chain file (optional step), in the Plans section.
  await t("settings-nav-plans").click();
  await expectPath("/projects/DEMO/settings/plans", "Plans has its own address");
  await t("chain-file").setInputFiles(demo);
  await expectText(t("imported"), "Imported chain demo", "chain file imported");

  // Agents: the limit, the check command the form prefilled, and the Claude model.
  await t("settings-nav-agents").click();
  await expectPath("/projects/DEMO/settings/agents", "Agents has its own address");
  if ((await t("check-command").inputValue()) !== "cargo xtask verify") throw new Error("the prefilled check command was not saved with the project");
  console.log("PASS the project was added with the prefilled check command");
  if ((await t("max-parallel").inputValue()) !== "2") throw new Error("the default limit should be 2");
  if ((await t("agent-model").inputValue()) !== "") throw new Error("the model should be empty (Claude Code's own default)");
  await t("max-parallel").fill("3");
  await page.locator("[data-alias=sonnet]").click();
  if ((await t("agent-model").inputValue()) !== "sonnet") throw new Error("an alias button should fill the model");
  await t("agent-model").fill("not a model");
  await t("save-settings").click();
  await expectText(t("error-agent_model"), "one word", "a model with spaces is refused at its field");
  await t("agent-model").fill("opus");
  await t("check-command").fill("cargo test");
  await t("save-settings").click();
  await expectText(t("saved"), "Saved", "agent settings saved");
  // A refused value comes back at its field and changes nothing.
  await t("max-parallel").fill("99");
  await t("save-settings").click();
  await expectText(t("error-max_parallel"), "between 1 and 64", "an out-of-range limit is refused at its field");
  await t("max-parallel").fill("3");
  await t("check-command").fill("cargo xtask verify");
  await t("save-settings").click();
  await expectText(t("saved"), "Saved", "agent settings saved again");

  // Review and merge: the review mode, and merging through GitHub (title and method appear with it).
  await t("settings-nav-review").click();
  await expectPath("/projects/DEMO/settings/review", "Review and merge has its own address");
  if (await t("saved").count()) throw new Error("the Saved of another section should not follow");
  await t("review-mode").selectOption("agent");
  if ((await t("merge-mode").inputValue()) !== "local") throw new Error("the default merge mode should be local");
  if (await t("pr-title").count()) throw new Error("the pull request fields belong to github mode");
  await t("merge-mode").selectOption("github");
  if ((await t("pr-title").inputValue()) !== "{key}: {title}") throw new Error("the default title template");
  await t("pr-title").fill("[{key}] {title}");
  await t("merge-method").selectOption("squash");
  await t("save-settings").click();
  await expectText(t("saved"), "Saved", "review and merge settings saved");

  // The top bar of the chain shows the new limit.
  await page.goto(at("/projects/DEMO/chains/demo"));
  await expectText(t("slots"), "/3 agents", "the top bar shows the new limit");

  // Settings survive a daemon restart.
  tok("daemon", "stop");
  tok("daemon", "start");
  await page.goto(at("/projects/DEMO/settings/agents"));
  await t("max-parallel").waitFor({ timeout: 20_000 });
  const limit = await t("max-parallel").inputValue();
  const model = await t("agent-model").inputValue();
  if (limit !== "3" || model !== "opus") throw new Error(`after the restart: limit ${limit}, model ${model}`);
  console.log("PASS limit 3 and the model opus survive a daemon restart");
  const check = await t("check-command").inputValue();
  if (check !== "cargo xtask verify") throw new Error(`after the restart: check command ${check}`);
  console.log("PASS the check command survives a daemon restart");
  await page.goto(at("/projects/DEMO/settings/review"));
  await t("review-mode").waitFor({ timeout: 20_000 });
  const mode = await t("review-mode").inputValue();
  if (mode !== "agent") throw new Error(`after the restart: review mode ${mode}`);
  console.log("PASS review mode agent survives a daemon restart");
  const merge = [await t("merge-mode").inputValue(), await t("pr-title").inputValue(), await t("merge-method").inputValue()];
  if (merge.join("|") !== "github|[{key}] {title}|squash") throw new Error(`after the restart: merge settings ${merge}`);
  console.log("PASS merge mode github, the title template and squash survive a daemon restart");

  // App: the checks and where things live; Profiles: the list and its folder.
  await t("settings-nav-app").click();
  await expectPath("/projects/DEMO/settings/app", "App has its own address");
  await t("checks").waitFor();
  await expectText(t("place-home"), "toknila-settings-", "App says where the data lives");
  await expectText(t("place-ships"), "ships", "App says where the ships are");
  await t("settings-nav-profiles").click();
  await expectText(t("profiles-folder"), "profiles", "Profiles says where the profile files go");
  await page.locator("[data-testid=profiles] li[data-profile=claude-code]").waitFor();
  await expectText(page.locator("[data-profile=claude-code]"), "built in", "the built-in profiles are listed");
  await page.goto(at("/settings"));
  await t("checks").waitFor({ timeout: 20_000 });
  console.log("PASS /settings is the App section");
  if (pageError) throw new Error(`page error: ${pageError}`);
  console.log("web settings: PASS");
} catch (e) {
  console.error(`web settings: FAIL: ${e.message}`);
  process.exitCode = 1;
} finally {
  await browser?.close();
  await cleanup(tok, home);
}
