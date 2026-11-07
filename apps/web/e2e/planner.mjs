// Browser test of the planner entry against a real daemon (P2-03). No real agent is launched:
// the planner profile runs `fake-agent`, which plays what the planner's tools do with its run
// token (it creates a chain and two cards over the planner routes).
// Usage: node e2e/planner.mjs <tok binary> <fake-agent binary>   (cargo xtask e2e-web runs it)
//
// The sidebar has a "Planner" entry at the top of the project's section; it opens the planner's
// panel (not running, Start). Start runs the planner in the project's repository: its terminal is
// live, the diagram above shows the chain and cards the planner makes as they happen, and what is
// typed in the terminal reaches it. Stop ends it.

import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { checks, cleanup, launch } from "./helpers.mjs";

const [tokArg, agentArg] = process.argv.slice(2);
if (!tokArg || !agentArg) {
  console.error("usage: node e2e/planner.mjs <tok binary> <fake-agent binary>");
  process.exit(2);
}
const tokBin = resolve(tokArg);
const fakeAgent = resolve(agentArg);
const home = mkdtempSync(join(tmpdir(), "toknila-planner-"));
const run = (cmd, args, opts = {}) => execFileSync(cmd, args, { encoding: "utf8", ...opts });
const tok = (...args) => run(tokBin, args, { env: { ...process.env, TOKNILA_HOME: home } });
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
    "post", "/planner/chains", JSON.stringify({ name: "roadmap", goal: "planned by the fake planner" }),
    "post", "/planner/chains/roadmap/cards", JSON.stringify({ title: "Design the list", prompt: "p", acceptance: ["a"] }),
    "post", "/planner/chains/roadmap/cards", JSON.stringify({ title: "Build the list", prompt: "p" }),
    "post", "/tickets/WEB-1/actions/start", "{}",
    // The replies are long and scroll; the line the page waits for comes last.
    "print", "planner ready",
    "echo", "hold",
  ];
  const list = steps.map((s) => `'${s}'`);
  writeFileSync(join(home, "profiles", "fake-planner.toml"), `role = 'planner'\ncommand = ['${fakeAgent}', ${list.join(", ")}]\nstartup_secs = 0\n`);
  tok("project", "new", "WEB", "--repo", repo, "--base", "develop");
  const { url } = JSON.parse(tok("open", "--json"));

  browser = await launch();
  const page = await browser.newPage();
  const errors = [];
  page.on("pageerror", (e) => errors.push(e.message));
  const { expectText, expectPath } = checks(page);
  const term = page.locator("[data-testid=term-host]");

  await page.goto(url);
  // The entry is at the top of the project's section, above its chains.
  const entry = page.locator("[data-testid=chains] li").first();
  await expectText(entry, "Planner", "the planner entry is first in the project's section");
  if ((await page.locator("[data-testid=planner-entry]").count()) !== 1) throw new Error("expected one planner entry");

  await page.locator("[data-testid=planner-link]").click();
  await expectPath("/projects/WEB/planner", "selecting it opens the planner's address");
  await expectText(page.locator("[data-testid=planner-state]"), "not running", "the planner is not running");
  await expectText(page.locator("[data-testid=planner-empty]"), "No chains yet", "no chain to show yet");
  await expectText(page.locator("[data-testid=placeholder]"), "has not run yet", "its terminal has nothing yet");

  // Start with the planner profile of this test (the built-in one would launch the real agent).
  await page.locator("[data-testid=planner-profile]").selectOption("fake-planner");
  await page.locator("[data-testid=planner-start]").click();
  await expectText(page.locator("[data-testid=planner-state]"), "running", "Start runs the planner");
  await expectText(page.locator("[data-testid=planner-stop]"), "Stop", "Stop replaces Start");
  await expectText(term, "planner ready", "the terminal shows the planner's live output");
  await expectText(page.locator("[data-testid=term-status]"), "live", "the terminal is live");

  // What the planner makes shows in the diagram above, live, and in the sidebar.
  await expectText(page.locator("[data-testid=planner-chain]"), "roadmap", "the diagram follows the chain the planner made");
  await expectText(page.locator("a[data-key=WEB-1]"), "Design the list", "its first card is on the diagram");
  await expectText(page.locator("a[data-key=WEB-2]"), "Build the list", "its second card is on the diagram");
  await expectText(page.locator("[data-testid=chains] li"), "roadmap", "the chain is in the sidebar");
  // The planner's attempt to start a card was refused: nothing runs.
  await expectText(page.locator("a[data-key=WEB-1] [data-testid=status]"), "ready", "the refused start left the card ready");

  // The owner types to the planner in its terminal.
  await term.click();
  await page.keyboard.type("hello planner");
  await page.keyboard.press("Enter");
  await expectText(term, "got:hello planner", "what is typed reaches the planner");

  // A reload keeps the planner running and its output.
  await page.reload();
  await expectText(page.locator("[data-testid=planner-state]"), "running", "after a reload it still runs");
  await expectText(term, "planner ready", "after a reload the scrollback has the output");

  // Stop ends it, and the panel offers Start again.
  await page.locator("[data-testid=planner-stop]").click();
  await expectText(page.locator("[data-testid=planner-state]"), "not running", "Stop ends the planner");
  await expectText(page.locator("[data-testid=term-status]"), "ended", "the terminal says it ended");
  await expectText(page.locator("[data-testid=planner-start]"), "Start", "Start is back");
  if (errors.length) throw new Error(`the page threw: ${errors.join("; ")}`);
  console.log("web planner: PASS");
} catch (e) {
  console.error(`web planner: FAIL: ${e.message.slice(0, 600)}`);
  process.exitCode = 1;
} finally {
  await browser?.close();
  await cleanup(tok, home);
}
