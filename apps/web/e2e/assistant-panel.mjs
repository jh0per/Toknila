// Browser test of the assistant panel beside the work, against a real daemon and the workspace's
// `fake-agent` (never a real agent). The fake planner only echoes the lines typed into its terminal,
// so the test reads what the assistant was told.
// Usage: node e2e/assistant-panel.mjs <tok binary> <fake-agent binary>   (cargo xtask e2e-web runs it)
// needs: fake-agent
//
// Opens and closes from the top bar and the shortcut on the roadmap, a chain and a card, and keeps
// its state over a reload; a message with the chain chip reaches the assistant as `About chain
// NAME: ...` and is stored with the chain, one with the roadmap chip as `About the roadmap: ...`,
// and one with the chip taken off as plain words; "Only about this" keeps the open chain's messages;
// under 1100 px the panel overlays the screen and Escape closes it.

import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { checks, cleanup, launch } from "./helpers.mjs";

const [tokArg, agentArg] = process.argv.slice(2);
if (!tokArg || !agentArg) {
  console.error("usage: node e2e/assistant-panel.mjs <tok binary> <fake-agent binary>");
  process.exit(2);
}
const tokBin = resolve(tokArg);
const fakeAgent = resolve(agentArg);
const home = mkdtempSync(join(tmpdir(), "toknila-panel-"));
const run = (cmd, args) => execFileSync(cmd, args, { encoding: "utf8" });
const tok = (...args) => execFileSync(tokBin, args, { env: { ...process.env, TOKNILA_HOME: home }, encoding: "utf8" });
const git = (dir, ...args) =>
  run("git", ["-C", dir, "-c", "user.name=t", "-c", "user.email=t@example.com", "-c", "commit.gpgsign=false", ...args]);

