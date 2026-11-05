// Browser smoke test of the IDE-layout page against a real daemon (P0-14, P1-02).
// Usage: node e2e/smoke.mjs <path to tok binary>   (cargo xtask e2e-web runs it)
//
// It starts a daemon in a private TOKNILA_HOME, imports examples/demo.toml, opens the page and
// checks: the sidebar leads to a chain, the canvas draws its cards and milestone, a card opens
// its Details and Events tabs, a change made from the CLI appears live, the card and inbox deep
// links and the title link navigate, and the page reconnects by itself after the daemon restarts.

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
  const at = (path) => url.replace("/?", `${path}?`);

  browser = await chromium.launch(
    process.env.PW_CHROMIUM ? { executablePath: process.env.PW_CHROMIUM } : {},
  );
  const page = await browser.newPage();
  page.on("pageerror", (e) => {
    console.error(`page error: ${e.message}`);
    failed = true;
  });
  await page.goto(url);
  const status = (key) => page.locator(`a[data-key="${key}"] [data-testid=status]`);
  const expectText = async (loc, text, what) => {
    await loc.filter({ hasText: text }).first().waitFor({ timeout: 10_000 }).catch(async () => {
      throw new Error(`${what}: expected "${text}", got "${await loc.first().textContent().catch(() => "?")}"`);
    });
    console.log(`PASS ${what}: ${text}`);
  };
  const expectPath = async (path, what) => {
    await page.waitForFunction((p) => window.location.pathname === p, path, { timeout: 10_000 }).catch(() => {
      throw new Error(`${what}: expected path ${path}, at ${new URL(page.url()).pathname}`);
    });
    console.log(`PASS ${what}: ${path}`);
  };
  const openChain = async () => {
    await page.locator("[data-testid=chains]").getByRole("link", { name: "demo" }).click();
    await expectPath("/projects/DEMO/chains/demo", "chain route");
  };

  // Sidebar: the project, then its chain; the canvas draws the chain.
  await expectText(page.locator("[data-testid=link]"), "live", "stream connected");
  await expectText(page.locator("[data-testid=projects]"), "DEMO", "project listed");
  await openChain();
  await expectText(status("DEMO-1"), "running", "DEMO-1 autostarted");
  await expectText(status("DEMO-2"), "backlog", "DEMO-2 waits for DEMO-1");
  await expectText(page.locator('[data-version="R1"]'), "planned", "milestone R1 is a node");
  await expectText(page.locator("[data-testid=running]"), "DEMO-1", "running agents list");
  const nodes = await page.locator(".canvas .node").count();
  if (nodes !== 5) throw new Error(`canvas: expected 5 nodes, got ${nodes}`);
  console.log("PASS canvas: 4 cards and 1 milestone");

  // Open a card: the Details tab shows the ticket.
  await page.locator('a[data-key="DEMO-1"]').click();
  await expectPath("/projects/DEMO/chains/demo/cards/DEMO-1", "card route");
  await expectText(page.locator("[data-testid=details]"), "First step.", "Details tab shows the prompt");
  await expectText(page.locator("[data-testid=details]"), "write the test", "Details tab shows subtasks");

  // A change made from the CLI shows live, in the canvas, the inbox and the Details tab.
  tok("sim", "DEMO-1", "ask", "which way?");
  await expectText(status("DEMO-1"), "waiting", "live update from the CLI");
  await expectText(page.locator("[data-testid=details]"), "question: which way?", "reason shown");
  await expectText(page.locator("[data-testid=chain-status]"), "needs human", "chain needs a human");
  await expectText(page.locator("[data-testid=inbox-count]"), "1", "inbox count");

  // Events tab: the card's journal, then a new line arrives from the stream.
  await page.getByRole("tab", { name: "Events" }).click();
  await expectText(page.locator("[data-testid=events]"), "ticket.", "Events tab lists the journal");
  const before = await page.locator("[data-testid=events] li").count();
  tok("comment", "DEMO-1", "live note");
  await page.waitForFunction(
    (n) => document.querySelectorAll("[data-testid=events] li").length > n,
    before,
    { timeout: 10_000 },
  );
  console.log("PASS Events tab: a new event arrived live");
  await page.getByRole("tab", { name: "Terminal" }).click();
  await expectText(page.locator("[data-testid=placeholder]"), "terminal", "Terminal tab is a placeholder");

  // The comment answered the question, so the card runs again; ask once more for the inbox.
  await page.getByRole("tab", { name: "Details" }).click();
  await expectText(status("DEMO-1"), "running", "a comment answers the question");
  tok("sim", "DEMO-1", "ask", "and now?");
  await expectText(status("DEMO-1"), "waiting", "asked again");

  // Navigation lives in the URL: deep links open the card and the inbox, the title leads back.
  await page.goto(at("/projects/DEMO/chains/demo/cards/DEMO-2"));
  await expectText(page.locator("[data-testid=details]"), "DEMO-2", "card deep link loads");
  await page.goto(at("/inbox"));
  await expectText(page.locator("[data-testid=link]"), "live", "inbox deep link loads");
  await page.locator('[data-testid=inbox] li[data-key="DEMO-1"]').waitFor({ timeout: 10_000 });
  const inbox = await page.locator("[data-testid=inbox] li").count();
  if (inbox !== 1) throw new Error(`inbox: expected 1 ticket, got ${inbox}`);
  console.log("PASS inbox route: 1 ticket");
  await page.getByRole("link", { name: "Toknila" }).click();
  await expectPath("/", "title link");

  tok("daemon", "stop");
  await expectText(page.locator("[data-testid=link]"), "offline", "daemon stop seen");
  tok("daemon", "start");
  tok("comment", "DEMO-2", "left");
  await expectText(page.locator("[data-testid=link]"), "live", "reconnected after restart");
  await openChain();
  await expectText(status("DEMO-1"), "waiting", "state is still there after the restart");
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
