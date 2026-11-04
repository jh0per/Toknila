// Browser smoke test of the read-only page against a real daemon (P0-14).
// Usage: node e2e/smoke.mjs <path to tok binary>   (cargo xtask e2e-web runs it)
//
// It starts a daemon in a private TOKNILA_HOME, imports examples/demo.toml, opens the page and
// checks: statuses render, a change made from the CLI appears live, the "needs a human" filter
// works, the inbox deep link and the title link navigate, and the page reconnects by itself
// after the daemon restarts.

import { execFileSync } from "node:child_process";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { chromium } from "playwright";

const tokBin = process.argv[2];
if (!tokBin) {
  console.error("usage: node e2e/smoke.mjs <tok binary>");
  process.exit(2);
}
const home = mkdtempSync(join(tmpdir(), "toknila-web-"));
const demo = resolve(import.meta.dirname, "../../../examples/demo.toml");
const tok = (...args) =>
  execFileSync(tokBin, args, { env: { ...process.env, TOKNILA_HOME: home }, encoding: "utf8" });

let browser;
let failed = false;
try {
  tok("project", "new", "DEMO");
  tok("chain", "import", demo, "--project", "DEMO");
  const { url } = JSON.parse(tok("open", "--json"));
  console.log(`page: ${url.replace(/token=.*/, "token=…")}`);

  browser = await chromium.launch(
    process.env.PW_CHROMIUM ? { executablePath: process.env.PW_CHROMIUM } : {},
  );
  const page = await browser.newPage();
  page.on("pageerror", (e) => {
    console.error(`page error: ${e.message}`);
    failed = true;
  });
  await page.goto(url);
  const status = (key) => page.locator(`li[data-key="${key}"] [data-testid=status]`);
  const expectText = async (loc, text, what) => {
    await loc.filter({ hasText: text }).first().waitFor({ timeout: 10_000 }).catch(async () => {
      throw new Error(`${what}: expected "${text}", got "${await loc.first().textContent().catch(() => "?")}"`);
    });
    console.log(`PASS ${what}: ${text}`);
  };

  await expectText(page.locator("[data-testid=link]"), "live", "stream connected");
  await expectText(status("DEMO-1"), "running", "DEMO-1 autostarted");
  await expectText(status("DEMO-2"), "backlog", "DEMO-2 waits for DEMO-1");
  await expectText(page.locator('li[data-version="R1"]'), "planned", "milestone R1 shown");

  tok("sim", "DEMO-1", "ask", "which way?");
  await expectText(status("DEMO-1"), "waiting", "live update from the CLI");
  await expectText(page.locator('li[data-key="DEMO-1"] .reason'), "question: which way?", "reason shown");
  await expectText(page.locator("[data-testid=chain-status]"), "needs human", "chain needs a human");

  await page.getByLabel("needs a human").check();
  const visible = await page.locator("li.ticket").count();
  if (visible !== 1) throw new Error(`filter: expected 1 ticket, got ${visible}`);
  console.log("PASS needs-a-human filter: 1 ticket");
  await page.getByLabel("needs a human").uncheck();

  // Navigation lives in the URL: a deep link opens the inbox, the title leads back.
  await page.goto(url.replace("/?", "/inbox?"));
  await expectText(page.locator("[data-testid=link]"), "live", "inbox deep link loads");
  await page.locator('li[data-key="DEMO-1"]').waitFor({ timeout: 10_000 });
  const inbox = await page.locator("li.ticket").count();
  if (inbox !== 1) throw new Error(`inbox: expected 1 ticket, got ${inbox}`);
  console.log("PASS inbox route: 1 ticket");
  await page.getByRole("link", { name: "Toknila" }).click();
  await page.locator('li[data-key="DEMO-2"]').waitFor({ timeout: 10_000 });
  if (new URL(page.url()).pathname !== "/") throw new Error(`back home: at ${page.url()}`);
  console.log("PASS title link: back to all chains");

  tok("daemon", "stop");
  await expectText(page.locator("[data-testid=link]"), "offline", "daemon stop seen");
  tok("daemon", "start");
  tok("comment", "DEMO-1", "left");
  await expectText(page.locator("[data-testid=link]"), "live", "reconnected after restart");
  await expectText(status("DEMO-1"), "running", "change made while away is shown");
  if (failed) throw new Error("the page threw errors");
  console.log("web smoke: PASS");
} catch (e) {
  console.error(`web smoke: FAIL: ${e.message}`);
  process.exitCode = 1;
} finally {
  await browser?.close();
  try {
    tok("daemon", "stop");
  } catch {
    /* already stopped */
  }
  rmSync(home, { recursive: true, force: true });
}
