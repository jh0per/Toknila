// Browser test of the assistant on the roadmap against a real daemon and the workspace's
// `fake-agent` as the planner (never a real agent).
// Usage: node e2e/roadmap-assistant.mjs <tok binary> <fake-agent binary>   (cargo xtask e2e-web runs it)
// needs: fake-agent
//
// An empty roadmap carries the starter and the panel's box has the focus. A message sent on the
// roadmap reaches the planner with the `About the roadmap:` prefix and is stored with the roadmap as
// its context; the planner's `say` about the roadmap shows under the roadmap chip; two `new_chain`
// calls with versions appear as two epics at the end of the roadmap, in order, with a did-line each.
// "Only about this" on the roadmap shows the roadmap's messages.

import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { checks, cleanup, launch } from "./helpers.mjs";

const [tokArg, agentArg] = process.argv.slice(2);
if (!tokArg || !agentArg) {
  console.error("usage: node e2e/roadmap-assistant.mjs <tok binary> <fake-agent binary>");
  process.exit(2);
}
const tokBin = resolve(tokArg);
const fakeAgent = resolve(agentArg);
const home = mkdtempSync(join(tmpdir(), "toknila-rmassist-"));
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
  const steps = [
    "print", "planner ready",
    "echo",
    "post", "/planner/say", JSON.stringify({ text: "Two releases: the list, then billing. Shall I add them?", about: "roadmap" }),
    "post", "/planner/chains", JSON.stringify({ name: "list", goal: "Show the items.", version: "0.5.0" }),
    "post", "/planner/chains", JSON.stringify({ name: "billing", goal: "Take payments.", version: "0.6.0" }),
    // An epic the owner named in words: the title only, the daemon derives the slug.
    "post", "/planner/chains", JSON.stringify({ title: "MIDI controllers", goal: "Play it with a keyboard.", version: "0.7.0" }),
    "hold",
  ].map((x) => `'${x}'`).join(", ");
  writeFileSync(join(home, "profiles", "fake-planner.toml"), `role = 'planner'\ncommand = ['${fakeAgent}', ${steps}]\nstartup_secs = 0\n`);
  tok("project", "new", "RA", "--repo", repo, "--base", "develop");
  tok("planner", "start", "RA", "--profile", "fake-planner");
  const { url } = JSON.parse(tok("open", "--json"));
  const origin = new URL(url).origin;
  const bearer = { authorization: `Bearer ${new URL(url).searchParams.get("token")}` };
  const scrollback = async () => await (await fetch(`${origin}/v1/projects/RA/planner/scrollback`, { headers: bearer })).text();
  const conversation = async () => (await (await fetch(`${origin}/v1/projects/RA/conversation`, { headers: bearer })).json()).messages;

  browser = await launch();
  const page = await browser.newPage({ viewport: { width: 1400, height: 900 } });
  const errors = [];
  page.on("pageerror", (e) => errors.push(e.message));
  const { expectText } = checks(page);
  const t = (id) => page.locator(`[data-testid=${id}]`);

  // 1. The empty roadmap and the empty panel carry the starter; the composer has the focus.
  const u = new URL(url);
  u.pathname = "/projects/RA";
  await page.goto(u.href);
  await t("no-chains").waitFor();
  await expectText(t("roadmap-starter"), "Tell the assistant what to build; it splits it into releases", "the empty roadmap says what to do");
  await page.waitForFunction(() => document.activeElement?.getAttribute("data-testid") === "empty-ask-input", null, { timeout: 10_000 });
  await page.keyboard.press("Control+.");
  await expectText(t("placeholder"), "Tell the assistant what to build; it splits it into releases", "the empty panel carries the starter");
  await page.waitForFunction(() => document.activeElement?.getAttribute("data-testid") === "planner-ask-input", null, { timeout: 10_000 }).catch(async (e) => {
    throw new Error(`${e.message} focus is on ${await page.evaluate(() => document.activeElement?.outerHTML.slice(0, 120))}`);
  });
  console.log("PASS the starter shows on the empty roadmap and panel, with the composer focused");

  // 2. A message on the roadmap reaches the planner with the prefix and is stored about the roadmap.
  await t("planner-ask-input").fill("build a shop with a list and billing");
  await t("planner-ask-send").click();
  const line = "About the roadmap: build a shop with a list and billing";
  for (const stop = Date.now() + 20_000; !(await scrollback()).includes(`got:${line}`); await new Promise((r) => setTimeout(r, 150))) {
    if (Date.now() > stop) throw new Error(`the planner was not told "${line}": ${(await scrollback()).slice(-400)}`);
  }
  console.log("PASS the planner got the line with the roadmap prefix");
  const mine = (await conversation()).find((m) => m.author === "person");
  if (mine?.about?.kind !== "roadmap") throw new Error(`the message is not about the roadmap: ${JSON.stringify(mine)}`);
  console.log("PASS the message is stored about the roadmap");

  // 3. The say shows under the roadmap chip; the two epics are at the end of the roadmap in order.
  const said = page.locator("[data-author=assistant]");
  await expectText(said, "Two releases", "the planner's say is an assistant bubble");
  await expectText(said.locator("[data-testid=chat-chip]"), "roadmap", "with the roadmap as its chip");
  await page.waitForFunction(() => document.querySelectorAll("[data-testid=roadmap-node]").length === 3, null, { timeout: 15_000 });
  const names = await t("roadmap-name").allInnerTexts();
  const versions = await t("roadmap-version").allInnerTexts();
  // A name without a title reads made readable ("list" -> "List"); the titled epic reads as titled.
  if (JSON.stringify(names) !== JSON.stringify(["List", "Billing", "MIDI controllers"])) throw new Error(`epics out of order: ${names}`);
  if (!versions[0].includes("0.5.0") || !versions[1].includes("0.6.0") || !versions[2].includes("0.7.0")) throw new Error(`versions: ${versions}`);
  console.log("PASS three epics at the end of the roadmap, in order, with their versions and titles");
  const did = (await t("chat-did").allInnerTexts()).map((l) => l.split("\n")[0].replace(/\s+/g, " ").trim());
  if (JSON.stringify(did) !== JSON.stringify(["added epic List v0.5.0", "added epic Billing v0.6.0", "added epic MIDI controllers v0.7.0"])) throw new Error(`did-lines: ${JSON.stringify(did)}`);
  console.log("PASS each epic is a did-line `added epic TITLE vVERSION`");
  // The titled epic: the daemon derived the slug, the window reads the title and keeps the slug for the machine.
  const shown = JSON.parse(tok("chain", "show", "midi-controllers", "--json"));
  if (shown.chain.title !== "MIDI controllers" || shown.chain.name !== "midi-controllers" || shown.summary.display_title !== "MIDI controllers") {
    throw new Error(`the titled epic: ${JSON.stringify(shown.chain)} ${shown.summary.display_title}`);
  }
  const node = page.locator("[data-testid=roadmap-node][data-chain=midi-controllers]");
  await expectText(node.locator("[data-testid=roadmap-name]"), "MIDI controllers", "the roadmap node of the titled epic reads its title");
  await expectText(page.locator("[data-chain-item=midi-controllers] .chain-name"), "MIDI controllers", "the sidebar row reads the title");
  await expectText(t("chat-did-link").last(), "MIDI controllers", "the did-line links the title");
  console.log("PASS the slug is derived from the title and stays out of the window's words");
  // An empty chain shows a prompt over the diagram; with a story the Start node is in reach.
  tok("ticket", "new", "midi-controllers", "Map the pads");
  const epicUrl = new URL(url);
  epicUrl.pathname = "/projects/RA/chains/midi-controllers";
  await page.goto(epicUrl.href);
  await expectText(t("chain-name-top"), "MIDI controllers", "the chain header and breadcrumb read the title");
  await t("start-node").click();
  await t("epic-panel").waitFor();
  await expectText(t("epic-slug"), "midi-controllers", "the epic panel shows the slug as the machine side");
  await t("epic-name").fill("MIDI pads and keys");
  await t("epic-save").click();
  await expectText(t("epic-title"), "MIDI pads and keys", "the Start node edits the title in place");
  const renamed = JSON.parse(tok("chain", "show", "midi-controllers", "--json"));
  if (renamed.chain.title !== "MIDI pads and keys" || renamed.chain.name !== "midi-controllers") throw new Error(`rename: ${JSON.stringify(renamed.chain)}`);
  console.log("PASS the new title is stored and the slug is untouched");
  await page.goto(u.href);
  await page.waitForFunction(() => document.querySelectorAll("[data-testid=roadmap-node]").length === 3, null, { timeout: 15_000 });

  // 4. "Only about this" on the roadmap shows the roadmap's messages (the person's and the say).
  await t("only-about").check();
  await page.waitForFunction(() => document.querySelectorAll("[data-testid=chat-message]").length === 2, null, { timeout: 10_000 });
  console.log("PASS only about this on the roadmap shows the roadmap's messages");

  if (errors.length) throw new Error(`the page threw: ${errors.join("; ")}`);
  console.log("web roadmap assistant: PASS");
} catch (e) {
  console.error(`web roadmap assistant: FAIL: ${e.message.slice(0, 800)}`);
  process.exitCode = 1;
} finally {
  await browser?.close();
  try {
    tok("planner", "stop", "RA");
  } catch {
    /* not running */
  }
  await cleanup(tok, home);
}
