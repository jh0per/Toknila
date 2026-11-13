// Browser test of hiding a project from the sidebar's list against a real daemon.
// Usage: node e2e/hide-project.mjs <path to tok binary>   (cargo xtask e2e-web runs it)
//
// Three --sim projects; HPB has a running story that asks a question. HPB is hidden from the
// right-click menu: its row leaves the list, "Show hidden (1)" says one item waits, the question is
// still in the inbox and the story still runs, and a reload keeps it hidden. Cmd/Ctrl+K and its key
// open it: the sidebar shows its tree with the "hidden" mark, and it leaves the list when the person
// goes elsewhere. "Show in list" (the button, then the right-click menu) brings a project back.

import { execFileSync } from "node:child_process";
import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { checks, cleanup, launch, shoot } from "./helpers.mjs";

const tokBin = process.argv[2];
if (!tokBin) {
  console.error("usage: node e2e/hide-project.mjs <tok binary>");
  process.exit(2);
}
const home = mkdtempSync(join(tmpdir(), "toknila-hide-"));
const tok = (...args) =>
  execFileSync(tokBin, args, { env: { ...process.env, TOKNILA_HOME: home }, encoding: "utf8" });
const status = (key) => JSON.parse(tok("show", key, "--json")).ticket.status;
const hiddenFlag = (key) => Boolean(JSON.parse(tok("project", "ls", "--json")).find((p) => p.key === key)?.hidden);

let browser;
try {
  for (const key of ["HPA", "HPB", "HPC"]) tok("project", "new", key, "--sim");
  tok("review-mode", "human", "--project", "HPB");
  const plan = join(home, "live.toml");
  writeFileSync(plan, '[chain]\nname = "live"\nautostart = true\n\n[[ticket]]\nid = "asks"\ntitle = "asks"\nprofile = "sim"\n');
  tok("chain", "import", plan, "--project", "HPB");
  tok("sim", "HPB-1", "ask", "which port?");
  const before = status("HPB-1");
  if (before !== "running" && before !== "waiting") throw new Error(`HPB-1 should run, it is ${before}`);

  const { url } = JSON.parse(tok("open", "--json"));
  browser = await launch();
  const page = await browser.newPage();
  let pageError = "";
  page.on("pageerror", (e) => {
    pageError = e.message;
  });
  const { expectText, expectPath, expectLink } = checks(page);
  const t = (id) => page.locator(`[data-testid=${id}]`);
  // A project in the sidebar's list, and one in the hidden group.
  const listed = (key) => page.locator(`[data-testid=projects] > ul[aria-label=Projects] > li[data-project=${key}]`);
  const inHidden = (key) => t("hidden-projects").locator(`li[data-project=${key}]`);
  const gone = async (loc, what) => {
    await loc.waitFor({ state: "detached", timeout: 10_000 }).catch(() => {
      throw new Error(`${what}: still there`);
    });
    console.log(`PASS ${what}`);
  };
  await page.goto(url);
  await expectLink("live", "stream connected");
  await listed("HPA").locator("a").first().click();
  await expectPath("/projects/HPA", "HPA is on the screen");
  await expectText(t("inbox-count"), "1", "the question waits in the inbox");

  // Hide HPB from the right-click menu.
  await listed("HPB").locator("> .row").click({ button: "right" });
  await expectText(t("menu-hide-project"), "Hide from list", "the menu offers Hide from list");
  await t("menu-hide-project").click();
  await gone(listed("HPB"), "the hidden project's row leaves the list");
  await expectText(t("show-hidden"), "Show hidden (1)", "the row under the list counts it");
  await expectText(t("hidden-waiting"), "1", "and says one item waits in it");
  if (!hiddenFlag("HPB")) throw new Error("the daemon does not say HPB is hidden");
  await expectText(t("inbox-count"), "1", "the question is still counted in the inbox");
  await t("inbox-link").click();
  await expectText(t("inbox").locator('li[data-kind=question][data-key="HPB-1"]'), "which port?", "the question is still in the inbox");
  const after = status("HPB-1");
  if (after !== before) throw new Error(`hiding changed HPB-1 from ${before} to ${after}`);
  console.log(`PASS the story still runs (${after})`);
  await listed("HPA").locator("a").first().click();
  await expectPath("/projects/HPA", "back on HPA");
  await shoot(page, "hide-project-closed", page.locator("nav[aria-label=Sidebar]"));

  // A reload keeps it hidden.
  await page.reload();
  await expectLink("live", "stream connected after the reload");
  await expectText(t("show-hidden"), "Show hidden (1)", "a reload keeps it hidden");
  if (await listed("HPB").count()) throw new Error("HPB is back in the list after a reload");
  console.log("PASS HPB stays out of the list after a reload");

  // Cmd/Ctrl+K and its key open it; the sidebar shows its tree with the mark while it is on the screen.
  await page.keyboard.press("ControlOrMeta+k");
  await t("switcher-menu").waitFor({ timeout: 10_000 });
  await expectText(t("switcher-hidden"), "Hidden", "the switcher has its Hidden group");
  await t("switcher-filter").fill("HPB");
  await t("switcher-item-HPB").waitFor({ timeout: 10_000 });
  await page.keyboard.press("Enter");
  await expectPath("/projects/HPB", "the switcher opens the hidden project");
  await listed("HPB").locator("[data-testid=hidden-mark]").waitFor({ timeout: 10_000 });
  if ((await listed("HPB").getAttribute("aria-expanded")) !== "true") throw new Error("HPB's tree is not open");
  await listed("HPB").locator("[data-testid=roadmap-link]").waitFor({ timeout: 10_000 });
  console.log("PASS the opened hidden project shows its tree with the hidden mark");
  await shoot(page, "hide-project-on-screen", page.locator("nav[aria-label=Sidebar]"));
  await listed("HPA").locator("a").first().click();
  await expectPath("/projects/HPA", "the person goes elsewhere");
  await gone(listed("HPB"), "the hidden project leaves the list again");

  // Show in list: the button of the hidden group's row.
  await t("show-hidden").click();
  await inHidden("HPB").waitFor({ timeout: 10_000 });
  await shoot(page, "hide-project-open", page.locator("nav[aria-label=Sidebar]"));
  await t("show-in-list-HPB").click();
  await listed("HPB").waitFor({ timeout: 10_000 });
  if (hiddenFlag("HPB")) throw new Error("the daemon still says HPB is hidden");
  await gone(t("show-hidden"), "Show in list brings it back, and the hidden row goes with the last one");

  // Show in list: the right-click menu, on HPC hidden from the menu too.
  await listed("HPC").locator("> .row").click({ button: "right" });
  await t("menu-hide-project").click();
  await gone(listed("HPC"), "HPC leaves the list");
  await inHidden("HPC").locator("> .row").click({ button: "right" });
  await expectText(t("menu-show-project"), "Show in list", "a hidden row's menu offers Show in list");
  await t("menu-show-project").click();
  await listed("HPC").waitFor({ timeout: 10_000 });
  console.log("PASS the right-click menu brings a project back to the list");
  if (status("HPB-1") !== before) throw new Error("the story's status moved during the test");
  if (pageError) throw new Error(`page error: ${pageError}`);
  console.log("web hide-project: PASS");
} catch (e) {
  console.error(`web hide-project: FAIL: ${e.message}`);
  process.exitCode = 1;
} finally {
  await browser?.close();
  await cleanup(tok, home);
}
