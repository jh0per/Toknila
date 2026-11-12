// Browser test: the add-a-project form fixes a new folder in place.
// Usage: node e2e/add-fix-folder.mjs <path to tok binary>   (cargo xtask e2e-web runs it)
//
// Onboarding: the path of a folder that does not exist shows its problem without pressing Add project;
// Create the folder, then Make it a git repository, leave no error and a filled Main branch (visible
// without opening Details); Add project then adds the project. A repository on `develop` shows
// `develop` before the form is sent.

import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { checks, cleanup, launch } from "./helpers.mjs";

const tokBin = process.argv[2];
if (!tokBin) {
  console.error("usage: node e2e/add-fix-folder.mjs <tok binary>");
  process.exit(2);
}
const home = mkdtempSync(join(tmpdir(), "toknila-addfix-"));
// The first commit needs an identity in git's configuration (the daemon asks `git config`); this
// one is a global config of its own, so the script does not depend on the machine's.
const gitconfig = join(home, "gitconfig");
writeFileSync(gitconfig, "[user]\n\tname = t\n\temail = t@example.com\n[init]\n\tdefaultBranch = main\n");
const identity = { GIT_CONFIG_GLOBAL: gitconfig, GIT_CONFIG_NOSYSTEM: "1" };
const tok = (...args) =>
  execFileSync(tokBin, args, { env: { ...process.env, ...identity, TOKNILA_HOME: home }, encoding: "utf8" });
const git = (dir, ...args) =>
  execFileSync("git", ["-C", dir, "-c", "commit.gpgsign=false", ...args], { encoding: "utf8", env: { ...process.env, ...identity } });

const develop = join(home, "develop-repo");
mkdirSync(develop);
git(develop, "init", "--quiet");
git(develop, "symbolic-ref", "HEAD", "refs/heads/develop");
git(develop, "commit", "--quiet", "--allow-empty", "-m", "init");

let browser;
try {
  const { url } = JSON.parse(tok("open", "--json"));
  browser = await launch();
  const page = await browser.newPage();
  const { expectText } = checks(page);
  const t = (id) => page.locator(`[data-testid=${id}]`);
  await page.goto(url);
  await t("onboarding").waitFor({ timeout: 10_000 });

  // A repository on develop: the Main branch says so before anything is sent, Details still folded.
  await t("repo-path").fill(develop);
  await page.waitForFunction(() => document.querySelector("[data-testid=work-branch]")?.value === "develop", null, { timeout: 10_000 });
  if (await t("add-details").evaluate((d) => d.open)) throw new Error("Details is open");
  console.log("PASS the Main branch shows develop without opening Details");

  // A folder that does not exist: its problem shows without Add project, with its button.
  const fresh = join(home, "new", "project");
  await t("repo-path").fill(fresh);
  await expectText(t("error-repo_path"), "does not exist", "the missing folder shows before Add project is pressed");
  await expectText(t("fix-folder"), "Create the folder", "the problem has its button");
  await t("fix-folder").click();
  await expectText(t("error-repo_path"), "is not a git repository", "the next problem appears");
  if (!existsSync(fresh)) throw new Error("the folder was not made");
  await expectText(t("fix-folder"), "Make it a git repository", "with its own button");
  await t("fix-folder").click();
  await t("error-repo_path").waitFor({ state: "detached", timeout: 15_000 });
  await t("fix-folder").waitFor({ state: "detached", timeout: 5_000 });
  await page.waitForFunction(() => (document.querySelector("[data-testid=work-branch]")?.value ?? "") !== "", null, { timeout: 10_000 });
  const branch = await t("work-branch").inputValue();
  console.log(`PASS no error is left and the Main branch is filled: ${branch}`);

  await t("add-project-submit").click();
  await t("onboarding").waitFor({ state: "detached", timeout: 15_000 });
  // The key comes from the folder's name; `project show` fails when there is no such project.
  const shown = tok("project", "show", "PROJECT");
  if (!shown.includes("PROJECT")) throw new Error(`the project is not there: ${shown}`);
  console.log("PASS Add project added the project");
  console.log("web add-fix-folder: PASS");
} catch (e) {
  console.error(`web add-fix-folder: FAIL: ${e.message}`);
  process.exitCode = 1;
} finally {
  await browser?.close();
  await cleanup(tok, home);
}
