// Browser test of the card's live terminal and actions against a real daemon.
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
import { checks, cleanup, launch, shipOf } from "./helpers.mjs";

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
  await expectText(status, "ready", "the card is ready, nothing started it");

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
  // The page settles on a new grid (the panel resized, the grid fitted, the daemon told).
  const settle = async () => {
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
    return g;
  };
  const panelHeight = () => page.locator("[data-testid=panel]").evaluate((el) => el.getBoundingClientRect().height);
  const diagramHeight = () => page.locator("[data-testid=panel-divider]").evaluate((el) => el.previousElementSibling.getBoundingClientRect().height);

  // The panel starts tall enough for an agent's terminal: about 45% of the window, and at 720px
  // more than the 12 rows it used to show (the divider gives more).
  {
    const g = await settle();
    const h = await panelHeight();
    if (h < 720 * 0.4 || h > 720 * 0.5) throw new Error(`the panel starts ${h}px tall in a 720px window, not about 45%`);
    if (g.rows <= 12) throw new Error(`the terminal shows only ${g.rows} rows at 720px`);
    console.log(`PASS the panel starts ${Math.round(h)}px of 720 and the terminal shows ${g.rows} rows`);
  }

  // Dragging the divider up gives the panel (and the terminal) more room; the grid is refitted,
  // the daemon told, and the last row stays inside the panel.
  const divider = page.locator("[data-testid=panel-divider]");
  const drag = async (dy) => {
    const box = await divider.boundingBox();
    const x = box.x + box.width / 2;
    const y = box.y + box.height / 2;
    await page.mouse.move(x, y);
    await page.mouse.down();
    await page.mouse.move(x, y + dy / 2, { steps: 5 });
    await page.mouse.move(x, y + dy, { steps: 5 });
    await page.mouse.up();
  };
  {
    const before = await settle();
    const hBefore = await panelHeight();
    await drag(-120);
    const after = await settle();
    const hAfter = await panelHeight();
    if (!(hAfter > hBefore + 100)) throw new Error(`dragging up moved the panel from ${hBefore} to ${hAfter}`);
    if (!(after.rows > before.rows)) throw new Error(`the terminal did not gain rows: ${before.rows} -> ${after.rows}`);
    if (after.lastBottom > after.panelBottom + 0.5) throw new Error(`after the drag the last row is cut: ${JSON.stringify(after)}`);
    const told = sizes[sizes.length - 1];
    if (told.rows !== after.rows) throw new Error(`after the drag the page shows ${after.rows} rows but told the daemon ${JSON.stringify(told)}`);
    console.log(`PASS dragging the divider up: panel ${Math.round(hBefore)} -> ${Math.round(hAfter)}px, rows ${before.rows} -> ${after.rows}, the last row inside the panel`);
  }

  // Both parts keep a minimum, however far the divider is dragged.
  await drag(-2000);
  if ((await diagramHeight()) < 150) throw new Error(`the diagram shrank to ${await diagramHeight()}px`);
  await drag(2000);
  if ((await panelHeight()) < 150) throw new Error(`the panel shrank to ${await panelHeight()}px`);
  console.log("PASS the diagram and the panel keep their minimum height");
  await drag(-3000);
  await drag(220);
  const kept = await panelHeight();

  // The height is remembered for this viewer: a reload brings it back.
  await page.reload();
  await tab("Terminal");
  await expectText(term, "agent ready", "after a reload the terminal is back");
  const restored = await panelHeight();
  if (Math.abs(restored - kept) > 1.5) throw new Error(`the panel was ${kept}px, after a reload ${restored}px`);
  console.log(`PASS the panel height is remembered across a reload (${Math.round(restored)}px)`);

  // Without storage (blocked site data) the page works and the divider still drags.
  {
    const blocked = await browser.newPage();
    await blocked.addInitScript(() => {
      Object.defineProperty(window, "localStorage", { get() { throw new Error("blocked"); } });
    });
    const failures = [];
    blocked.on("pageerror", (e) => failures.push(e.message));
    await blocked.goto(url.replace("/?", "/projects/WEB/chains/work/cards/WEB-1?"));
    await blocked.getByRole("tab", { name: "Terminal" }).click();
    const d = blocked.locator("[data-testid=panel-divider]");
    const box = await d.boundingBox();
    const h0 = await blocked.locator("[data-testid=panel]").evaluate((el) => el.getBoundingClientRect().height);
    await blocked.mouse.move(box.x + 5, box.y + 4);
    await blocked.mouse.down();
    await blocked.mouse.move(box.x + 5, box.y - 80, { steps: 5 });
    await blocked.mouse.up();
    const h1 = await blocked.locator("[data-testid=panel]").evaluate((el) => el.getBoundingClientRect().height);
    await blocked.close();
    if (failures.length) throw new Error(`with storage blocked the page threw: ${failures.join("; ")}`);
    if (!(h1 > h0 + 50)) throw new Error(`with storage blocked the divider did not drag: ${h0} -> ${h1}`);
    console.log("PASS the page and the divider work with storage blocked");
  }
  await page.setViewportSize({ width: 1280, height: 720 });
  await page.evaluate(() => {
    try { localStorage.removeItem("toknila.panelHeight"); } catch { /* none */ }
  });
  await page.reload();
  await tab("Terminal");
  await expectText(term, "agent ready", "back to the default height");

  for (const height of [720, 520, 900]) {
    await page.setViewportSize({ width: 1280, height });
    const g = await settle();
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

  // The agent hands in: the card is in review. Round 1 is the hand-in's own check (a card with a
  // ship is brought up to its base and checked); a Return with a comment is round 2.
  // The work is committed on the ship first: handing in a ship with nothing committed is refused.
  const ship = shipOf(tok, "WEB-1");
  writeFileSync(join(ship, "work.txt"), "work\n");
  git(ship, "add", "work.txt");
  git(ship, "commit", "--quiet", "-m", "work");
  tok("sim", "WEB-1", "finish", "first try");
  await expectText(status, "review", "the agent finished, the card is in review");
  await page.getByRole("button", { name: "Return", exact: true }).click();
  await page.getByLabel("What to change").fill("handle the error case");
  await page.getByRole("button", { name: "Send back" }).click();
  await expectText(status, "running", "Return sends the card back to the agent");
  await expectText(page.locator("[data-testid=rounds]"), "Round 2: you asked for changes — handle the error case", "the return is a review round");

  // Hand in again and accept: merged, and the run's end shows in the terminal. A hand-in after a
  // Return carries a new commit: the same commit is refused.
  writeFileSync(join(ship, "work2.txt"), "work\n");
  git(ship, "add", "work2.txt");
  git(ship, "commit", "--quiet", "-m", "work again");
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
