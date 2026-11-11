// Browser test of what agents ask and a person answers from the Inbox, against a real daemon
// and the workspace's `fake-agent` (never a real agent), in a real git ship.
// Usage: node e2e/questions.mjs <tok binary> <fake-agent binary>   (cargo xtask e2e-web runs it)
// needs: fake-agent
//
// Three cards run: two play an agent's permission hook (the call waits for the person), one shows
// a screen and no sign of life. From the inbox: Allow one permission, Deny the other with a
// comment (the hook gets each decision, the cards are Running at once), and answer the waiting
// screen with typed text (the agent receives it). Two more: a folder trust dialog with unnumbered
// options is answered with the arrow quick key and Enter (the agent reads "trust=yes"), and a
// reviewer that ends its turns without a verdict becomes a review item with its screen, from which
// the person can Run the reviewer again.

import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { checks, cleanup, launch, shipOf } from "./helpers.mjs";

const [tokArg, agentArg] = process.argv.slice(2);
if (!tokArg || !agentArg) {
  console.error("usage: node e2e/questions.mjs <tok binary> <fake-agent binary>");
  process.exit(2);
}
const tokBin = resolve(tokArg);
const fakeAgent = resolve(agentArg);
const home = mkdtempSync(join(tmpdir(), "toknila-q-"));
const run = (cmd, args, opts = {}) => execFileSync(cmd, args, { encoding: "utf8", ...opts });
const tok = (...args) => run(tokBin, args, { env: { ...process.env, TOKNILA_HOME: home } });
const git = (dir, ...args) =>
  run("git", ["-C", dir, "-c", "user.name=t", "-c", "user.email=t@example.com", "-c", "commit.gpgsign=false", ...args]);
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const logs = (key) => tok("logs", key);

async function until(what, f) {
  for (const stop = Date.now() + 20_000; Date.now() < stop; await sleep(100)) {
    if (f()) return;
  }
  throw new Error(`timed out waiting for ${what}`);
}