let browser;
try {
  const repo = join(home, "repo");
  mkdirSync(repo, { recursive: true });
  git(repo, "init", "--quiet");
  git(repo, "symbolic-ref", "HEAD", "refs/heads/develop");
  writeFileSync(join(repo, "README.md"), "hi\n");
  git(repo, "add", "README.md");
  git(repo, "commit", "--quiet", "-m", "init");
  mkdirSync(join(home, "profiles"), { recursive: true });
  // Each line typed is echoed and answered with a `say`, so the box is free for the next message.
  const answer = ["echo", "post", "/planner/say", JSON.stringify({ text: "Done." })];
  const steps = ["print", "planner ready", ...answer, ...answer, ...answer, "hold"].map((x) => `'${x}'`).join(", ");
  writeFileSync(join(home, "profiles", "fake-planner.toml"), `role = 'planner'\ncommand = ['${fakeAgent}', ${steps}]\nstartup_secs = 0\n`);
  tok("project", "new", "PN", "--repo", repo, "--base", "develop");
  tok("chain", "new", "work", "--project", "PN", "--no-autostart");
  tok("ticket", "new", "work", "Login screen");
  tok("planner", "start", "PN", "--profile", "fake-planner");
  const { url } = JSON.parse(tok("open", "--json"));
  const origin = new URL(url).origin;
  const bearer = { authorization: `Bearer ${new URL(url).searchParams.get("token")}` };
  const scrollback = async () => await (await fetch(`${origin}/v1/projects/PN/planner/scrollback`, { headers: bearer })).text();
  const conversation = async () => (await (await fetch(`${origin}/v1/projects/PN/conversation`, { headers: bearer })).json()).messages;
  const at = (path) => {
    const u = new URL(url);
    u.pathname = path;
    return u.href;
  };

  browser = await launch();
  const page = await browser.newPage({ viewport: { width: 1400, height: 900 } });
  const errors = [];
  page.on("pageerror", (e) => errors.push(e.message));
  const { expectText } = checks(page);
  const t = (id) => page.locator(`[data-testid=${id}]`);
  const shown = async (what, want) => {
    await page.waitForFunction((n) => document.querySelectorAll("[data-testid=assistant-panel]").length === n, want ? 1 : 0, { timeout: 10_000 });
    console.log(`PASS ${what}`);
  };
  const toldAbout = async (line) => {
    for (const stop = Date.now() + 15_000; Date.now() < stop; await new Promise((r) => setTimeout(r, 150))) {
      if ((await scrollback()).includes(`got:${line}`)) return console.log(`PASS the assistant was told: ${line}`);
    }
    throw new Error(`the assistant was not told "${line}": ${(await scrollback()).slice(-400)}`);
  };

  // 1. On a chain the panel is closed until the top bar's button opens it; the chip names the chain.
  await page.goto(at("/projects/PN/chains/work"));
  await t("assistant-toggle").waitFor();
  await shown("the panel is closed by default", false);
  await t("assistant-toggle").click();
  await shown("the top bar's button opens the panel", true);
  await expectText(t("context-chip"), "Chain work", "the chip says which chain is open");
  const grid = await t("assistant-panel").boundingBox();
  if (!grid || Math.abs(grid.width - 360) > 2) throw new Error(`the panel is not 360 px wide by default: ${grid?.width}`);
  console.log("PASS the panel is 360 px wide by default");

  // 2. Its width and state survive a reload.
  await t("assistant-handle").focus();
  await page.keyboard.press("ArrowLeft");
  await page.keyboard.press("ArrowLeft");
  await page.waitForFunction(() => document.querySelector("[data-testid=assistant-handle]")?.getAttribute("aria-valuenow") === "408");
  await page.reload();
  await shown("the panel is still open after a reload", true);
  if ((await t("assistant-handle").getAttribute("aria-valuenow")) !== "408") throw new Error("the width was not remembered");
  console.log("PASS the width survived the reload");

  // 3. A message with the chain chip is typed `About chain work: ...` and stored with the chain.
  await t("planner-ask-input").fill("reorder the cards");
  await t("planner-ask-send").click();
  await toldAbout("About chain work: reorder the cards");
  const sent = (await conversation()).find((m) => m.text === "reorder the cards");
  if (sent?.about?.kind !== "chain" || sent.about.name !== "work") throw new Error(`stored without the chain: ${JSON.stringify(sent)}`);
  console.log("PASS the message is stored with the chain as its context");

  // 4. The person takes the chip off: the next message is plain.
  await t("planner-ask-input").waitFor();
  await t("context-chip-remove").click();
  if ((await t("context-chip").count()) !== 0) throw new Error("the chip is still there");
  await t("planner-ask-input").fill("plain words");
  await t("planner-ask-send").click();
  await toldAbout("plain words");
  const plain = (await conversation()).find((m) => m.text === "plain words");
  if (plain?.about) throw new Error(`the plain message has a context: ${JSON.stringify(plain)}`);
  console.log("PASS with the chip off the message is general");

  // 5. The roadmap: the chip says so and the line is `About the roadmap: ...`. The shortcut closes
  //    and opens the panel there.
  await page.goto(at("/projects/PN"));
  await t("roadmap").waitFor();
  await expectText(t("context-chip"), "Roadmap", "the roadmap has its chip (the chip came back with a new screen)");
  await page.keyboard.press("Control+.");
  await shown("Ctrl+. closes the panel on the roadmap", false);
  await page.keyboard.press("Control+.");
  await shown("Ctrl+. opens it again", true);
  await t("planner-ask-input").fill("add an epic for billing");
  await t("planner-ask-send").click();
  await toldAbout("About the roadmap: add an epic for billing");
  if ((await t("roadmap").count()) !== 1) throw new Error("sending from the roadmap left the roadmap");
  console.log("PASS the roadmap stays on screen while the assistant is told");

  // 6. "Only about this" on the chain shows the chain's messages and no others; it is remembered.
  await page.goto(at("/projects/PN/chains/work"));
  await expectText(t("chat-text"), "plain words", "the whole chat shows every message");
  await t("only-about").check();
  await page.waitForFunction(() => [...document.querySelectorAll("[data-testid=chat-text]")].every((e) => e.textContent === "reorder the cards"));
  console.log("PASS only about this keeps the open chain's messages");
  await page.reload();
  await t("only-about").waitFor();
  if (!(await t("only-about").isChecked())) throw new Error("the filter was not remembered");
  console.log("PASS the filter survived the reload");

  // 7. On a card the chip names the card; the top bar's button closes the panel.
  await page.goto(at("/projects/PN/chains/work/cards/PN-1"));
  await expectText(t("context-chip"), "Card PN-1", "a card has its chip");
  await t("assistant-toggle").click();
  await shown("the button closes the panel on a card", false);
  await t("assistant-toggle").click();
  await shown("and opens it again", true);

  // 8. Under 1100 px the panel overlays the screen with a scrim; Escape closes it.
  await page.setViewportSize({ width: 1000, height: 800 });
  await t("assistant-scrim").waitFor();
  const overlay = await t("assistant-panel").evaluate((el) => getComputedStyle(el).position);
  if (overlay !== "fixed") throw new Error(`the panel is ${overlay}, not an overlay, under 1100 px`);
  console.log("PASS under 1100 px the panel overlays the screen");
  await page.keyboard.press("Escape");
  await shown("Escape closes the overlay", false);

  if (errors.length) throw new Error(`the page threw: ${errors.join("; ")}`);
  console.log("web assistant panel: PASS");
} catch (e) {
  console.error(`web assistant panel: FAIL: ${e.message.slice(0, 800)}`);
  process.exitCode = 1;
} finally {
  await browser?.close();
  try {
    tok("planner", "stop", "PN");
  } catch {
    /* not running */
  }
  await cleanup(tok, home);
}
