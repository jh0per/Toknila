// Browser test of onboarding and settings against a real daemon (P1-19).
// Usage: node e2e/settings.mjs <path to tok binary>   (cargo xtask e2e-web runs it)
//
// An empty home opens onboarding: the git, claude and gh checks, then a form that adds a project
// from a temporary repository. Wrong folders and a missing base branch are refused next to their
// field (the repository's branches are offered). Then a chain file is imported, the parallel limit
// the review mode and the merge settings (github mode, title template, squash) are changed, the top
// bar shows the new limit, and all of them are still there after the daemon restarts.

import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync } from "node:fs";
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
git(repo, "commit", "--quiet", "--allow-empty", "-m", "init");

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
  await t("work-branch").fill("develop");
  await t("add-project-submit").click();
  await expectText(t("error-work_branch"), "develop", "missing base branch is refused at the branch field");
  await expectText(t("branches"), "main", "the repository's branches are offered");
  await page.locator("[data-branch=main]").click();
  await t("add-project-submit").click();
  await expectPath("/projects/DEMO/settings", "the new project opens its settings");

  // Import a chain file (optional step).
  await t("chain-file").setInputFiles(demo);
  await expectText(t("imported"), "Imported chain demo", "chain file imported");

  // Change the limit and the review mode.
  await expectText(t("settings"), "Settings of DEMO", "settings of the new project");
  if ((await t("max-parallel").inputValue()) !== "2") throw new Error("the default limit should be 2");
  await t("max-parallel").fill("3");
  await t("review-mode").selectOption("agent");
  // Merging through GitHub: the pull request title and the merge method appear with the mode.
  if ((await t("merge-mode").inputValue()) !== "local") throw new Error("the default merge mode should be local");
  if (await t("pr-title").count()) throw new Error("the pull request fields belong to github mode");
  await t("merge-mode").selectOption("github");
  if ((await t("pr-title").inputValue()) !== "{key}: {title}") throw new Error("the default title template");
  await t("pr-title").fill("[{key}] {title}");
  await t("merge-method").selectOption("squash");
  await t("check-command").fill("cargo xtask verify");
  await t("save-settings").click();
  await expectText(t("saved"), "Saved", "settings saved");
  // A refused value comes back at its field and changes nothing.
  await t("max-parallel").fill("99");
  await t("save-settings").click();
  await expectText(t("error-max_parallel"), "between 1 and 64", "an out-of-range limit is refused at its field");

  // The top bar of the chain shows the new limit.
  await page.goto(at("/projects/DEMO/chains/demo"));
  await expectText(t("slots"), "/3 agents", "the top bar shows the new limit");

  // Settings survive a daemon restart.
  tok("daemon", "stop");
  tok("daemon", "start");
  await page.goto(at("/projects/DEMO/settings"));
  await t("max-parallel").waitFor({ timeout: 20_000 });
  const limit = await t("max-parallel").inputValue();
  const mode = await t("review-mode").inputValue();
  if (limit !== "3" || mode !== "agent") throw new Error(`after the restart: limit ${limit}, review mode ${mode}`);
  console.log("PASS limit 3 and review mode agent survive a daemon restart");
  const merge = [await t("merge-mode").inputValue(), await t("pr-title").inputValue(), await t("merge-method").inputValue()];
  if (merge.join("|") !== "github|[{key}] {title}|squash") throw new Error(`after the restart: merge settings ${merge}`);
  console.log("PASS merge mode github, the title template and squash survive a daemon restart");
  const check = await t("check-command").inputValue();
  if (check !== "cargo xtask verify") throw new Error(`after the restart: check command ${check}`);
  console.log("PASS the check command survives a daemon restart");
  if (pageError) throw new Error(`page error: ${pageError}`);
  console.log("web settings: PASS");
} catch (e) {
  console.error(`web settings: FAIL: ${e.message}`);
  process.exitCode = 1;
} finally {
  await browser?.close();
  await cleanup(tok, home);
}
