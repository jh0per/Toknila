// Browser test of messages sent while the assistant answers, against a real daemon and the
// workspace's `fake-agent` as the planner (never a real agent). The fake reads a line, keeps quiet,
// answers, reads the next line, and so on. Three messages are sent while the first answer is slow:
// the second and third say they wait; when the first answer arrives the second becomes the awaited
// one (its note turns into the waiting line) and the third keeps its note; the last answer clears
// the waiting line.
// Usage: node e2e/queued-turns.mjs <tok binary> <fake-agent binary>   (cargo xtask e2e-web runs it)
// needs: fake-agent

import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { checks, cleanup, launch } from "./helpers.mjs";

const [tokArg, agentArg] = process.argv.slice(2);
if (!tokArg || !agentArg) {
  console.error("usage: node e2e/queued-turns.mjs <tok binary> <fake-agent binary>");
  process.exit(2);
}
const tokBin = resolve(tokArg);
const fakeAgent = resolve(agentArg);
const home = mkdtempSync(join(tmpdir(), "toknila-queued-"));
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
  const say = (text) => ["post", "/planner/say", JSON.stringify({ text })];
  const steps = [
    "print", "planner ready",
    "echo", "sleep", "6000", ...say("First answer."),
    "echo", "sleep", "4000", ...say("Second answer."),
    "echo", "sleep", "4000", ...say("Third answer."),
    "hold",
  ];
  const list = steps.map((s) => `'${s}'`);
  writeFileSync(join(home, "profiles", "fake-planner.toml"), `role = 'planner'\ncommand = ['${fakeAgent}', ${list.join(", ")}]\nstartup_secs = 0\n`);
  tok("project", "new", "WEB", "--repo", repo, "--base", "develop");
  const { url } = JSON.parse(tok("open", "--json"));

  browser = await launch();
  const context = await browser.newContext({ viewport: { width: 1280, height: 1000 } });
  const page = await context.newPage();
  const errors = [];
  page.on("pageerror", (e) => errors.push(e.message));
  const { expectText } = checks(page);
  const t = (id) => page.locator(`[data-testid=${id}]`);
  const count = async (id) => t(id).count();
  const u = new URL(url);
  u.pathname = "/projects/WEB/planner";
  await page.goto(u.href);

  await t("planner-tab-terminal").click();
  await t("planner-profile").selectOption("fake-planner");
  await t("planner-tab-chat").click();

  const send = async (text) => {
    await t("planner-ask-input").fill(text);
    await t("planner-ask-send").click();
  };
  await send("one");
  await expectText(t("chat-waiting-text"), "The assistant is answering", "the first message is awaited");
  await send("two");
  await send("three");
  await expectText(t("chat-queued"), "sent, the assistant reads it after this answer", "the later messages say they wait");
  await t("chat-queued").nth(1).waitFor({ timeout: 10_000 });
  if ((await count("chat-queued")) !== 2) throw new Error(`expected two queued notes, got ${await count("chat-queued")}`);
  console.log("PASS two messages sent meanwhile are marked as queued");

  // The first answer: the second message is now the awaited one, the third still queued.
  await expectText(page.locator("[data-author=assistant]"), "First answer.", "the first answer arrives");
  if ((await count("chat-waiting")) !== 1) throw new Error("the waiting line went with the first answer although the second message is unanswered");
  if ((await count("chat-queued")) !== 1) throw new Error(`after the first answer expected one queued note, got ${await count("chat-queued")}`);
  const awaited = page.locator("[data-testid=chat-message][data-author=person]").nth(1);
  if ((await awaited.locator("[data-testid=chat-queued]").count()) !== 0) throw new Error("the second message kept its queued note");
  await expectText(t("chat-waiting-text"), "The assistant is answering", "the second message is awaited after the first answer");
  console.log("PASS the second message is the awaited one and the third stays queued");

  // The second answer: the third becomes the awaited one; the note is gone.
  await expectText(page.locator("[data-author=assistant]"), "Second answer.", "the second answer arrives");
  if ((await count("chat-queued")) !== 0 || (await count("chat-waiting")) !== 1) throw new Error("after the second answer the third should be awaited with no queued note");

  // The last answer clears the waiting line.
  await expectText(page.locator("[data-author=assistant]"), "Third answer.", "the third answer arrives");
  await t("chat-waiting").waitFor({ state: "detached", timeout: 10_000 });
  console.log("PASS the waiting line clears with the last answer");

  if (errors.length) throw new Error(`the page threw: ${errors.join("; ")}`);
  console.log("web queued turns: PASS");
} catch (e) {
  console.error(`web queued turns: FAIL: ${e.message.slice(0, 800)}`);
  process.exitCode = 1;
} finally {
  await browser?.close();
  await cleanup(tok, home);
}
