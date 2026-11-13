// Browser test of asking the assistant in a story's chat, against a real daemon and the workspace's
// `fake-agent` (never a real agent). The fake planner reads a line from its terminal and answers
// with `say` about the story, as the planner's tool does.
// Usage: node e2e/ask-story.mjs <tok binary> <fake-agent binary>   (cargo xtask e2e-web runs it)
// needs: fake-agent
//
// A story in Review at the person's turn (its hand-in has a PERSON item): the composer offers Ask
// and Tell the agent, Ask is the default; the card shows Needs you with the item, a hint above
// the buttons (until dismissed) and a line under Accept and Return. The person asks; the question
// reaches the assistant's terminal with the story's record; the assistant's answer appears as its
// own bubble; no instruction is queued for the agent and the story is as it was.

import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { checks, cleanup, launch, shipOf } from "./helpers.mjs";

const [tokArg, agentArg] = process.argv.slice(2);
if (!tokArg || !agentArg) {
  console.error("usage: node e2e/ask-story.mjs <tok binary> <fake-agent binary>");
  process.exit(2);
}
const tokBin = resolve(tokArg);
const fakeAgent = resolve(agentArg);
const home = mkdtempSync(join(tmpdir(), "toknila-ask-story-"));
const tok = (...args) => execFileSync(tokBin, args, { env: { ...process.env, TOKNILA_HOME: home }, encoding: "utf8" });
const git = (dir, ...args) =>
  execFileSync("git", ["-C", dir, "-c", "user.name=t", "-c", "user.email=t@example.com", "-c", "commit.gpgsign=false", ...args], { encoding: "utf8" });
