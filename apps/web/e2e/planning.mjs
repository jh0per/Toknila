// Browser test of planning from the window against a real daemon (P1-29).
// Usage: node e2e/planning.mjs <path to tok binary>   (cargo xtask e2e-web runs it)
//
// An empty home adds a project from a repository folder named Toknila: the key and the name come
// from the folder. The project has no chains yet, so the window says so and offers New chain and
// Import. A chain is created (a refusal shows inline first), two cards are added at its end, one is
// edited and the other moved before it, and the canvas shows the new order. The "+" next to
// PROJECTS, the right-click menu and the desktop menu's address hash all open the add form (the
// last one adds a second project). Projects are archived from the right-click menu and from
// Settings, disappear from the sidebar, and come back through "Show archived".

import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { checks, cleanup, launch } from "./helpers.mjs";

const tokBin = process.argv[2];
if (!tokBin) {
  console.error("usage: node e2e/planning.mjs <tok binary>");
  process.exit(2);
}
const home = mkdtempSync(join(tmpdir(), "toknila-planning-"));
const tok = (...args) =>
  execFileSync(tokBin, args, { env: { ...process.env, TOKNILA_HOME: home }, encoding: "utf8" });
const git = (dir, ...args) =>
  execFileSync("git", ["-C", dir, "-c", "user.name=t", "-c", "user.email=t@example.com", "-c", "commit.gpgsign=false", ...args], { encoding: "utf8" });

