// Browser test of editing dependencies on the diagram against a real daemon.
// Usage: node e2e/edges.mjs <path to tok binary>   (cargo xtask e2e-web runs it)
//
// With the mouse: drag from a card's handle onto another card adds a dependency; a click on a
// line selects it; dragging the selected line's end onto another card moves it; Delete and the
// right-click menu remove it. The daemon's refusals (a cycle, a card that has started) show as it
// worded them. The "Depends on" list in the card's Details does the same from the keyboard.
// SCREENSHOT_DIR, when set, receives pictures of the states.

import { execFileSync } from "node:child_process";
import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { checks, cleanup, launch } from "./helpers.mjs";

const tokBin = process.argv[2];
if (!tokBin) {
  console.error("usage: node e2e/edges.mjs <tok binary>");
  process.exit(2);
}
const home = mkdtempSync(join(tmpdir(), "toknila-edges-"));
const tok = (...args) =>
  execFileSync(tokBin, args, { env: { ...process.env, TOKNILA_HOME: home }, encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] });
/// What the daemon says when `tok` is refused: the same message the page must show.
const refusal = (...args) => {
  try {
    tok(...args);
  } catch (e) {
    return String(e.stderr).replace(/^error:\s*/i, "").trim();
  }
  throw new Error(`tok ${args.join(" ")} was not refused`);
};

