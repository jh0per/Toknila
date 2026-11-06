// Browser test of the card's live terminal and actions against a real daemon (P1-09).
// Usage: node e2e/terminal.mjs <tok binary> <fake-agent binary>   (cargo xtask e2e-web runs it)
//
// A project whose profile runs `fake-agent` in a real git ship: start the card from the page,
// see the terminal's live output, type a line and see the agent echo it, send an instruction
// comment, bring the card to review (the agent's `finished` event, as the CLI sends it), return it
// with a comment (a review round shows, from the daemon), finish again and Accept: the card is merged and the
// terminal says the run ended.

import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { checks, launch } from "./helpers.mjs";

const [tokArg, agentArg] = process.argv.slice(2);
if (!tokArg || !agentArg) {
  console.error("usage: node e2e/terminal.mjs <tok binary> <fake-agent binary>");
  process.exit(2);
}
// The agent runs in a ship's folder, so its path must not depend on the working directory.
const tokBin = resolve(tokArg);
const fakeAgent = resolve(agentArg);
const home = mkdtempSync(join(tmpdir(), "toknila-term-"));
const run = (cmd, args, opts = {}) => execFileSync(cmd, args, { encoding: "utf8", ...opts });
const tok = (...args) => run(tokBin, args, { env: { ...process.env, TOKNILA_HOME: home } });
const git = (dir, ...args) =>
  run("git", ["-C", dir, "-c", "user.name=t", "-c", "user.email=t@example.com", "-c", "commit.gpgsign=false", ...args]);

let browser;
try {
  // A repository whose only branch is develop, and a profile running the fake agent: it prints a
  // line, echoes one line it is typed, then waits to be stopped.
  const repo = join(home, "repo");
  mkdirSync(repo, { recursive: true });
  git(repo, "init", "--quiet");
  git(repo, "symbolic-ref", "HEAD", "refs/heads/develop");
  writeFileSync(join(repo, "README.md"), "hi\n");
  git(repo, "add", "README.md");
  git(repo, "commit", "--quiet", "-m", "init");
  mkdirSync(join(home, "profiles"), { recursive: true });
  writeFileSync(join(home, "profiles", "fake.toml"), `command = ['${fakeAgent}', 'print', 'agent ready', 'echo', 'hold']\n`);
  tok("project", "new", "WEB", "--repo", repo, "--base", "develop", "--profile", "fake");
  tok("chain", "new", "work", "--project", "WEB", "--no-autostart");
  tok("ticket", "new", "work", "Echo task", "--prompt", "Say hello.");
  const { url } = JSON.parse(tok("open", "--json"));

  browser = await launch();
  const page = await browser.newPage();
  const errors = [];
  page.on("pageerror", (e) => errors.push(e.message));
  const { expectText } = checks(page);
  const status = page.locator('a[data-key="WEB-1"] [data-testid=status]');
  const term = page.locator("[data-testid=term-host]");
  const tab = (name) => page.getByRole("tab", { name }).click();

  await page.goto(url.replace("/?", "/projects/WEB/chains/work/cards/WEB-1?"));
  await expectText(status, "todo · ready", "the card is ready, nothing started it");

  // Start from the action bar: the runner starts and its output shows live in the terminal.
  await page.getByRole("button", { name: "Start", exact: true }).click();
  await expectText(status, "running", "Start runs the card");
  await tab("Terminal");
  await expectText(term, "agent ready", "the terminal shows the agent's live output");
  await expectText(page.locator("[data-testid=term-status]"), "live", "the terminal is live");

  // Type a line: it goes to the agent, which echoes it back through the stream.
  await term.click();
  await page.keyboard.type("hello agent");
  await page.keyboard.press("Enter");
  await expectText(term, "got:hello agent", "a typed line reaches the agent and its echo comes back");

  // A reload gets the same output from the scrollback.
  await page.reload();
  await tab("Terminal");
  await expectText(term, "got:hello agent", "after a reload the scrollback has the output");

  // Send an instruction comment from the Details tab.
  await tab("Details");
  await page.getByLabel("Comment", { exact: true }).fill("also cover the empty case");
  await page.getByRole("button", { name: "Send instruction" }).click();
  await expectText(page.locator("[data-testid=comments]"), "also cover the empty case", "the instruction is on the card");
  await expectText(page.locator("[data-testid=comments] [data-kind=instruction]"), "instruction", "it is an instruction comment");

  // The agent hands in: the card is in review. Return it with a comment: round 1.
  tok("sim", "WEB-1", "finish", "first try");
  await expectText(status, "review", "the agent finished, the card is in review");
  await page.getByRole("button", { name: "Return", exact: true }).click();
  await page.getByLabel("What to change").fill("handle the error case");
  await page.getByRole("button", { name: "Send back" }).click();
  await expectText(status, "running", "Return sends the card back to the agent");
  await expectText(page.locator("[data-testid=rounds]"), "Round 1: you asked for changes — handle the error case", "the return is a review round");

  // Hand in again and accept: merged, and the run's end shows in the terminal.
  tok("sim", "WEB-1", "finish", "second try");
  await expectText(status, "review", "the agent finished again");
  await page.getByRole("button", { name: "Accept", exact: true }).click();
  await expectText(status, "merged", "Accept merges the card");
  await tab("Terminal");
  await expectText(page.locator("[data-testid=term-status]"), "ended", "the terminal says the run ended");
  if (errors.length) throw new Error(`the page threw: ${errors.join("; ")}`);
  console.log("web terminal: PASS");
} catch (e) {
  console.error(`web terminal: FAIL: ${e.message.slice(0, 600)}`);
  // What a CI log needs to tell a slow runner from a broken one: the ticket and its journal.
  try {
    const shown = JSON.parse(tok("show", "WEB-1", "--json")).ticket;
    console.error(`WEB-1: ${shown.status} ${JSON.stringify(shown.status_reason)}`);
  } catch {
    /* the daemon is not reachable */
  }
  process.exitCode = 1;
} finally {
  await browser?.close();
  try {
    tok("daemon", "stop");
  } catch {
    /* already stopped */
  }
  // Best effort: the daemon may still be closing files of the ship it removed.
  try {
    rmSync(home, { recursive: true, force: true, maxRetries: 5, retryDelay: 200 });
  } catch {
    /* a temp folder left behind is not a failure of the test */
  }
}