/// A repository whose only branch is develop, the form's default base branch.
function repo(name) {
  const dir = join(home, "work", name);
  mkdirSync(dir, { recursive: true });
  git(dir, "init", "--quiet");
  git(dir, "symbolic-ref", "HEAD", "refs/heads/develop");
  git(dir, "commit", "--quiet", "--allow-empty", "-m", "init");
  return dir;
}
const toknila = repo("Toknila");
const spare = repo("spare-repo");

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
  const row = (key) => page.locator(`[data-testid=projects] > ul > li[data-project=${key}]`);
  const card = (key) => page.locator(`.canvas a[data-key="${key}"]`);
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
  const gone = async (loc, what) => {
    await loc.waitFor({ state: "detached", timeout: 10_000 });
    console.log(`PASS ${what}`);
  };
  const leftOf = async (a, b) => (await card(a).boundingBox()).x < (await card(b).boundingBox()).x;

  // 1. The first project: the folder suggests its key and name; the default key is kept.
  await page.goto(url);
  await t("onboarding").waitFor({ timeout: 10_000 });
  await t("repo-path").fill(toknila);
  await value("project-key", "TOKNILA", "the folder suggests the key");
  await value("project-name", "Toknila", "the folder suggests the name");
  await t("add-project-submit").click();
  await expectPath("/projects/TOKNILA/settings", "the new project opens its settings");

  // 2. No chains yet: the window says so and offers New chain and Import.
  await row("TOKNILA").locator("a").click();
  await expectPath("/projects/TOKNILA", "the project opens");
  await expectText(t("no-chains"), "No chains yet", "a project without chains says so");
  await t("empty-import").click();
  await t("import-dialog").locator("[data-testid=chain-file]").waitFor({ timeout: 10_000 });
  console.log("PASS Import opens the chain file import");
  await page.keyboard.press("Escape");
  await gone(t("import-dialog"), "Escape closes the dialog");

  // 3. A new chain: a refusal shows inline, then the chain is created and opens.
  await t("empty-new-chain").click();
  await t("create-chain").click();
  await expectText(t("chain-error"), "chain name is empty", "an empty chain name is refused inline");
  await t("chain-name").fill("plan");
  await t("chain-goal").fill("Plan from the window");
  await t("chain-autostart").uncheck();
  await t("create-chain").click();
  await expectPath("/projects/TOKNILA/chains/plan", "the new chain opens");
  await expectText(t("chain-name-top"), "plan", "the top bar names the new chain");

  // 4. Two cards at the end of the chain.
  for (const [n, title] of [[1, "First card"], [2, "Second card"]]) {
    await t("new-card").click();
    await t("card-title").fill(title);
    await t("card-prompt").fill(`Do the **${title.toLowerCase()}** thing.`);
    await t("create-card").click();
    await expectPath(`/projects/TOKNILA/chains/plan/cards/TOKNILA-${n}`, `card ${n} is added and opens`);
    await expectText(card(`TOKNILA-${n}`), title, `card ${n} is on the canvas`);
  }
  if (!(await leftOf("TOKNILA-1", "TOKNILA-2"))) throw new Error("the second card should follow the first");
  console.log("PASS the second card goes after the first (at the end)");

  // 5. Edit the first card's title and description.
  await card("TOKNILA-1").click();
  await t("edit-card").click();
  await t("edit-title").fill("First card, edited");
  await t("edit-prompt").fill("Do the first thing, then check it.");
  await t("save-card").click();
  await expectText(card("TOKNILA-1"), "First card, edited", "the edited title is on the canvas");
  await expectText(t("details"), "then check it", "the edited description is in Details");
  await gone(t("card-editor"), "the editor closes after the save");

  // 6. Move the second card before the first: the canvas order changes.
  await card("TOKNILA-2").click();
  await t("move-card").click();
  await t("move-place").selectOption("before");
  await t("move-anchor").selectOption("TOKNILA-1");
  await t("move-submit").click();
  await page.waitForFunction(() => {
    const x = (k) => document.querySelector(`.canvas a[data-key="${k}"]`)?.getBoundingClientRect().x ?? 0;
    return x("TOKNILA-2") < x("TOKNILA-1");
  }, null, { timeout: 10_000 }).catch(() => {
    throw new Error("after the move, TOKNILA-2 should be left of TOKNILA-1");
  });
  console.log("PASS the moved card now comes first on the canvas");

  // 7. The add form opens from the "+", from the right-click menu and from the desktop menu.
  await t("add-project-plus").click();
  await t("add-project-dialog").locator("[data-testid=repo-path]").waitFor({ timeout: 10_000 });
  console.log("PASS the + next to PROJECTS opens the add form");
  await page.keyboard.press("Escape");
  await gone(t("add-project-dialog"), "Escape closes the add form");
  await page.locator("[data-testid=projects] h2").click({ button: "right" });
  await t("menu-add-project").click();
  await t("add-project-dialog").waitFor({ timeout: 10_000 });
  console.log("PASS the right-click menu opens the add form");
  await t("modal-close").click();
  await gone(t("add-project-dialog"), "the close button closes the add form");
  await page.evaluate(() => {
    window.location.hash = "#/add-project";
  });
  await t("add-project-dialog").waitFor({ timeout: 10_000 });
  if (await page.evaluate(() => window.location.hash)) throw new Error("the hash should be cleared once the form is open");
  console.log("PASS the desktop menu's address hash opens the add form, then goes");
  await t("repo-path").fill(spare);
  await value("project-key", "SPAREREPO", "the dialog suggests the key");
  await value("project-name", "spare-repo", "the dialog suggests the name");
  await t("add-project-submit").click();
  await expectPath("/projects/SPAREREPO/settings", "the second project opens its settings");
  await gone(t("add-project-dialog"), "the dialog closes after adding");

  // 8. Archive from the right-click menu and from Settings; bring both back.
  await row("SPAREREPO").click({ button: "right" });
  await t("menu-archive").click();
  await gone(row("SPAREREPO"), "a project archived from the menu leaves the sidebar");
  await row("TOKNILA").click({ button: "right" });
  await t("menu-project-settings").click();
  await expectPath("/projects/TOKNILA/settings", "the menu opens the project's settings");
  await t("archive-project-button").click();
  await gone(row("TOKNILA"), "a project archived in Settings leaves the sidebar");
  await expectText(t("show-archived"), "Show archived (2)", "the sidebar offers the archived projects");
  await t("show-archived").click();
  await expectText(t("archived-projects"), "TOKNILA", "the archived projects are listed");
  await t("unarchive-TOKNILA").click();
  await row("TOKNILA").waitFor({ timeout: 10_000 });
  console.log("PASS Unarchive brings a project back to the sidebar");
  await t("archived-projects").locator("li[data-project=SPAREREPO]").click({ button: "right" });
  await t("menu-unarchive").click();
  await row("SPAREREPO").waitFor({ timeout: 10_000 });
  console.log("PASS the menu brings an archived project back");

  // 9. A project with chains and none picked shows one by itself.
  await row("TOKNILA").locator("a").click();
  await expectPath("/projects/TOKNILA", "the project opens");
  await expectText(t("chain-name-top"), "plan", "its chain is shown without a pick");
  if (pageError) throw new Error(`page error: ${pageError}`);
  console.log("web planning: PASS");
} catch (e) {
  console.error(`web planning: FAIL: ${e.message}`);
  process.exitCode = 1;
} finally {
  await browser?.close();
  await cleanup(tok, home);
}
