// Browser test of what the window shows while the assistant's turn is on, against a real daemon and
// the workspace's `fake-agent` as the planner (never a real agent). The fake plays the planner's
// hooks (`activity`): on the first line it starts a turn, names a step, keeps working a few
// seconds, answers and stops; on the second line it starts a turn and stops without answering.
// The person sends from an empty epic's box: the box shows what was sent and the waiting line, the
// daemon's step shows beside the minutes (the box, the chat, the Assistant button's and the
// sidebar row's pulse), the answer ends it; the second turn ends without an answer and the chat
// and the box say so at once with Nudge.
// Usage: node e2e/assistant-working.mjs <tok binary> <fake-agent binary>   (cargo xtask e2e-web runs it)
// needs: fake-agent

import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { checks, cleanup, launch, shoot } from "./helpers.mjs";

const [tokArg, agentArg] = process.argv.slice(2);
if (!tokArg || !agentArg) {
  console.error("usage: node e2e/assistant-working.mjs <tok binary> <fake-agent binary>");
  process.exit(2);
}
const tokBin = resolve(tokArg);
const fakeAgent = resolve(agentArg);
const home = mkdtempSync(join(tmpdir(), "toknila-working-"));
const run = (cmd, args) => execFileSync(cmd, args, { encoding: "utf8" });
const tok = (...args) => execFileSync(tokBin, args, { env: { ...process.env, TOKNILA_HOME: home }, encoding: "utf8" });
const git = (dir, ...args) =>
  run("git", ["-C", dir, "-c", "user.name=t", "-c", "user.email=t@example.com", "-c", "commit.gpgsign=false", ...args]);

