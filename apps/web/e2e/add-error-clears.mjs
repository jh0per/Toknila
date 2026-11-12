// Browser test: an error in the add-a-project form belongs to the text it was about.
// Usage: node e2e/add-error-clears.mjs <path to tok binary>   (cargo xtask e2e-web runs it)
//
// Onboarding: a missing folder is refused under the Folder field (and repeated under step 4); typing
// the path of a real repository removes both before the form is sent again.

import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { checks, cleanup, launch } from "./helpers.mjs";

const tokBin = process.argv[2];
if (!tokBin) {
  console.error("usage: node e2e/add-error-clears.mjs <tok binary>");
  process.exit(2);
}
const home = mkdtempSync(join(tmpdir(), "toknila-adderr-"));
const tok = (...args) =>
  execFileSync(tokBin, args, { env: { ...process.env, TOKNILA_HOME: home }, encoding: "utf8" });
const git = (dir, ...args) =>
  execFileSync("git", ["-C", dir, "-c", "user.name=t", "-c", "user.email=t@example.com", "-c", "commit.gpgsign=false", ...args], { encoding: "utf8" });

const repo = join(home, "repo");
mkdirSync(repo);
git(repo, "init", "--quiet");
git(repo, "symbolic-ref", "HEAD", "refs/heads/main");
git(repo, "commit", "--quiet", "--allow-empty", "-m", "init");

let browser;
try {
  const { url } = JSON.parse(tok("open", "--json"));
  browser = await launch();
  const page = await browser.newPage();
  const { expectText } = checks(page);
  const t = (id) => page.locator(`[data-testid=${id}]`);
  await page.goto(url);
  await t("onboarding").waitFor({ timeout: 10_000 });

  await t("repo-path").fill(join(home, "no-such-folder"));
  await t("add-project-submit").click();
  await expectText(t("error-repo_path"), "does not exist", "the missing folder is refused at the Folder field");
  await expectText(t("onboarding").locator("p.error[role=alert]"), "does not exist", "step 4 repeats it");

  // No second send: the error goes when the folder text changes.
  await t("repo-path").fill(repo);
  await t("error-repo_path").waitFor({ state: "detached", timeout: 10_000 });
  await page.locator("p.error[role=alert]").first().waitFor({ state: "detached", timeout: 10_000 });
  console.log("PASS the error is gone under the Folder field and under step 4 before the form is sent again");
  if (await t("repo-path").inputValue() !== repo) throw new Error("the folder text was not kept");
  console.log("web add-error-clears: PASS");
} catch (e) {
  console.error(`web add-error-clears: FAIL: ${e.message}`);
  process.exitCode = 1;
} finally {
  await browser?.close();
  await cleanup(tok, home);
}
