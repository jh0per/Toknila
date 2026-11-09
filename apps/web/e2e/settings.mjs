// Browser test of onboarding and settings against a real daemon (rebuilt in D2).
// Usage: node e2e/settings.mjs <path to tok binary>   (cargo xtask e2e-web runs it)
//
// An empty home opens onboarding: the git, claude and gh checks, then a form that adds a project
// from a temporary repository (a typed folder; no Browse outside the desktop app). What the folder
// suggests (key, name, main branch, test command, merge mode) is under a collapsed Details as real
// values; wrong folders and a missing main branch are refused next to their field and open Details
// (the repository's branches are offered). Then in Settings, which replace the sidebar with a back
// arrow and a flat list (Projects, Agents, Updates, About, System checks): a chain file is
// imported, the parallel limit, the review mode, the merge settings and the Claude models are
// changed with ONE Save bar that shows only when something changed, the top bar shows the new
// limit, and all of it is still there after the daemon restarts. Ctrl+, opens the settings, Back to
// work returns to the page they were opened from.

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
// An xtask with a verify command: the form suggests `cargo xtask verify` as the test command.
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
  const value = async (id, expected, what) => {
    await page.waitForFunction(
      ([sel, v]) => document.querySelector(sel)?.value === v,
      [`[data-testid=${id}]`, expected],
      { timeout: 10_000 },
    ).catch(async () => {
      throw new Error(`${what}: expected "${expected}", got "${await t(id).inputValue().catch(() => "?")}"`);
    });
    console.log(`PASS ${what}: ${expected}`);
  };
  const absent = async (loc, what) => {
    if (await loc.count()) throw new Error(`${what}: it is there`);
    console.log(`PASS ${what}`);
  };

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
  // The Claude Code trust step is one line with a How to, not a paragraph.
  await expectText(t("trust-note"), "asks once to trust a project folder", "the trust step is one line");
  if (await t("trust-note").locator("details").evaluate((d) => d.open)) throw new Error("How to should start closed");
  console.log("PASS its How to starts closed");

  // The add form: a folder, and Details folded. No Browse outside the desktop app.
  await absent(t("browse-folder"), "a browser page has no Browse button");
  if (await t("add-details").evaluate((d) => d.open)) throw new Error("Details should start closed");
  console.log("PASS Details starts closed");
  // A folder that does not exist is refused next to the path field.
  await t("repo-path").fill(join(home, "no-such-folder"));
  await t("add-project-submit").click();
  await expectText(t("error-repo_path"), "does not exist", "missing folder is refused at the path field");
  // A folder that is not a repository.
  await t("repo-path").fill(home);
  await t("add-project-submit").click();
  await expectText(t("error-repo_path"), "not a git repository", "plain folder is refused at the path field");
  // A repository: everything it suggests shows as a real value under Details.
  await t("repo-path").fill(repo);
  await value("work-branch", "main", "the main branch is prefilled with the branch the repository is on");
  await value("project-key", "REPO", "the key is detected from the folder");
  await value("project-name", "repo", "the name is detected from the folder");
  await value("new-check-command", "cargo xtask verify", "the test command is detected from the repository");
  if ((await t("new-merge-mode").inputValue()) !== "local") throw new Error("a repository without a remote should suggest local");
  console.log("PASS the merge mode is local without a remote");
  await expectText(t("add-found"), "merges locally", "the closed Details line says what was found");
  if (await t("add-details").evaluate((d) => d.open)) throw new Error("Details should stay closed while nothing is refused");
  // A missing main branch is refused at its field, which opens Details and offers the branches.
  await t("add-details").locator("summary").click();
  await t("project-key").fill("DEMO");
  await t("work-branch").fill("develop");
  await t("add-project-submit").click();
  await expectText(t("error-work_branch"), "develop", "missing main branch is refused at the branch field");
  await expectText(t("branches"), "main", "the repository's branches are offered");
  await page.locator("[data-branch=main]").click();
  await t("add-project-submit").click();
  await expectPath("/projects/DEMO/planner", "the first project leads into the assistant");
  await t("settings-link").click();
  await expectPath("/projects/DEMO/settings", "the settings open");

  // Settings: the sidebar is a back arrow and a flat list; the project picker names the project.
  await t("settings-sidebar").waitFor();
  await absent(t("projects"), "the settings have no project list in the sidebar");
  for (const page_ of ["projects", "profiles", "updates", "about", "checks"]) {
    await t(`settings-page-${page_}`).waitFor();
  }
  console.log("PASS the flat list: Projects, Agents, Updates, About, System checks");
  await value("project-picker", "DEMO", "the project picker names the project");
  await expectText(t("project-name-shown"), "repo", "General shows the project's name");
  await absent(page.getByText("Release branch"), "General does not show the release branch");
  await absent(t("savebar"), "the Save bar is hidden while nothing changed");

  // Import a chain file (optional step), in the Plans section.
  await t("settings-nav-plans").click();
  await expectPath("/projects/DEMO/settings/plans", "Plans has its own address");
  await t("chain-file").setInputFiles(demo);
  await expectText(t("imported"), "Imported chain demo", "chain file imported");

  // Agents: the limit and the test command the form detected.
  await t("settings-nav-agents").click();
  await expectPath("/projects/DEMO/settings/agents", "Agents has its own address");
  await value("check-command", "cargo xtask verify", "the project was added with the detected test command");
  await value("max-parallel", "2", "the default limit is 2");
  await expectText(page.locator("[data-testid=project-agents] .field-hint"), "before they hand in", "the test command says when agents run it");
  await t("max-parallel").fill("3");
  await t("savebar").waitFor();
  await expectText(t("savebar"), "unsaved changes", "the Save bar appears with the first change");
  // The one Save bar covers every section: the models are in Advanced.
  await t("settings-nav-advanced").click();
  await expectPath("/projects/DEMO/settings/advanced", "Advanced has its own address");
  await expectText(t("savebar"), "unsaved changes", "the Save bar follows to the other section");
  for (const id of ["worker-model", "reviewer-model", "planner-model"]) {
    if ((await t(id).inputValue()) !== "") throw new Error(`${id} should be empty (Claude Code's own default)`);
  }
  await expectText(t("model-aliases"), "Leave empty to use your Claude Code default", "the models say what empty means");
  const aliases = await page.locator("#model-aliases option").evaluateAll((os) => os.map((o) => o.value));
  if (!["sonnet", "opus", "haiku"].every((a) => aliases.includes(a))) throw new Error(`the aliases offered: ${aliases}`);
  console.log("PASS the documented model aliases are offered");
  await absent(page.getByText("Release branch"), "Advanced shows the release branch only when it differs from the main branch");
  // Where things live and the danger zone are folded.
  for (const id of ["places", "danger-zone"]) {
    if (await t(id).evaluate((d) => d.open)) throw new Error(`${id} should start folded`);
  }
  console.log("PASS Where things live and the Danger zone start folded");
  await t("places").locator("summary").click();
  await expectText(t("place-home"), "toknila-settings-", "Where things live says where the data lives");
  await expectText(t("place-ships"), "ships", "and where the workspaces are");
  await t("worker-model").fill("sonnet");
  await t("planner-model").fill("not a model");
  await t("save-settings").click();
  await expectText(t("error-planner_model"), "one word", "a model with spaces is refused at its field");
  await expectText(t("savebar-error"), "one word", "and the Save bar says so");
  await t("planner-model").fill("opus");
  await t("settings-nav-agents").click();
  await t("check-command").fill("cargo test");
  await t("save-settings").click();
  await expectText(t("saved"), "Saved", "settings saved");
  await absent(t("save-settings"), "Save goes away once it is saved");
  // A refused value comes back at its field and changes nothing.
  await t("max-parallel").fill("99");
  await t("save-settings").click();
  await expectText(t("error-max_parallel"), "between 1 and 64", "an out-of-range limit is refused at its field");
  // Discard puts every field back and hides the bar.
  await t("discard-settings").click();
  await value("max-parallel", "3", "Discard puts the saved limit back");
  await absent(t("savebar"), "and hides the Save bar");
  await t("check-command").fill("cargo xtask verify");
  await t("save-settings").click();
  await expectText(t("saved"), "Saved", "settings saved again");

  // Review and merge: the review mode, and merging through GitHub (title and method appear with it).
  await t("settings-nav-review").click();
  await expectPath("/projects/DEMO/settings/review", "Review and merge has its own address");
  await absent(t("saved"), "the Saved of another page does not follow");
  // A new project reviews with both (the loop first, then the person), three rounds of it.
  await value("review-mode", "both", "a new project defaults to review mode both");
  await value("review-rounds", "3", "the round limit shows next to the review mode, default 3");
  await t("review-rounds").fill("21");
  await t("save-settings").click();
  await expectText(t("error-max_review_rounds"), "0 to 20", "an out-of-range round limit is refused at its field");
  await t("review-rounds").fill("5");
  await t("review-mode").selectOption("agent");
  if ((await t("merge-mode").inputValue()) !== "local") throw new Error("the default merge mode should be local");
  await expectText(page.locator("[data-testid=project-review]"), "When a card is approved", "the merge setting says when it applies");
  await absent(t("pr-title"), "the pull request fields belong to github mode");
  await t("merge-mode").selectOption("github");
  await value("pr-title", "{key}: {title}", "the default title template");
  await t("pr-title").fill("[{key}] {title}");
  await t("merge-method").selectOption("squash");
  // The sites the reviewer may read: none by default; a URL is refused at its field; then saved.
  await value("reviewer-web-domains", "", "the reviewer may read no site by default");
  if (tok("project", "show", "DEMO").includes("reviewer reads")) throw new Error("a new project lists sites for the reviewer");
  await t("reviewer-web-domains").fill("https://docs.rs/x");
  await t("save-settings").click();
  await expectText(t("error-reviewer_web_domains"), "plain host name", "a URL is refused at the sites field");
  await t("reviewer-web-domains").fill("Docs.rs\nexample.com");
  await t("save-settings").click();
  await expectText(t("saved"), "Saved", "review and merge settings saved");
  if (!tok("project", "show", "DEMO").includes("reviewer reads: docs.rs, example.com")) {
    throw new Error(`tok project show: ${tok("project", "show", "DEMO")}`);
  }
  console.log("PASS the reviewer's sites are saved lower-cased and tok project show lists them");

  // The top bar of the chain shows the new limit; a project that opens pull requests has the tab.
  await page.goto(at("/projects/DEMO/chains/demo"));
  await expectText(t("slots"), "/3 agents", "the top bar shows the new limit");

  // Ctrl+, opens the settings from anywhere; Back to work returns to the page they were opened from.
  await page.keyboard.press("Control+,");
  await expectPath("/projects/DEMO/settings", "Ctrl+, opens the settings");
  await t("settings-page-checks").click();
  await expectPath("/projects/DEMO/settings/checks", "System checks has its own address");
  await t("checks").waitFor();
  await t("settings-back").click();
  await expectPath("/projects/DEMO/chains/demo", "Back to work returns to the page the settings were opened from");
  await t("settings-link").click();
  await expectPath("/projects/DEMO/settings", "the Settings link opens them too");

  // Settings survive a daemon restart.
  tok("daemon", "stop");
  tok("daemon", "start");
  await page.goto(at("/projects/DEMO/settings/agents"));
  await t("max-parallel").waitFor({ timeout: 20_000 });
  const limit = await t("max-parallel").inputValue();
  if (limit !== "3") throw new Error(`after the restart: limit ${limit}`);
  const check = await t("check-command").inputValue();
  if (check !== "cargo xtask verify") throw new Error(`after the restart: test command ${check}`);
  console.log("PASS limit 3 and the test command survive a daemon restart");
  await page.goto(at("/projects/DEMO/settings/advanced"));
  await t("worker-model").waitFor({ timeout: 20_000 });
  const models = [await t("worker-model").inputValue(), await t("reviewer-model").inputValue(), await t("planner-model").inputValue()];
  if (models.join("|") !== "sonnet||opus") throw new Error(`after the restart: models ${models}`);
  console.log("PASS worker model sonnet, no reviewer model and planner model opus survive a daemon restart");
  await page.goto(at("/projects/DEMO/settings/review"));
  await t("review-mode").waitFor({ timeout: 20_000 });
  const mode = await t("review-mode").inputValue();
  if (mode !== "agent") throw new Error(`after the restart: review mode ${mode}`);
  console.log("PASS review mode agent survives a daemon restart");
  await value("review-rounds", "5", "the round limit 5 survives a daemon restart");
  const sites = await t("reviewer-web-domains").inputValue();
  if (sites !== "docs.rs\nexample.com") throw new Error(`after the restart: reviewer sites ${JSON.stringify(sites)}`);
  console.log("PASS the reviewer's sites survive a daemon restart");
  const merge = [await t("merge-mode").inputValue(), await t("pr-title").inputValue(), await t("merge-method").inputValue()];
  if (merge.join("|") !== "github|[{key}] {title}|squash") throw new Error(`after the restart: merge settings ${merge}`);
  console.log("PASS merge mode github, the title template and squash survive a daemon restart");

  // The other pages of the list: Agents (profiles), Updates, About, System checks; old addresses still open.
  await t("settings-page-profiles").click();
  await expectText(t("profiles-folder"), "profiles", "Agents says where the agent files go");
  await page.locator("[data-testid=profiles] li[data-profile=claude-code]").waitFor();
  await expectText(page.locator("[data-profile=claude-code]"), "built in", "the built-in agents are listed");
  await t("settings-page-updates").click();
  await expectText(t("updates"), "desktop app", "Updates says it comes with the desktop app in a browser");
  await t("settings-page-about").click();
  await expectText(t("version"), "Toknila", "About shows the version");
  await page.goto(at("/settings"));
  await value("project-picker", "DEMO", "/settings is the Projects page");
  await page.goto(at("/settings/checks"));
  await t("checks").waitFor({ timeout: 20_000 });
  console.log("PASS /settings/checks is System checks");
  await page.goto(at("/projects/DEMO/settings/danger"));
  await t("project-advanced").waitFor({ timeout: 20_000 });
  console.log("PASS the old /danger address opens Advanced");
  if (pageError) throw new Error(`page error: ${pageError}`);
  console.log("web settings: PASS");
} catch (e) {
  console.error(`web settings: FAIL: ${e.message}`);
  process.exitCode = 1;
} finally {
  await browser?.close();
  await cleanup(tok, home);
}