/// The step the daemon makes of the fake's `Read` of the repository's README.
const STEP = "Reading README.md";
/// How long the window waits before it offers Nudge for a message no turn started for.
const NUDGE_AFTER_MS = 90_000;

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
    // The first message: a turn with a step, a few seconds of work, the answer, the end.
    "echo",
    "activity", "user_prompt_submit",
    "activity", "pre_tool_use", "Read", `Read: ${join(repo, "README.md")}`,
    "sleep", "8000",
    "post", "/planner/say", JSON.stringify({ text: "Two stories." }),
    "activity", "stop",
    // The second message: a turn that ends without an answer.
    "echo",
    "activity", "user_prompt_submit",
    "sleep", "1500",
    "activity", "stop",
    "hold",
  ];
  const list = steps.map((s) => `'${s}'`);
  writeFileSync(join(home, "profiles", "fake-planner.toml"), `role = 'planner'\ncommand = ['${fakeAgent}', ${list.join(", ")}]\nstartup_secs = 0\n`);
  tok("project", "new", "WEB", "--repo", repo, "--base", "develop");
  tok("chain", "new", "hide", "--project", "WEB", "--no-autostart");
  const { url } = JSON.parse(tok("open", "--json"));

  browser = await launch();
  const context = await browser.newContext({ viewport: { width: 1440, height: 1000 } });
  const page = await context.newPage();
  const errors = [];
  page.on("pageerror", (e) => errors.push(e.message));
  const { expectText, expectPath } = checks(page);
  const t = (id) => page.locator(`[data-testid=${id}]`);
  const panel = (id) => page.locator(`[data-testid=assistant-panel] [data-testid=${id}]`);
  const gone = (loc, what) => loc.waitFor({ state: "detached", timeout: 30_000 }).catch(() => { throw new Error(`${what}: still on the screen`); });
  const u = new URL(url);
  u.pathname = "/projects/WEB/planner";
  await page.goto(u.href);

  // The planner profile of this test (the built-in one would launch the real agent); the choice is
  // the window's, so the epic is opened from the sidebar, not by loading a new page.
  await t("planner-tab-terminal").click();
  await t("planner-profile").selectOption("fake-planner");
  await page.locator('[data-chain-item="hide"] a').first().click();
  await expectPath("/projects/WEB/chains/hide", "the empty epic opens");
  await expectText(t("chain-ask-send"), "Plan it with the assistant", "the empty epic leads with its box");

  // 1. The person sends from the empty epic's box: what was sent and the waiting line stay.
  await t("chain-ask-input").fill("plan");
  await t("chain-ask-send").click();
  await shoot(page, "assistant-working-sent");
  await expectText(t("chain-ask-sent-text"), "plan", "the box shows what was sent");
  await expectText(t("chain-ask-waiting-text"), "The assistant is answering", "the box shows the waiting line under it");
  await expectText(t("chain-ask-open-chat"), "Open the chat", "the box links to the chat");
  if ((await t("chain-ask-input").count()) !== 0) throw new Error("the box's text box is back while the assistant answers");
  console.log("PASS the empty epic's box shows the sent text and the waiting line");

  // 2. The fake names a step: the box, the chat, the Assistant button and the sidebar row show it.
  await t("chain-ask-step").filter({ hasText: STEP }).waitFor({ timeout: 20_000 }).catch(() => { throw new Error(`the box's waiting line never showed the step ${STEP}`); });
  await expectText(panel("chat-step"), STEP, "the chat's waiting line shows the step beside the minutes");
  await t("assistant-working").waitFor({ timeout: 10_000 });
  const title = (await t("assistant-toggle").getAttribute("title")) ?? "";
  if (!title.includes(`The assistant is working: ${STEP}`)) throw new Error(`the Assistant button's title does not say the step: ${title}`);
  await page.locator(`[data-testid=planner-working][aria-label="The assistant is working: ${STEP}"]`).waitFor({ timeout: 10_000 });
  if ((await t("chat-nudge").count()) !== 0) throw new Error("Nudge is offered while the assistant works");
  console.log("PASS the step shows in the box, the chat, the Assistant button and the sidebar row, and no Nudge");
  await shoot(page, "assistant-working-step");

  // 3. The answer ends it: the waiting lines and the pulse go, the box is back.
  await expectText(panel("chat-message").and(page.locator("[data-author=assistant]")), "Two stories.", "the answer is in the chat");
  await gone(t("chain-ask-sent"), "the box's sent view after the answer");
  await gone(panel("chat-waiting"), "the chat's waiting line after the answer");
  await gone(t("assistant-working"), "the Assistant button's pulse after the turn");
  await t("chain-ask-input").waitFor({ timeout: 10_000 });
  console.log("PASS the answer ends the waiting line and the box is back");

  // 4. A second turn ends without an answer: the chat and the box say so at once, with Nudge.
  const sent = Date.now();
  await t("chain-ask-input").fill("and the tests");
  await t("chain-ask-send").click();
  await expectText(t("chain-ask-sent-text"), "and the tests", "the box shows the second message");
  await panel("chat-stopped").waitFor({ timeout: 30_000 }).catch(() => { throw new Error("the chat never said the assistant stopped without answering"); });
  if (Date.now() - sent >= NUDGE_AFTER_MS) throw new Error("the stopped line came only after the 90 seconds of a lost message");
  await expectText(panel("chat-stopped"), "The assistant stopped without answering.", "the chat says the turn ended without an answer");
  await panel("chat-nudge").waitFor({ timeout: 5_000 });
  await expectText(t("chain-ask-stopped"), "The assistant stopped without answering.", "the box says it too");
  await t("chain-ask-nudge").waitFor({ timeout: 5_000 });
  await t("chain-ask-input").waitFor({ timeout: 5_000 });
  if ((await panel("chat-waiting").count()) !== 0) throw new Error("the chat still shows the waiting line after the turn ended");
  console.log(`PASS a turn that ended without an answer is said at once (${Math.round((Date.now() - sent) / 1000)} s), with Nudge`);
  await shoot(page, "assistant-working-stopped");

  if (errors.length) throw new Error(`the page threw: ${errors.join("; ")}`);
  console.log("web assistant working: PASS");
} catch (e) {
  console.error(`web assistant working: FAIL: ${e.message.slice(0, 800)}`);
  process.exitCode = 1;
} finally {
  await browser?.close();
  await cleanup(tok, home);
}
