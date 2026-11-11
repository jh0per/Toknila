// Browser test of "Talk to the assistant" on a card's page, against a real daemon and the
// workspace's `fake-agent` (never a real agent). The fake planner plays what the planner's tools do
// with its run token: it reads a line from its terminal, answers in the card's discussion thread
// (`thread_message`) and proposes an edit (`edit_card`).
// Usage: node e2e/discuss.mjs <tok binary> <fake-agent binary>   (cargo xtask e2e-web runs it)
//
// One round, end to end: the assistant is not running, so the box offers to start it; running, a
// message goes to the planner with the card as context and its answer shows in the same box; the
// edit it proposes is a diff that changes nothing until Accept (the description changes then); a
// second proposal is rejected and the card stays as it was; and the card's agent, started after
// the talk, finds the discussion in its instructions.
// SCREENSHOT_DIR, when set, receives the picture of the proposed diff.

import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { checks, cleanup, launch } from "./helpers.mjs";

const [tokArg, agentArg] = process.argv.slice(2);
if (!tokArg || !agentArg) {
  console.error("usage: node e2e/discuss.mjs <tok binary> <fake-agent binary>");
  process.exit(2);
}
const tokBin = resolve(tokArg);
const fakeAgent = resolve(agentArg);
const home = mkdtempSync(join(tmpdir(), "toknila-discuss-"));
const run = (cmd, args) => execFileSync(cmd, args, { encoding: "utf8" });
const tok = (...args) => execFileSync(tokBin, args, { env: { ...process.env, TOKNILA_HOME: home }, encoding: "utf8" });
const git = (dir, ...args) =>
  run("git", ["-C", dir, "-c", "user.name=t", "-c", "user.email=t@example.com", "-c", "commit.gpgsign=false", ...args]);
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const show = (key) => JSON.parse(tok("show", key, "--json"));
const logs = (key) => {
  try {
    return tok("logs", key);
  } catch {
    return "";
  }
};
async function until(what, f) {
  for (const stop = Date.now() + 20_000; Date.now() < stop; await sleep(100)) {
    if (f()) return;
  }
  throw new Error(`timed out waiting for ${what}`);
}

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
  const steps = (...s) => s.map((x) => `'${x}'`).join(", ");
  const answer = (text) => ["post", "/planner/cards/DS-1/threads/1/messages", JSON.stringify({ text })];
  const edit = (change) => ["patch", "/planner/cards/DS-1", JSON.stringify(change)];
  writeFileSync(
    join(home, "profiles", "fake-planner.toml"),
    `role = 'planner'\ncommand = ['${fakeAgent}', ${steps(
      "print", "planner ready",
      "echo", ...answer("I would add the 4xx case"), ...edit({ acceptance: ["Errors are shown", "A 4xx keeps the form filled"] }),
      "echo", ...answer("Here is a shorter prompt"), ...edit({ prompt: "A shorter prompt" }),
      "hold",
    )}]\nstartup_secs = 0\n`,
  );
  writeFileSync(
    join(home, "profiles", "reader.toml"),
    `command = ['${fakeAgent}', ${steps("print", "ready", "flagfile", "--instr", "hold")}]\ninstructions_flag = "--instr"\n`,
  );
  tok("project", "new", "DS", "--repo", repo, "--base", "develop", "--profile", "reader");
  tok("chain", "new", "work", "--project", "DS", "--no-autostart");
  tok("ticket", "new", "work", "Login screen", "--prompt", "Old prompt", "--accept", "Errors are shown");
  const { url } = JSON.parse(tok("open", "--json"));

  browser = await launch();
  const page = await browser.newPage({ viewport: { width: 1280, height: 1000 } });
  const errors = [];
  page.on("pageerror", (e) => errors.push(e.message));
  const { expectText } = checks(page);
  const t = (id) => page.locator(`[data-testid=${id}]`);
  const open = () => page.goto(url.replace("/?", "/projects/DS/chains/work/cards/DS-1?"));
  const description = () => page.locator(".details .cols section").first().locator("pre.prompt");

  // 1. The assistant is not running: the box offers to start it, and there is nothing to send to.
  await open();
  await expectText(t("discuss-off"), "The assistant is not running", "the box says the assistant is not running");
  await expectText(t("discuss-start"), "Start the assistant", "with a button to start it");
  if ((await t("discuss-input").count()) !== 0) throw new Error("no message box while the assistant is not running");
  console.log("PASS no message box while the assistant is not running");

  // 2. Running (started with the fake planner profile), the box is there.
  tok("planner", "start", "DS", "--profile", "fake-planner");
  await open();
  await t("discuss-input").waitFor({ timeout: 10_000 });
  await expectText(description(), "Old prompt", "the card has its prompt");

  // 3. A message reaches the planner with the card as context; the answer shows in the same box.
  await t("discuss-input").fill("make the acceptance stricter on errors");
  await t("discuss-send").click();
  await expectText(t("discuss-items"), "make the acceptance stricter on errors", "the message shows in the box");
  await expectText(t("discuss-items"), "I would add the 4xx case", "the planner's answer shows in the same box");
  const terminalLine = await (async () => {
    const out = await fetch(new URL("/v1/projects/DS/planner/scrollback", new URL(url).origin), {
      headers: { authorization: `Bearer ${new URL(url).searchParams.get("token")}` },
    });
    return await out.text();
  })();
  if (!terminalLine.includes("got:Discussion on card DS-1 (thread 1): make the acceptance stricter on errors")) {
    throw new Error(`the planner was not told the card and the words: ${terminalLine.slice(-400)}`);
  }
  console.log("PASS the planner was told the card, the thread and the words");

  // 4. Its proposed edit is a diff; nothing landed.
  const proposal = t("discuss-edit").first();
  await proposal.waitFor({ timeout: 10_000 });
  await expectText(proposal.locator("[data-field=acceptance]"), "+ A 4xx keeps the form filled", "the proposal is a diff with the added line");
  await expectText(proposal.locator("[data-field=prompt]"), "No change", "the prompt is not touched");
  await expectText(description(), "Old prompt", "nothing landed yet");
  if (JSON.stringify(show("DS-1").ticket.acceptance) !== JSON.stringify(["Errors are shown"])) throw new Error("the acceptance changed before Accept");
  if (process.env.SCREENSHOT_DIR) await page.screenshot({ path: join(process.env.SCREENSHOT_DIR, "discuss-diff.png") });
  await t("edit-accept").click();
  await expectText(t("edit-outcome"), "Accepted", "Accept says so");
  await until("the card has the accepted acceptance", () => show("DS-1").ticket.acceptance.length === 2);
  await expectText(page.locator(".details .cols section").nth(1), "A 4xx keeps the form filled", "the card's page shows it");
  console.log("PASS an accepted edit lands on the card");

  // 5. A second proposal is rejected: the card stays as it was.
  await t("discuss-input").fill("and a shorter prompt");
  await t("discuss-send").click();
  await expectText(t("discuss-items"), "Here is a shorter prompt", "the second answer shows");
  const second = t("discuss-edit").nth(1);
  await expectText(second.locator("[data-field=prompt]"), "+ A shorter prompt", "the second diff shows the new prompt");
  await t("edit-reject").click();
  await expectText(t("edit-outcome").nth(1), "Rejected", "Reject says so");
  if (show("DS-1").ticket.prompt !== "Old prompt") throw new Error("a rejected edit changed the prompt");
  await expectText(description(), "Old prompt", "the prompt is unchanged");
  console.log("PASS a rejected edit leaves the card as it was");

  // 6. The discussion is a thread of the card that its agent reads.
  tok("start", "DS-1");
  await until("the agent printed its instructions", () => logs("DS-1").includes("file:## Discussion with the assistant"));
  const out = logs("DS-1");
  for (const line of [
    "file:- person: make the acceptance stricter on errors",
    "file:- assistant: I would add the 4xx case",
    "(accepted: the card above has it)",
    "(rejected: the card is unchanged)",
  ]) {
    if (!out.includes(line)) throw new Error(`the agent's instructions lack "${line}":\n${out.slice(-800)}`);
  }
  console.log("PASS the card's agent reads the discussion in its instructions");

  if (errors.length) throw new Error(`the page threw: ${errors.join("; ")}`);
  console.log("web discuss: PASS");
} catch (e) {
  console.error(`web discuss: FAIL: ${e.message.slice(0, 800)}`);
  if (process.env.SCREENSHOT_DIR) {
    await browser?.contexts()[0]?.pages()[0]?.screenshot({ path: join(process.env.SCREENSHOT_DIR, "discuss-failed.png") }).catch(() => {});
  }
  process.exitCode = 1;
} finally {
  await browser?.close();
  try {
    tok("planner", "stop", "DS");
  } catch {
    /* not running */
  }
  await cleanup(tok, home);
}
