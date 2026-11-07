// Browser test of the card's live terminal and actions against a real daemon (P1-09).
// Usage: node e2e/terminal.mjs <tok binary> <fake-agent binary>   (cargo xtask e2e-web runs it)
//
// A project whose profile runs `fake-agent` in a real git ship: start the card from the page,
// see the terminal's live output, type a line and see the agent echo it, send an instruction
// comment, bring the card to review (the agent's `finished` event, as the CLI sends it), return it
// with a comment (a review round shows, from the daemon), finish again and Accept: the card is merged and the
// terminal says the run ended.

import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { checks, cleanup, launch } from "./helpers.mjs";

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
  writeFileSync(join(home, "profiles", "fake.toml"), `command = ['${fakeAgent}', 'print', 'agent ready', 'echo', 'size', 'hold']\n`);
  tok("project", "new", "WEB", "--repo", repo, "--base", "develop", "--profile", "fake");
  tok("chain", "new", "work", "--project", "WEB", "--no-autostart");
  tok("ticket", "new", "work", "Echo task", "--prompt", "Say hello.");
  const { url } = JSON.parse(tok("open", "--json"));

  browser = await launch();
  const page = await browser.newPage();
  const errors = [];
  page.on("pageerror", (e) => errors.push(e.message));
  const { expectText } = checks(page);
  const sizes = [];
  page.on("request", (r) => {
    if (r.method() === "POST" && r.url().endsWith("/runner/size")) sizes.push(r.postDataJSON());
  });
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

  // The agent's terminal follows the panel: the size it reports after the line is the grid the
  // page fitted, not the daemon's default of 160x40.
  await expectText(term, "size=", "the agent reports its terminal size");
  const reported = (await term.innerText()).match(/size=(\d+)x(\d+)/);
  if (!reported) throw new Error("no size line in the terminal");
  const [, cols, rows] = reported;
  const fitted = await page.evaluate(() => {
    const screen = document.querySelector("[data-testid=term-host] .xterm-screen");
    return screen ? { width: screen.clientWidth, height: screen.clientHeight } : null;
  });
  if (`${cols}x${rows}` === "160x40") throw new Error(`the terminal kept the default size (${fitted && fitted.width}px wide)`);

  // The whole grid is inside the panel, at two window heights and after resizing the window: the
  // last row (where Claude Code draws its prompt and footer) is not cut by the panel's edge, and
  // the agent's terminal got the same number of rows.
  const grid = () =>
    page.evaluate(() => {
      const rows = [...document.querySelectorAll("[data-testid=term-host] .xterm-rows > div")];
      const edge = (sel) => document.querySelector(sel).getBoundingClientRect();
      const last = rows[rows.length - 1].getBoundingClientRect();
      const screen = edge("[data-testid=term-host] .xterm-screen");
      return {
        rows: rows.length,
        lastTop: last.top,
        lastBottom: last.bottom,
        panelBottom: edge("[data-testid=panel]").bottom,
        hostBottom: edge("[data-testid=term-host]").bottom,
        rowHeight: last.height,
        padBottom: Number.parseFloat(getComputedStyle(document.querySelector("[data-testid=term-host]")).paddingBottom),
        screenBottom: screen.bottom,
        screenRight: screen.right,
        hostRight: edge("[data-testid=term-host]").right,
      };
    });
  for (const height of [720, 520, 900]) {
    await page.setViewportSize({ width: 1280, height });
    // The page settles on the new grid (the panel resized, the grid fitted, the daemon told).
    let g;
    const settled = () =>
      g.lastBottom <= g.panelBottom + 0.5 &&
      g.hostBottom - g.padBottom - g.lastBottom < g.rowHeight + 0.5 &&
      sizes.length > 0 &&
      sizes[sizes.length - 1].rows === g.rows;
    for (const until = Date.now() + 8000; Date.now() < until; await page.waitForTimeout(100)) {
      g = await grid();
      if (settled()) break;
    }
    if (g.lastBottom > g.panelBottom + 0.5 || g.lastBottom > g.hostBottom + 0.5 || g.screenRight > g.hostRight + 0.5) {
      throw new Error(`at window height ${height} the grid is cut: ${JSON.stringify(g)}`);
    }
    // Less than one row of room is left under the last row: no row that would fit was left out.
    if (g.hostBottom - g.padBottom - g.lastBottom >= g.rowHeight + 0.5) {
      throw new Error(`at window height ${height} a row was left out: ${JSON.stringify(g)}`);
    }
    const told = sizes[sizes.length - 1];
    if (!told || told.rows !== g.rows) {
      throw new Error(`at window height ${height} the page shows ${g.rows} rows but told the daemon ${JSON.stringify(told)}`);
    }
    console.log(`PASS at window height ${height} the last of ${g.rows} rows ends at ${g.lastBottom.toFixed(1)}, inside the panel (${g.panelBottom.toFixed(1)})`);
    console.log(`PASS the daemon was told ${told.cols}x${told.rows} for ${g.rows} visible rows`);
  }

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
  // The work is committed on the ship first: handing in a ship with nothing committed is refused.
  const ship = JSON.parse(tok("show", "WEB-1", "--json")).ship.path;
  writeFileSync(join(ship, "work.txt"), "work\n");
  git(ship, "add", "work.txt");
  git(ship, "commit", "--quiet", "-m", "work");
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
    console.error(`WEB-1: ${shown.status} ${JSON.stringify(shown.status_reason)}`);  } catch {
    /* the daemon is not reachable */
  }
  process.exitCode = 1;
} finally {
  await browser?.close();
  await cleanup(tok, home);
}
