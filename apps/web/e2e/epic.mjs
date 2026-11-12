// Browser test of the epic and the release against a real daemon.
// Usage: node e2e/epic.mjs <path to tok binary>   (cargo xtask e2e-web runs it)
//
// A chain is created in the window with a version (the dialog suggests one first). The Start node
// shows the chain's name and goal as the epic, the Finish node reads Release v0.3.0 planned, and
// the sidebar row and the header carry the version. Opening Start edits the epic, opening Finish
// edits the version; a taken or malformed one is refused as the daemon said it.

import { execFileSync } from "node:child_process";
import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { checks, cleanup, launch } from "./helpers.mjs";

const tokBin = process.argv[2];
if (!tokBin) {
  console.error("usage: node e2e/epic.mjs <tok binary>");
  process.exit(2);
}
const home = mkdtempSync(join(tmpdir(), "toknila-epic-"));
const tok = (...args) =>
  execFileSync(tokBin, args, { env: { ...process.env, TOKNILA_HOME: home }, encoding: "utf8" });

let browser;
try {
  tok("project", "new", "EPC", "--sim");
  const { url } = JSON.parse(tok("open", "--json"));
  browser = await launch();
  const page = await browser.newPage({ viewport: { width: 1280, height: 800 } });
  const errors = [];
  page.on("pageerror", (e) => errors.push(e.message));
  const { expectText } = checks(page);
  const t = (id) => page.locator(`[data-testid=${id}]`);

  await page.goto(url.replace("/?", "/projects/EPC?"));
  await t("new-chain-plus").click();
  await page.waitForFunction(() => document.querySelector("[data-testid=chain-version]")?.value === "0.1.0", null, { timeout: 10_000 }).catch(() => {
    throw new Error("the dialog did not suggest 0.1.0");
  });
  console.log("PASS the New chain dialog suggests a version: 0.1.0");
  await t("chain-name").fill("checkout");
  await t("chain-goal").fill("Let customers pay with saved cards.\n\n- wallets\n- the old flow behind a flag");
  await t("chain-version").fill("0.3.0");
  await t("chain-autostart").uncheck();
  await t("create-chain").click();

  await t("start-node").waitFor({ timeout: 10_000 });
  await expectText(t("epic-title"), "checkout", "Start shows the chain's name as the epic title");
  await expectText(t("epic-goal"), "Let customers pay with saved cards.", "Start shows the goal as the description");
  await expectText(t("finish-title"), "Release v0.3.0", "Finish reads Release v0.3.0");
  await expectText(t("finish-state"), "planned", "Finish is planned while nothing is merged");
  await expectText(t("chain-version-top"), "v0.3.0", "the chain header shows the version");
  await expectText(page.locator("[data-testid=chains] li[data-chain-item=checkout]"), "v0.3.0", "the sidebar row shows the version after the name");

  // An empty chain shows a prompt over the diagram; with a card the nodes are all in reach.
  tok("ticket", "new", "checkout", "Saved cards");
  await t("title").first().waitFor({ timeout: 10_000 });
  await expectText(t("end-node-text"), "1 left", "the Finish node counts the card that is open");

  // Opening Start edits the epic.
  await t("start-node").click();
  await t("epic-panel").waitFor();
  await t("epic-goal-input").fill("Pay with saved cards, wallets and vouchers.");
  await t("epic-save").click();
  await expectText(t("epic-goal"), "wallets and vouchers", "the saved goal is on the Start node");
  console.log("PASS opening Start edits the epic");

  // Opening Finish edits the version while nothing is released.
  await t("end-node").click();
  await t("epic-version").waitFor();
  await t("epic-version").fill("soon");
  await t("epic-save").click();
  await expectText(t("epic-error"), "numbers separated by dots", "a malformed version is refused as the daemon said it");
  await t("epic-version").fill("0.4.0");
  await t("epic-save").click();
  await expectText(t("finish-title"), "Release v0.4.0", "the edited version is on the Finish node");
  const shown = JSON.parse(tok("chain", "show", "checkout", "--json"));
  if (shown.summary.version !== "0.4.0" || shown.releases[0].version !== "0.4.0") throw new Error("the daemon did not take the version");
  console.log("PASS the daemon holds the edited version");

  if (errors.length) throw new Error(`the page threw: ${errors.join("; ")}`);
  console.log("web epic: PASS");
} catch (e) {
  console.error(`web epic: FAIL: ${e.message.slice(0, 800)}`);
  process.exitCode = 1;
} finally {
  await browser?.close();
  await cleanup(tok, home);
}