const show = (key) => JSON.parse(tok("show", key, "--json"));
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
async function until(what, f) {
  for (const stop = Date.now() + 30_000; Date.now() < stop; await sleep(100)) {
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
  const answer = "Open the login on a phone. Accept merges the story; Return sends your words to the agent.";
  writeFileSync(
    join(home, "profiles", "fake-planner.toml"),
    `role = 'planner'\ncommand = ['${fakeAgent}', ${steps("print", "planner ready", "echo", "post", "/planner/say", JSON.stringify({ text: answer, about: "ASK-1" }), "hold")}]\nstartup_secs = 0\n`,
  );
  tok("project", "new", "ASK", "--repo", repo, "--base", "develop", "--sim");
  tok("review-mode", "human", "--project", "ASK");
  tok("chain", "new", "work", "--project", "ASK", "--no-autostart");
  tok("ticket", "new", "work", "Login screen", "--prompt", "Build the login.", "--accept", "Looks right on a phone");
  tok("start", "ASK-1");
  const ship = shipOf(tok, "ASK-1");
  writeFileSync(join(ship, "login.txt"), "login\n");
  git(ship, "add", "login.txt");
  git(ship, "commit", "--quiet", "-m", "login");
  tok("sim", "ASK-1", "finish", "**What changed:** the login\n**Checks:**\n- PASS cargo test: test result: ok. 12 passed; 0 failed\n- PERSON look at the login on a phone\n**Left:** nothing");
  await until("the card to wait for a person", () => show("ASK-1").review.awaiting?.by === "person");
  const instructions = () => (show("ASK-1").comments ?? []).filter((c) => c.kind === "instruction").length;
  const before = { instructions: instructions(), prompt: show("ASK-1").ticket.prompt, status: show("ASK-1").ticket.status };
  tok("planner", "start", "ASK", "--profile", "fake-planner");
  const { url } = JSON.parse(tok("open", "--json"));

  browser = await launch();
  const page = await browser.newPage({ viewport: { width: 1280, height: 1000 } });
  const errors = [];
  page.on("pageerror", (e) => errors.push(e.message));
  const { expectText } = checks(page);
  const t = (id) => page.locator(`[data-testid=${id}]`);
  const open = () => page.goto(url.replace("/?", "/projects/ASK/chains/work/cards/ASK-1?"));

  // 1. The Review card says what the person must do and what the buttons do.
  await open();
  await t("review-block").waitFor({ timeout: 10_000 });
  await expectText(t("needs-you"), "look at the login on a phone", "Needs you lists the PERSON item of the hand-in");
  await expectText(t("review-lines"), "Accept: the story merges", "a line under the buttons says what Accept does");
  await expectText(t("review-lines"), "Return: the agent fixes what you found", "and what Return does");
  await expectText(t("review-hint"), "Accept merges the story. Return sends your words to the agent, which fixes and hands in again.", "the first-time hint is there");

  // 2. The composer: Ask is the default at the person's turn.
  await page.getByRole("tab", { name: "Chat" }).click();
  await t("chat-box").waitFor();
  if ((await t("chat-box").getAttribute("data-mode")) !== "ask") throw new Error("Ask is not the default on a Review card");
  const placeholder = await t("chat-input").getAttribute("placeholder");
  if (placeholder !== "Ask what to do, or tell the agent for its next run") throw new Error(`placeholder: ${placeholder}`);
  await expectText(t("chat-ask"), "Ask", "the composer has Ask");
  await expectText(t("chat-send"), "Tell the agent", "and Tell the agent");
  console.log("PASS the composer offers Ask and Tell the agent, Ask by default");

  // 3. Enter asks: the question reaches the assistant with the record; its answer is its own bubble.
  await t("chat-input").fill("шо треба зробити?");
  await page.keyboard.press("Enter");
  await expectText(t("chat-items"), answer, "the assistant's answer appears in the chat");
  const sides = await page.locator("[data-testid=bubble]").evaluateAll((els) => els.map((e) => `${e.dataset.side}:${e.dataset.kind}`));
  // The agent's hand-in is the first bubble; the question and the answer follow it.
  if (sides.slice(-2).join(",") !== "you:ask,assistant:answer") throw new Error(`the bubbles are ${sides}`);
  console.log("PASS the question and the assistant's answer are two bubbles");
  const terminal = await (async () => {
    const out = await fetch(new URL("/v1/projects/ASK/planner/scrollback", new URL(url).origin), {
      headers: { authorization: `Bearer ${new URL(url).searchParams.get("token")}` },
    });
    return await out.text();
  })();
  for (const want of ["got:Question on story ASK-1: шо треба зробити?", "Title: Login screen", "PERSON look at the login on a phone"]) {
    if (!terminal.includes(want)) throw new Error(`the assistant was not told "${want}": ${terminal.slice(-600)}`);
  }
  console.log("PASS the assistant was told the question with the story's record");

  // 4. Nothing was queued for the agent and the story is as it was.
  const after = show("ASK-1");
  if (instructions() !== before.instructions) throw new Error("asking queued an instruction");
  if (after.ticket.prompt !== before.prompt || after.ticket.status !== before.status) throw new Error("asking changed the story");
  if ((await t("bubble-pending").count()) !== 0) throw new Error("a pending sentence shows after an ask");
  console.log("PASS asking queued no instruction and changed nothing on the story");

  // 5. The hint stays until dismissed, then does not come back.
  await open();
  await t("review-hint").waitFor();
  await t("review-hint-dismiss").click();
  await open();
  await t("review-block").waitFor();
  if ((await t("review-hint").count()) !== 0) throw new Error("the hint came back after it was dismissed");
  console.log("PASS the hint stays until dismissed");

  if (errors.length) throw new Error(`the page threw: ${errors.join("; ")}`);
  console.log("web ask-story: PASS");
} catch (e) {
  console.error(`web ask-story: FAIL: ${e.message.slice(0, 800)}`);
  process.exitCode = 1;
} finally {
  await browser?.close();
  try {
    tok("planner", "stop", "ASK");
  } catch {
    /* not running */
  }
  await cleanup(tok, home);
}
