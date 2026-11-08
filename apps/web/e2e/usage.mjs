// Browser test of the Usage page against a real daemon.
// Usage: node e2e/usage.mjs <path to tok binary>   (cargo xtask e2e-web runs it)
//
// Synthetic Claude Code transcripts (invented ids and numbers, no message content) are written
// under a temporary CLAUDE_CONFIG_DIR, so the test never reads the real ~/.claude. One session is
// tied to a card the way a hook does it (the activity route with session_id and transcript_path);
// the other is a session on the machine that is not a Toknila run. The page must show totals, a
// chart, breakdowns, the card's chip, and follow the window, the metric and the machine-wide switch.

import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { checks, cleanup, launch } from "./helpers.mjs";

const tokBin = process.argv[2];
if (!tokBin) {
  console.error("usage: node e2e/usage.mjs <tok binary>");
  process.exit(2);
}
const home = mkdtempSync(join(tmpdir(), "toknila-usage-"));
const claude = mkdtempSync(join(tmpdir(), "toknila-claude-"));
const demo = resolve(import.meta.dirname, "../../../examples/demo.toml");
const env = { ...process.env, TOKNILA_HOME: home, CLAUDE_CONFIG_DIR: claude };
const tok = (...args) => execFileSync(tokBin, args, { env, encoding: "utf8" });

const now = new Date().toISOString();
/// One assistant line of a transcript: [input, cache write, cache read, output].
const call = (id, model, cwd, t) =>
  JSON.stringify({
    type: "assistant", uuid: `u-${id}`, requestId: `r-${id}`, timestamp: now, cwd,
    message: {
      id, model,
      usage: { input_tokens: t[0], cache_creation_input_tokens: t[1], cache_read_input_tokens: t[2], output_tokens: t[3] },
    },
  }) + "\n";
const transcript = (project, session, lines) => {
  const dir = join(claude, "projects", project);
  mkdirSync(dir, { recursive: true });
  const path = join(dir, `${session}.jsonl`);
  writeFileSync(path, lines);
  return path;
};

let browser;
let failed = false;
try {
  tok("project", "new", "DEMO");
  tok("chain", "import", demo, "--project", "DEMO");
  // The card's session writes one call twice (streaming); it counts once.
  const card = transcript("-work-demo", "sess-card",
    call("m1", "claude-opus-4-8", "/work/demo", [1_000_000, 0, 0, 10]) +
    call("m1", "claude-opus-4-8", "/work/demo", [1_000_000, 0, 0, 100_000]) +
    call("m2", "claude-opus-4-8", "/work/demo", [0, 0, 2_000_000, 0]));
  transcript("-work-other", "sess-other", call("m3", "claude-haiku-4-5", "/work/other", [1_000_000, 0, 0, 0]));

  const { url } = JSON.parse(tok("open", "--json"));
  const base = new URL(url);
  const bearer = base.searchParams.get("token");
  const hook = await fetch(`${base.origin}/v1/tickets/DEMO-1/activity`, {
    method: "POST",
    headers: { Authorization: `Bearer ${bearer}`, "Content-Type": "application/json" },
    body: JSON.stringify({ signal: "hook:stop", session_id: "sess-card", transcript_path: card }),
  });
  if (hook.status !== 204) throw new Error(`the hook was refused: ${hook.status} ${await hook.text()}`);

  browser = await launch();
  const page = await browser.newPage();
  page.on("pageerror", (e) => {
    console.error(`page error: ${e.message}`);
    failed = true;
  });
  const { expectText } = checks(page);
  const t = (id) => page.locator(`[data-testid=${id}]`);
  const at = (path) => url.replace("/?", `${path}?`);

  await page.goto(at("/usage"));
  await t("usage").waitFor({ timeout: 10_000 });
  await expectText(t("usage-processed"), "4.1M", "tokens processed: the card's two calls and the other session");
  await expectText(t("usage-cached"), "50%", "cached share");
  await expectText(t("usage-uncached"), "2M", "uncached input");
  await expectText(t("usage-output"), "100k", "output");
  await expectText(t("usage-cost"), "$9.50", "estimated cost");
  await expectText(t("usage-savings"), "$9.00", "estimated cache savings");
  await expectText(t("usage-estimate"), "subscription", "the estimate is labelled");
  await expectText(t("usage-prices"), "platform.claude.com", "the price source is shown");
  await expectText(t("quota-unavailable"), "Not available", "no quota reported: it says so");

  const chart = t("usage-chart");
  if ((await chart.locator("g[data-day]").count()) !== 7) throw new Error("the 7-day chart should have 7 bars");
  console.log("PASS the chart has one entry per day (7)");
  await chart.locator("g[data-day]").last().locator("rect.hit").hover();
  await expectText(t("usage-readout"), "$9.50", "hovering the last day reads its cost");

  const row = (table, label) => t(`usage-by-${table}`).locator(`tr[data-label="${label}"]`);
  await expectText(row("model", "claude-opus-4-8"), "$8.50", "model breakdown row");
  await expectText(row("workspace", "/work/other"), "$1.00", "workspace breakdown row");
  await expectText(row("card", "DEMO-1"), "$8.50", "card breakdown row");
  await expectText(row("project", "DEMO"), "$8.50", "project breakdown row");
  await expectText(row("chain", "demo"), "$8.50", "chain breakdown row");

  // The metric toggle changes what the chart reads.
  await t("usage-metric").getByText("Tokens").click();
  await chart.locator("g[data-day]").last().locator("rect.hit").hover();
  await expectText(t("usage-readout"), "4.1M tok", "the tokens metric reads tokens");
  await t("usage-metric").getByText("Cost").click();

  // The window switch reloads the page for 30 and 90 days.
  for (const days of [30, 90]) {
    await t("usage-days-switch").getByText(`${days}d`).click();
    await page.waitForFunction(
      (n) => document.querySelectorAll("[data-testid=usage-chart] g[data-day]").length === n,
      days,
      { timeout: 10_000 },
    );
    console.log(`PASS the ${days}d window shows ${days} days`);
  }
  await expectText(t("usage-processed"), "4.1M", "totals still hold in the longer window");

  // The card carries a chip, on the canvas and in its details.
  await page.goto(at("/projects/DEMO/chains/demo/cards/DEMO-1"));
  await expectText(page.locator("a.node[data-key=DEMO-1] [data-testid=usage-chip]"), "3.1M tok · ~$8.50", "the card's chip on the canvas");
  await expectText(t("details-usage"), "~$8.50", "the card's chip in its details");

  // Only Toknila's runs: the other session disappears.
  await page.goto(at("/usage"));
  // The box follows the daemon's answer, so it changes after the save, not at the click.
  await t("usage-machine-scan").click();
  await expectText(t("usage-processed"), "3.1M", "without the machine-wide view only the card is counted");
  await row("workspace", "/work/other").waitFor({ state: "detached", timeout: 10_000 });
  console.log("PASS the other session is no longer listed");
  if (failed) throw new Error("the page threw errors");
  console.log("usage: PASS");
} catch (e) {
  console.error(`usage: FAIL: ${e.message}`);
  process.exitCode = 1;
} finally {
  await browser?.close();
  await cleanup(tok, home);
}
