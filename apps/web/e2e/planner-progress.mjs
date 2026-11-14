// Browser test of what the chat shows while the assistant works, against a real daemon and the
// workspace's `fake-agent` as the planner (never a real agent). The fake reads a line, sends a
// progress line (`say` with `progress`), keeps quiet for a few seconds, then answers, and reads the
// next line. While it is quiet the panel shows the waiting line and the progress line under it, a
// second message is kept and marked as waiting for the next turn, and the answer replaces the
// waiting line; the progress line stays in the record as a muted line.
// Usage: node e2e/planner-progress.mjs <tok binary> <fake-agent binary>   (cargo xtask e2e-web runs it)
// needs: fake-agent

import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { checks, cleanup, launch } from "./helpers.mjs";

const [tokArg, agentArg] = process.argv.slice(2);
if (!tokArg || !agentArg) {
  console.error("usage: node e2e/planner-progress.mjs <tok binary> <fake-agent binary>");
  process.exit(2);
}
const tokBin = resolve(tokArg);
const fakeAgent = resolve(agentArg);
const home = mkdtempSync(join(tmpdir(), "toknila-progress-"));
const run = (cmd, args) => execFileSync(cmd, args, { encoding: "utf8" });
const tok = (...args) => execFileSync(tokBin, args, { env: { ...process.env, TOKNILA_HOME: home }, encoding: "utf8" });
const git = (dir, ...args) =>
  run("git", ["-C", dir, "-c", "user.name=t", "-c", "user.email=t@example.com", "-c", "commit.gpgsign=false", ...args]);

// No apostrophe: the fake's steps sit in single-quoted TOML strings.
const PROGRESS = "Reading the code of the daemon, this takes a few minutes";

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
    "post", "/planner/say", JSON.stringify({ text: PROGRESS, progress: true }),
    "sleep", "7000",
    "post", "/planner/say", JSON.stringify({ text: "Two stories." }),
    "echo",
    "print", "planner done",
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
  const u = new URL(url);
  u.pathname = "/projects/WEB/planner";
  await page.goto(u.href);

  await t("planner-tab-terminal").click();
  await t("planner-profile").selectOption("fake-planner");
  await t("planner-tab-chat").click();

  // 1. The first message: the waiting line, and under it the progress line the assistant sent.
  await t("planner-ask-input").fill("Plan");
  await t("planner-ask-send").click();
  await expectText(t("chat-waiting-text"), "The assistant is answering", "the panel says the assistant is answering");
  await expectText(t("chat-progress"), PROGRESS, "the waiting line shows the assistant's last progress line");
  if ((await page.locator("[data-author=assistant]").count()) !== 0) throw new Error("the answer was there before the progress line was looked at");
  console.log("PASS the waiting line shows the progress line the assistant sent");

  // 2. A second message while the first is answered: kept, marked, not a new turn.
  if (await t("planner-ask-input").isDisabled()) throw new Error("the box does not take a second message while the assistant answers");
  await t("planner-ask-input").fill("and also the tests");
  await t("planner-ask-send").click();
  await expectText(t("chat-queued"), "sent, the assistant reads it after this answer", "the second message says it waits for the next turn");
  if ((await t("chat-message").and(page.locator("[data-author=person]")).count()) !== 2) throw new Error("both messages are in the record");
  console.log("PASS a second message is kept and marked as waiting for the next turn");

  // 3. The answer replaces the waiting line; the progress line stays in the record, muted.
  await t("chat-waiting").waitFor({ state: "detached", timeout: 30_000 });
  await expectText(page.locator("[data-author=assistant]"), "Two stories.", "the answer is an assistant bubble");
  await expectText(t("chat-progress-line"), PROGRESS, "the progress line stays in the record as a muted line");
  await t("chat-queued").waitFor({ state: "detached" });
  console.log("PASS the answer replaces the waiting line and the progress line stays in the record");

  // 4. The queued message reached the planner's terminal, to be read with the next turn.
  await t("planner-tab-terminal").click();
  await expectText(t("term-host"), "got:and also the tests", "the planner got the second message with the next turn");
  console.log("PASS the planner reads the second message with the next turn");

  if (errors.length) throw new Error(`the page threw: ${errors.join("; ")}`);
  console.log("web planner progress: PASS");
} catch (e) {
  console.error(`web planner progress: FAIL: ${e.message.slice(0, 800)}`);
  process.exitCode = 1;
} finally {
  await browser?.close();
  await cleanup(tok, home);
}
