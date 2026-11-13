// Browser test of a question that carries choices, against a real daemon and the workspace's
// `fake-agent` (never a real agent), in a real git ship.
// Usage: node e2e/question-choices.mjs <tok binary> <fake-agent binary>   (cargo xtask e2e-web runs it)
// needs: fake-agent
//
// The agent asks with two choices. The inbox row shows them as buttons, the first marked
// recommended, with the field for another answer (the story's panel renders the same component,
// covered by the unit test). Pressing the recommended one answers at once: the agent receives that
// choice's text and the story runs again.

import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { checks, cleanup, launch } from "./helpers.mjs";

const [tokArg, agentArg] = process.argv.slice(2);
if (!tokArg || !agentArg) {
  console.error("usage: node e2e/question-choices.mjs <tok binary> <fake-agent binary>");
  process.exit(2);
}
const tokBin = resolve(tokArg);
const fakeAgent = resolve(agentArg);
const home = mkdtempSync(join(tmpdir(), "toknila-qc-"));
const run = (cmd, args, opts = {}) => execFileSync(cmd, args, { encoding: "utf8", ...opts });
const tok = (...args) => run(tokBin, args, { env: { ...process.env, TOKNILA_HOME: home } });
const git = (dir, ...args) =>
  run("git", ["-C", dir, "-c", "user.name=t", "-c", "user.email=t@example.com", "-c", "commit.gpgsign=false", ...args]);
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function until(what, f) {
  for (const stop = Date.now() + 20_000; Date.now() < stop; await sleep(100)) {
    if (f()) return;
  }
  throw new Error(`timed out waiting for ${what}`);
}

const status = (key) => JSON.parse(tok("show", key, "--json")).ticket.status;
const FIRST = "Hand in now; you check on the phone at review";
const SECOND = "Wait; I check on the phone first and tell you";

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
  const question = JSON.stringify({ event: "ask_human", text: "Can you check the page on your phone now?", choices: [FIRST, SECOND] });
  writeFileSync(
    join(home, "profiles", "chooser.toml"),
    `command = ['${fakeAgent}', ${steps("post", "/tickets/{ticket}/events", question, "echo", "hold")}]\n`,
  );
  tok("project", "new", "QC", "--repo", repo, "--base", "develop", "--profile", "chooser");
  tok("chain", "new", "work", "--project", "QC");
  tok("ticket", "new", "work", "Phone check");
  const { url } = JSON.parse(tok("open", "--json"));

  browser = await launch();
  const page = await browser.newPage();
  const errors = [];
  page.on("pageerror", (e) => errors.push(e.message));
  const { expectText } = checks(page);
  await page.goto(url);
  await page.locator("[data-testid=inbox-link]").click();
  const row = page.locator("[data-testid=inbox] li[data-kind=question][data-key=QC-1]");

  await expectText(row.locator("[data-testid=choice-0]"), FIRST, "the first choice is a button");
  await expectText(row.locator("[data-testid=choice-0]"), "recommended", "and is marked recommended");
  await expectText(row.locator("[data-testid=choice-1]"), SECOND, "the second choice is a button");
  if ((await row.locator("[data-testid=choice-1][data-recommended=true]").count()) !== 0) {
    throw new Error("only the first choice is recommended");
  }
  await row.getByLabel("Reply to QC-1").waitFor({ timeout: 10_000 });
  console.log("PASS the inbox row shows the choices as buttons and the field for another answer");

  // tok show numbers them, the same way.
  const shown = tok("show", "QC-1");
  if (!shown.includes(`1. ${FIRST} (recommended)`) || !shown.includes(`2. ${SECOND}`)) {
    throw new Error(`tok show: ${shown}`);
  }
  console.log("PASS tok show prints the choices numbered");

  // Pressing the recommended one answers at once.
  await row.locator("[data-testid=choice-0]").click();
  await row.waitFor({ state: "detached", timeout: 10_000 });
  await until("the agent got the choice", () => tok("logs", "QC-1").includes(FIRST));
  await until("QC-1 running", () => status("QC-1") === "running");
  console.log("PASS pressing the recommended choice answered at once and the agent got its text");

  if (errors.length) throw new Error(`the page threw: ${errors.join("; ")}`);
  console.log("web question choices: PASS");
} catch (e) {
  console.error(`web question choices: FAIL: ${e.message.slice(0, 600)}`);
  process.exitCode = 1;
} finally {
  await browser?.close();
  await cleanup(tok, home);
}