let browser;
try {
  tok("project", "new", "EDG");
  tok("chain", "new", "work", "--project", "EDG", "--no-autostart");
  for (const t of ["Alpha", "Bravo", "Charlie", "Delta"]) tok("ticket", "new", "work", t);
  tok("start", "EDG-4");
  const { url } = JSON.parse(tok("open", "--json"));

  browser = await launch();
  const page = await browser.newPage({ viewport: { width: 1280, height: 1000 } });
  const errors = [];
  page.on("pageerror", (e) => errors.push(e.message));
  const { expectText } = checks(page);
  const shot = async (name) => {
    if (process.env.SCREENSHOT_DIR) await page.screenshot({ path: join(process.env.SCREENSHOT_DIR, `${name}.png`) });
  };

  const hit = (edge) => page.locator(`[data-testid=edge-hit][data-edge="${edge}"]`);
  const lineIs = (edge, selected) => page.locator(`g.edge[data-edge="${edge}"][data-selected=${selected}]`);
  const edgeCount = () => page.locator("[data-testid=edge-hit]").count();
  const centre = async (loc) => {
    const b = await loc.boundingBox();
    if (!b) throw new Error("not on the page");
    return { x: b.x + b.width / 2, y: b.y + b.height / 2 };
  };
  /// A drag with the mouse from the middle of `from` to the middle of `to`, in steps.
  const drag = async (from, to) => {
    const a = await centre(from);
    await page.mouse.move(a.x, a.y);
    await page.mouse.down();
    const b = await centre(to);
    await page.mouse.move((a.x + b.x) / 2, (a.y + b.y) / 2, { steps: 5 });
    await page.mouse.move(b.x, b.y, { steps: 5 });
    await page.mouse.up();
  };
  /// A click on the middle of a line (the line is a curve through the middle of its box).
  const clickLine = async (edge) => {
    const c = await centre(hit(edge));
    await page.mouse.click(c.x, c.y);
  };
  const handle = (key) => page.locator(`[data-testid=handle-${key}]`);
  const card = (key) => page.locator(`a.node[data-key="${key}"]`);
  const waitEdges = async (n, what) => {
    for (const until = Date.now() + 10_000; Date.now() < until; await page.waitForTimeout(100)) {
      if ((await edgeCount()) === n) return console.log(`PASS ${what}: ${n} line(s)`);
    }
    throw new Error(`${what}: expected ${n} line(s), got ${await edgeCount()}`);
  };

  await page.goto(url.replace("/?", "/projects/EDG/chains/work?"));
  await expectText(page.locator("[data-testid=chain-name-top]"), "work", "the chain is open");
  await waitEdges(0, "no dependencies yet");

  // Add: from Alpha's handle onto Bravo; Bravo comes after Alpha.
  await drag(handle("EDG-1"), card("EDG-2"));
  await hit("EDG-1>EDG-2").waitFor({ state: "attached" });
  await waitEdges(1, "dragging from a handle onto a card adds a dependency");
  await expectText(card("EDG-2").locator("[data-testid=status]"), "backlog", "the card that now waits is in backlog");
  if (!(await lineIs("EDG-1>EDG-2", true).count())) throw new Error("the new line is not selected");
  console.log("PASS the new line is selected");
  await shot("edge-added");

  // Move: select the line, drag its end from Bravo onto Charlie.
  await page.locator("[data-testid=canvas] .canvas-inner").click({ position: { x: 4, y: 4 } });
  await lineIs("EDG-1>EDG-2", false).waitFor({ state: "attached" });
  await clickLine("EDG-1>EDG-2");
  await lineIs("EDG-1>EDG-2", true).waitFor({ state: "attached" });
  console.log("PASS a click on a line selects it");
  await shot("edge-selected");
  await drag(page.locator("[data-testid=edge-end-to]"), card("EDG-3"));
  await hit("EDG-1>EDG-3").waitFor({ state: "attached" });
  await waitEdges(1, "dragging the end of the selected line onto another card moves it");
  if (await hit("EDG-1>EDG-2").count()) throw new Error("the old line is still there");
  await expectText(card("EDG-2").locator("[data-testid=status]"), "ready", "the card that no longer waits is ready again");

  // The CLI's move is the same one request: it lands whole, and a refusal (EDG-4 has started)
  // leaves the line where it was.
  tok("dep", "mv", "EDG-1", "EDG-3", "--new-to", "EDG-2");
  await hit("EDG-1>EDG-2").waitFor({ state: "attached" });
  if (await hit("EDG-1>EDG-3").count()) throw new Error("tok dep mv left the old line");
  refusal("dep", "mv", "EDG-1", "EDG-2", "--new-to", "EDG-4");
  tok("dep", "mv", "EDG-1", "EDG-2", "--new-to", "EDG-3");
  await hit("EDG-1>EDG-3").waitFor({ state: "attached" });
  await waitEdges(1, "tok dep mv moves a line whole and a refused move changes nothing");

  // A cycle is refused with the daemon's own message; nothing changes.
  const cycle = refusal("dep", "add", "EDG-3", "EDG-1");
  await drag(handle("EDG-3"), card("EDG-1"));
  await expectText(page.locator("[data-testid=edge-error]"), cycle, "a cycle is refused with the daemon's message");
  await shot("edge-refused");
  await waitEdges(1, "the refused line was not added");

  // A card that has started cannot be made to wait: the daemon says so.
  const started = refusal("dep", "add", "EDG-1", "EDG-4");
  await drag(handle("EDG-1"), card("EDG-4"));
  await expectText(page.locator("[data-testid=edge-error]"), started, "a card that has started is refused with the daemon's message");
  await waitEdges(1, "the refused line was not added to the started card");
  await page.locator("[data-testid=edge-error] button").click();
  await page.locator("[data-testid=edge-error]").waitFor({ state: "detached" });

  // Remove with the keyboard: select, Delete.
  await clickLine("EDG-1>EDG-3");
  await lineIs("EDG-1>EDG-3", true).waitFor({ state: "attached" });
  await page.keyboard.press("Delete");
  await waitEdges(0, "Delete removes the selected line");

  // Remove with the right-click menu.
  await drag(handle("EDG-1"), card("EDG-2"));
  await hit("EDG-1>EDG-2").waitFor({ state: "attached" });
  const c = await centre(hit("EDG-1>EDG-2"));
  await page.mouse.click(c.x, c.y, { button: "right" });
  await page.locator("[data-testid=menu-remove-edge]").click();
  await waitEdges(0, "the right-click menu removes a line");

  // The keyboard way: Depends on, in the card's Details.
  await card("EDG-2").click();
  const deps = page.locator("[data-testid=depends-on]");
  await deps.waitFor();
  await page.locator("[data-testid=dep-pick]").selectOption({ label: "EDG-1 · Alpha" });
  await page.locator("[data-testid=dep-add]").click();
  await expectText(deps, "EDG-1 · Alpha", "the list shows what the card waits for");
  await waitEdges(1, "Add dependency in the card's Details adds the line");
  await shot("depends-on");
  await page.locator("[data-testid^=dep-remove-]").first().click();
  await waitEdges(0, "Remove in the card's Details removes the line");
  // A cycle from the list too: Bravo after Alpha, then Alpha after Bravo is refused.
  tok("dep", "add", "EDG-1", "EDG-2");
  await card("EDG-1").click();
  await page.locator("[data-testid=dep-pick]").selectOption({ label: "EDG-2 · Bravo" });
  await page.locator("[data-testid=dep-add]").click();
  await expectText(page.locator("[data-testid=dep-error]"), "dependency cycle", "the list shows the daemon's refusal");

  if (errors.length) throw new Error(`the page threw: ${errors.join("; ")}`);
  console.log("web edges: PASS");
} catch (e) {
  console.error(`web edges: FAIL: ${e.message.slice(0, 800)}`);
  process.exitCode = 1;
} finally {
  await browser?.close();
  await cleanup(tok, home);
}
