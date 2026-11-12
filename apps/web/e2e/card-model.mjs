// Browser test of a card's own model against a real daemon.
// Usage: node e2e/card-model.mjs <path to tok binary>   (cargo xtask e2e-web runs it)
//
// The New card dialog offers a model (the aliases, "The project's model" and another name). A card
// made with opus shows it in Details; while the card can be edited a small editor changes it to
// another alias, to a full name and back to the project's, and `tok ticket show` says what was
// kept. A card made with `tok ticket new --model` shows its model in the window too.

import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { checks, cleanup, launch } from "./helpers.mjs";

const tokBin = process.argv[2];
if (!tokBin) {
  console.error("usage: node e2e/card-model.mjs <tok binary>");
  process.exit(2);
}
const home = mkdtempSync(join(tmpdir(), "toknila-card-model-"));
const tok = (...args) =>
  execFileSync(tokBin, args, { env: { ...process.env, TOKNILA_HOME: home }, encoding: "utf8" });
const git = (dir, ...args) =>
  execFileSync("git", ["-C", dir, "-c", "user.name=t", "-c", "user.email=t@example.com", "-c", "commit.gpgsign=false", ...args], { encoding: "utf8" });

const repo = join(home, "work", "repo");
mkdirSync(repo, { recursive: true });
git(repo, "init", "--quiet");
git(repo, "symbolic-ref", "HEAD", "refs/heads/develop");
git(repo, "commit", "--quiet", "--allow-empty", "-m", "init");

let browser;
try {
  tok("project", "new", "DEMO", "--repo", repo);
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
  const selected = async (id, expected, what) => {
    await page.waitForFunction(
      ([sel, v]) => document.querySelector(sel)?.value === v,
      [`[data-testid=${id}]`, expected],
      { timeout: 10_000 },
    ).catch(async () => {
      throw new Error(`${what}: expected "${expected}", got "${await t(id).inputValue().catch(() => "?")}"`);
    });
    console.log(`PASS ${what}: ${expected}`);
  };
  const shown = () => tok("ticket", "show", "DEMO-1");

  // A chain that waits for the person to start its cards, so a card stays editable.
  await page.goto(at("/projects/DEMO"));
  await t("empty-new-chain").click();
  await t("chain-name").fill("plan");
  await t("chain-autostart").uncheck();
  await t("create-chain").click();
  await expectPath("/projects/DEMO/chains/plan", "the chain opens");

  // The New card dialog offers the model.
  await t("new-card").click();
  await t("card-prompt").fill("Design the thing\nThink it through.");
  const offered = await t("card-model").locator("option").evaluateAll((os) => os.map((o) => o.value));
  if (!["", "sonnet", "opus", "haiku", "fable"].every((a) => offered.includes(a))) throw new Error(`models offered: ${offered}`);
  await selected("card-model", "", "a new card starts on the project's model");
  await t("card-model").selectOption("opus");
  await t("create-card").click();
  await expectPath("/projects/DEMO/chains/plan/cards/DEMO-1", "the card is added and opens");

  // Details shows it, and the editor changes it while the card has not started.
  await selected("card-model-select", "opus", "Details shows the card's own model");
  if (!shown().includes("model: opus")) throw new Error(`tok ticket show: ${shown()}`);
  console.log("PASS the daemon kept opus");
  await t("card-model-select").selectOption("fable");
  await t("save-card-model").click();
  await page.waitForFunction(() => !document.querySelector("[data-testid=save-card-model]"), null, { timeout: 10_000 });
  await selected("card-model-select", "fable", "the new model is shown after the save");
  if (!shown().includes("model: fable")) throw new Error(`tok ticket show: ${shown()}`);
  console.log("PASS fable was saved");
  await t("card-model-select").selectOption("__another__");
  await t("card-model-select-custom").fill("claude-opus-5-5");
  await t("save-card-model").click();
  await page.waitForFunction(() => !document.querySelector("[data-testid=save-card-model]"), null, { timeout: 10_000 });
  if (!shown().includes("model: claude-opus-5-5")) throw new Error(`tok ticket show: ${shown()}`);
  console.log("PASS a full model name was saved");
  await t("card-model-select").selectOption("");
  await t("save-card-model").click();
  await page.waitForFunction(() => !document.querySelector("[data-testid=save-card-model]"), null, { timeout: 10_000 });
  if (shown().includes("model:")) throw new Error(`the model should be cleared: ${shown()}`);
  console.log("PASS the project's model applies again");

  // A card made from the terminal shows its model in the window.
  tok("ticket", "new", "plan", "Second", "--model", "haiku");
  await page.goto(at("/projects/DEMO/chains/plan/cards/DEMO-2"));
  await selected("card-model-select", "haiku", "a card made with tok ticket new --model shows it");
  await expectText(t("details"), "Second", "the card is the one made from the terminal");
  if (pageError) throw new Error(`page error: ${pageError}`);
  console.log("web card model: PASS");
} catch (e) {
  console.error(`web card model: FAIL: ${e.message}`);
  process.exitCode = 1;
} finally {
  await browser?.close();
  await cleanup(tok, home);
}