const status = (key) => JSON.parse(tok("show", key, "--json")).ticket.status;

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
  const ask = (tool) =>
    steps(
      "post", "/tickets/{ticket}/activity", JSON.stringify({ signal: "hook:pre_tool_use", tool }),
      "post", "/tickets/{ticket}/permission", "{}",
      "hold",
    );
  writeFileSync(join(home, "profiles", "allow.toml"), `command = ['${fakeAgent}', ${ask("Bash: cargo test -p core")}]\n`);
  writeFileSync(join(home, "profiles", "deny.toml"), `command = ['${fakeAgent}', ${ask("Bash: rm -rf build\nrm -rf dist")}]\n`);
  writeFileSync(
    join(home, "profiles", "screen.toml"),
    `command = ['${fakeAgent}', ${steps("print", "Do you trust this folder?", "print", "1. Yes  2. No", "echo", "hold")}]\n` +
      "startup_secs = 1\n[hooks]\nenabled = true\n",
  );
  writeFileSync(
    join(home, "profiles", "trust.toml"),
    `command = ['${fakeAgent}', ${steps("trust", "hold")}]\nstartup_secs = 1\n[hooks]\nenabled = true\n`,
  );
  // A worker that only holds, and a reviewer that ends two turns (the Stop hook) without a verdict.
  writeFileSync(join(home, "profiles", "plain.toml"), `command = ['${fakeAgent}', ${steps("print", "working", "hold")}]\n`);
  writeFileSync(
    join(home, "profiles", "plain-reviewer.toml"),
    `command = ['${fakeAgent}', ${steps("print", "I could not run the checks", "stop", "sleep", "300", "stop", "hold")}]\n`,
  );
  tok("project", "new", "QA", "--repo", repo, "--base", "develop", "--profile", "allow");
  tok("project", "set", "QA", "--parallel", "6");
  tok("chain", "new", "work", "--project", "QA");
  tok("ticket", "new", "work", "Allowed");
  tok("ticket", "new", "work", "Denied", "--profile", "deny");
  tok("ticket", "new", "work", "Silent", "--profile", "screen");
  tok("ticket", "new", "work", "Trusting", "--profile", "trust");
  tok("ticket", "new", "work", "Reviewed", "--profile", "plain");
  const { url } = JSON.parse(tok("open", "--json"));

  browser = await launch();
  const page = await browser.newPage();
  const errors = [];
  page.on("pageerror", (e) => errors.push(e.message));
  const { expectText } = checks(page);
  await page.goto(url);
  await page.locator("[data-testid=inbox-link]").click();
  const inbox = page.locator("[data-testid=inbox]");
  const row = (key) => inbox.locator(`li[data-kind=question][data-key="${key}"]`);

  // What is asked is shown with the tool, not just "needs your permission".
  await expectText(row("QA-1"), "Bash: cargo test -p core", "the permission names the command");
  await expectText(row("QA-2"), "Bash: rm -rf build", "the second permission names its command");
  // The list line is one line; the whole command, with its lines, is behind a disclosure.
  const detail = row("QA-2").locator("[data-testid=question-detail]");
  await detail.locator("summary").click();
  await expectText(detail.locator("pre"), "rm -rf build\nrm -rf dist", "the full command keeps its lines");
  await expectText(detail.locator("summary"), "Show the full command", "the disclosure is worded plainly");
  if ((await row("QA-1").locator("[data-testid=question-detail]").count()) !== 0) {
    throw new Error("a one-line command has no disclosure");
  }
  await expectText(row("QA-3"), "The agent waits on its screen", "the silent agent is listed");
  await expectText(row("QA-3").locator("[data-testid=screen]"), "Do you trust this folder?", "its screen lines are shown");

  // Allow: the card is Running at once and the hook got the decision.
  await row("QA-1").locator("[data-testid=answer-allow]").click();
  await row("QA-1").waitFor({ state: "detached", timeout: 10_000 });
  await until("QA-1 running", () => status("QA-1") === "running");
  await until("the allow reached the hook", () => logs("QA-1").includes('"decision":"allow"'));
  console.log("PASS Allow reached the agent and the card is running");

  // Deny with a comment.
  await page.getByLabel("Why QA-2 is denied").fill("use cargo clean instead");
  await row("QA-2").locator("[data-testid=answer-deny]").click();
  await row("QA-2").waitFor({ state: "detached", timeout: 10_000 });
  await until("QA-2 running", () => status("QA-2") === "running");
  await until("the deny reached the hook", () =>
    logs("QA-2").includes('"decision":"deny","message":"use cargo clean instead"'),
  );
  console.log("PASS Deny with a comment reached the agent");

  // The screen is answered with text typed to the agent.
  await page.getByLabel("Type to QA-3").fill("yes");
  await row("QA-3").getByRole("button", { name: "Send" }).click();
  await until("the agent got the text", () => logs("QA-3").includes("got:yes"));
  await expectText(row("QA-3").locator("[data-testid=screen]"), "got:yes", "the changed screen is listed again");
  console.log("PASS text typed from the inbox reached the agent's terminal");

  // The trust dialog's options are not numbered: the arrow quick key and Enter choose "Yes". The
  // fake agent reads the arrow from a Unix pty only (a Windows console line read never sees it).
  await expectText(row("QA-4").locator("[data-testid=screen]"), "Yes, I trust this folder", "the trust dialog is shown");
  if (process.platform !== "win32") {
    await row("QA-4").locator("[data-testid=answer-down]").click();
    await row("QA-4").locator("[data-testid=answer-enter]").click();
    await until("the agent chose Yes", () => logs("QA-4").includes("trust=yes"));
    console.log("PASS the arrow quick key and Enter answered the trust dialog");
  }

  // A reviewer that ends its turns without a verdict: the card goes to the person.
  tok("review-mode", "agent", "--ticket", "QA-5");
  await until("QA-5 running", () => status("QA-5") === "running");
  const ship = shipOf(tok, "QA-5");
  writeFileSync(join(ship, "work.txt"), "work\n");
  git(ship, "add", "work.txt");
  git(ship, "commit", "--quiet", "-m", "work");
  tok("sim", "QA-5", "finish", "done");
  const review = inbox.locator('li[data-kind=review][data-key="QA-5"]');
  await expectText(review, "The reviewer gave no verdict", "the inbox says the reviewer gave no verdict");
  await expectText(review.locator("[data-testid=screen]"), "I could not run the checks", "with the reviewer's screen");
  for (const id of ["review-accept", "review-run-again", "review-return-open"]) {
    await review.locator(`[data-testid=${id}]`).waitFor({ timeout: 10_000 });
  }
  console.log("PASS Accept, Return and Run the reviewer again are offered");
  await review.locator("[data-testid=review-run-again]").click();
  await until("the reviewer runs again", () => JSON.parse(tok("show", "QA-5", "--json")).review.awaiting?.by === "agent");
  await expectText(review, "The reviewer gave no verdict", "and gives no verdict again");

  // The journal says who answered and for which role.
  const origin = new URL(url).origin;
  const token = new URL(url).searchParams.get("token");
  const res = await fetch(`${origin}/v1/events?since=0&limit=10000`, { headers: { Authorization: `Bearer ${token}` } });
  const answered = (await res.json()).filter((e) => e.kind === "question_answered");
  const seen = answered.map((e) => `${e.actor}:${e.payload.key}:${e.payload.role}:${e.payload.answer}`).sort();
  const want = ["human:QA-1:worker:allow", "human:QA-2:worker:deny", "human:QA-3:worker:text"];
  if (process.platform !== "win32") want.push("human:QA-4:worker:down", "human:QA-4:worker:enter");
  want.sort();
  if (JSON.stringify(seen) !== JSON.stringify(want)) throw new Error(`journal: ${JSON.stringify(seen)}`);
  console.log("PASS the journal records who answered, for which role");
  if (errors.length) throw new Error(`the page threw: ${errors.join("; ")}`);
  console.log("web questions: PASS");
} catch (e) {
  console.error(`web questions: FAIL: ${e.message.slice(0, 600)}`);
  process.exitCode = 1;
} finally {
  await browser?.close();
  await cleanup(tok, home);
}
