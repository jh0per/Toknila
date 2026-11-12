// Browser test of deleting a chain from the sidebar against a real daemon.
// Usage: node e2e/chain-delete.mjs <path to tok binary>   (cargo xtask e2e-web runs it)
//
// A chain imported by mistake (all its cards cancelled) is deleted through the chain's context
// menu: the dialog names the chain and its cards, Cancel changes nothing, Delete removes the
// chain and only it. A chain with a running card is refused with the daemon's message that names
// the card. SCREENSHOT_DIR, when set, receives pictures of the dialog.

import { execFileSync } from "node:child_process";
import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { checks, cleanup, launch } from "./helpers.mjs";

const tokBin = process.argv[2];
if (!tokBin) {
  console.error("usage: node e2e/chain-delete.mjs <tok binary>");
  process.exit(2);
}
const home = mkdtempSync(join(tmpdir(), "toknila-chaindel-"));
const tok = (...args) =>
  execFileSync(tokBin, args, { env: { ...process.env, TOKNILA_HOME: home }, encoding: "utf8" });

let browser;
try {
  tok("project", "new", "CHD", "--sim");
  tok("chain", "new", "phase-1", "--project", "CHD", "--no-autostart");
  tok("ticket", "new", "phase-1", "Old one");
  tok("ticket", "new", "phase-1", "Old two");
  tok("cancel", "CHD-1", "imported by mistake");
  tok("cancel", "CHD-2", "imported by mistake");
  tok("chain", "new", "live", "--project", "CHD", "--no-autostart");
  tok("ticket", "new", "live", "Busy card");
  tok("start", "CHD-3");
  tok("chain", "new", "keep", "--project", "CHD", "--no-autostart");
  tok("ticket", "new", "keep", "Stays");
  const { url } = JSON.parse(tok("open", "--json"));

  browser = await launch();
  const page = await browser.newPage({ viewport: { width: 1280, height: 800 } });
  const errors = [];
  page.on("pageerror", (e) => errors.push(e.message));
  const { expectText } = checks(page);
  const shot = async (name) => {
    if (process.env.SCREENSHOT_DIR) await page.screenshot({ path: join(process.env.SCREENSHOT_DIR, `${name}.png`) });
  };
  const item = (name) => page.locator(`[data-testid=chains] li[data-chain-item="${name}"]`);
  const chains = () => JSON.parse(tok("chain", "ls", "--json")).map((c) => c.chain.name).sort();

  await page.goto(url.replace("/?", "/projects/CHD/chains/phase-1?"));
  await expectText(page.locator("[data-testid=chain-name-top]"), "phase-1", "the mistaken chain is open");

  // The dialog names what goes; Cancel changes nothing.
  await item("phase-1").click({ button: "right" });
  await page.locator("[data-testid=menu-delete-chain]").click();
  const what = page.locator("[data-testid=delete-chain-what]");
  await expectText(what, "phase-1", "the dialog names the chain");
  await expectText(what, "2 cards", "the dialog says how many cards go");
  await shot("delete-chain-dialog");
  await page.locator("[data-testid=delete-chain-cancel]").click();
  await page.locator("[data-testid=delete-chain-dialog]").waitFor({ state: "detached" });
  if (chains().length !== 3) throw new Error("Cancel deleted something");
  console.log("PASS Cancel closes the dialog and deletes nothing");

  // A chain with a running card is refused, naming the card.
  await item("live").click({ button: "right" });
  await page.locator("[data-testid=menu-delete-chain]").click();
  await expectText(page.locator("[data-testid=delete-chain-what]"), "1 card", "the dialog says one card");
  await page.locator("[data-testid=delete-chain-confirm]").click();
  await expectText(
    page.locator("[data-testid=delete-chain-error]"),
    "chain live has agents working on CHD-3; stop them before deleting the chain",
    "the refusal names the running card",
  );
  await shot("delete-chain-refused");
  await page.locator("[data-testid=delete-chain-cancel]").click();
  if (chains().length !== 3) throw new Error("a refused deletion deleted something");

  // The mistaken chain goes, and only it.
  await item("phase-1").click({ button: "right" });
  await page.locator("[data-testid=menu-delete-chain]").click();
  await page.locator("[data-testid=delete-chain-confirm]").click();
  await expectText(page.locator("[data-testid=delete-chain-result]"), "phase-1 was deleted", "the result says the chain was deleted");
  await expectText(page.locator("[data-testid=delete-chain-result]"), "2 cards deleted", "the result counts the cards");
  await shot("delete-chain-done");
  await page.locator("[data-testid=delete-chain-done]").click();
  await item("phase-1").waitFor({ state: "detached" });
  const left = chains().join(",");
  if (left !== "keep,live") throw new Error(`expected keep,live to stay, got ${left}`);
  console.log("PASS the chain is gone from the sidebar and the daemon, the others stay: keep,live");
  await page.waitForURL((u) => !u.pathname.includes("phase-1"));
  console.log("PASS the window left the deleted chain");

  if (errors.length) throw new Error(`the page threw: ${errors.join("; ")}`);
  console.log("web chain delete: PASS");
} catch (e) {
  console.error(`web chain delete: FAIL: ${e.message.slice(0, 800)}`);
  process.exitCode = 1;
} finally {
  await browser?.close();
  await cleanup(tok, home);
}
