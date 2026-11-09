// Browser test of the chain's autostart toggle against a real daemon.
// Usage: node e2e/autostart.mjs <path to tok binary>   (cargo xtask e2e-web runs it)
//
// A chain without autostart holds a ready card. The right-click menu of the chain in the sidebar
// turns autostart on: the toggle in the chain's header follows, and the ready card starts (the
// hint says it will). The header toggle turns it off again; the daemon agrees each time.

import { execFileSync } from "node:child_process";
import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { checks, cleanup, launch } from "./helpers.mjs";

const tokBin = process.argv[2];
if (!tokBin) {
  console.error("usage: node e2e/autostart.mjs <tok binary>");
  process.exit(2);
}
const home = mkdtempSync(join(tmpdir(), "toknila-autostart-"));
const tok = (...args) =>
  execFileSync(tokBin, args, { env: { ...process.env, TOKNILA_HOME: home }, encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] });
const daemonSays = () => JSON.parse(tok("chain", "show", "work", "--json")).chain.autostart;

let browser;
try {
  tok("project", "new", "AUT");
  tok("chain", "new", "work", "--project", "AUT", "--no-autostart");
  tok("ticket", "new", "work", "Alpha");
  const { url } = JSON.parse(tok("open", "--json"));

  browser = await launch();
  const page = await browser.newPage();
  const errors = [];
  page.on("pageerror", (e) => errors.push(e.message));
  const { expectText } = checks(page);
  const status = page.locator('a[data-key="AUT-1"] [data-testid=status]');
  const toggle = page.locator("[data-testid=chain-autostart-toggle]");
  const row = page.locator('[data-chain-item="work"]');

  await page.goto(url.replace("/?", "/projects/AUT/chains/work?"));
  await expectText(status, "ready", "the card is ready and nothing starts it");
  if (await toggle.isChecked()) throw new Error("the toggle shows autostart on for a chain made without it");
  console.log("PASS the header toggle shows autostart off");
  await expectText(page.locator(".autostart"), "Start cards automatically", "the header toggle is worded like the menu");

  // The sidebar menu: its entry offers to turn it on, and the hint says it may start cards.
  await row.click({ button: "right" });
  const entry = page.locator("[data-testid=menu-autostart]");
  await expectText(entry, "Start cards automatically", "the chain menu offers autostart");
  const hint = await entry.getAttribute("title");
  if (!hint || !/start/.test(hint) || !/ready/.test(hint)) throw new Error(`the hint does not say it starts ready cards: ${hint}`);
  console.log(`PASS the hint says what it does: ${hint}`);
  await entry.click();
  await expectText(status, "running", "turning autostart on started the ready card");
  await page.waitForFunction(() => document.querySelector("[data-testid=chain-autostart-toggle]").checked, null, { timeout: 10_000 });
  console.log("PASS the header toggle follows: on");
  if (daemonSays() !== true) throw new Error("the daemon does not have autostart on");
  console.log("PASS the daemon has autostart on");

  // The header toggle turns it off; the menu then offers to turn it on again.
  await toggle.click();
  await page.waitForFunction(() => !document.querySelector("[data-testid=chain-autostart-toggle]").checked, null, { timeout: 10_000 });
  if (daemonSays() !== false) throw new Error("the daemon still has autostart on");
  console.log("PASS the header toggle turned autostart off in the daemon");
  await row.click({ button: "right" });
  await expectText(page.locator("[data-testid=menu-autostart]"), "Start cards automatically", "after off the menu offers on again");
  await page.keyboard.press("Escape");

  // And the menu turns it off when it is on.
  await toggle.click();
  await page.waitForFunction(() => document.querySelector("[data-testid=chain-autostart-toggle]").checked, null, { timeout: 10_000 });
  await row.click({ button: "right" });
  await expectText(page.locator("[data-testid=menu-autostart]"), "Stop starting cards automatically", "with autostart on the menu offers off");
  await page.locator("[data-testid=menu-autostart]").click();
  await page.waitForFunction(() => !document.querySelector("[data-testid=chain-autostart-toggle]").checked, null, { timeout: 10_000 });
  if (daemonSays() !== false) throw new Error("the menu did not turn autostart off");
  console.log("PASS the menu turned autostart off");

  if (errors.length) throw new Error(`the page threw: ${errors.join("; ")}`);
  console.log("web autostart: PASS");
} catch (e) {
  console.error(`web autostart: FAIL: ${e.message.slice(0, 600)}`);
  process.exitCode = 1;
} finally {
  await browser?.close();
  await cleanup(tok, home);
}
