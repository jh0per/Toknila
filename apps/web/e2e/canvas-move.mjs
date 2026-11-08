// Browser test of moving cards and of the Start and end nodes against a real daemon.
// Usage: node e2e/canvas-move.mjs <path to tok binary>   (cargo xtask e2e-web runs it)
//
// A card is dragged by its body and keeps its place after a reload; a press that barely moves is
// a click and opens the card; the arrow keys move the open card; Arrange puts every card back.
// The diagram has a Start node with the goal and a Finish node, and a thin implicit line from
// Start to every card with nothing before it and from every card nothing waits for to the end.
// SCREENSHOT_DIR, when set, receives pictures.

import { execFileSync } from "node:child_process";
import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { checks, cleanup, launch } from "./helpers.mjs";

const tokBin = process.argv[2];
if (!tokBin) {
  console.error("usage: node e2e/canvas-move.mjs <tok binary>");
  process.exit(2);
}
const home = mkdtempSync(join(tmpdir(), "toknila-move-"));
const tok = (...args) =>
  execFileSync(tokBin, args, { env: { ...process.env, TOKNILA_HOME: home }, encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] });
const savedPos = (key) => JSON.parse(tok("chain", "show", "flow", "--json")).tickets.find((t) => t.key === key)?.pos;

let browser;
try {
  tok("project", "new", "MOV");
  tok("chain", "new", "flow", "--project", "MOV", "--goal", "Make the thing work", "--no-autostart");
  for (const t of ["Alpha", "Bravo", "Charlie"]) tok("ticket", "new", "flow", t);
  tok("dep", "add", "MOV-1", "MOV-2");
  const { url } = JSON.parse(tok("open", "--json"));

  browser = await launch();
  const page = await browser.newPage({ viewport: { width: 1280, height: 900 } });
  const errors = [];
  page.on("pageerror", (e) => errors.push(e.message));
  const { expectText } = checks(page);
  const shot = async (name) => {
    if (process.env.SCREENSHOT_DIR) await page.screenshot({ path: join(process.env.SCREENSHOT_DIR, `${name}.png`) });
  };
  const open = () => page.goto(url.replace("/?", "/projects/MOV/chains/flow?"));
  const card = (key) => page.locator(`a.node[data-key="${key}"]`);
  const check = (ok, what, detail = "") => {
    if (!ok) throw new Error(`${what} ${detail}`);
    console.log(`PASS ${what}`);
  };
  /// Where a card sits on the canvas, in canvas pixels.
  const where = async (key) => {
    const [c, i] = [await card(key).boundingBox(), await page.locator(".canvas-inner").boundingBox()];
    if (!c || !i) throw new Error(`${key} is not on the page`);
    return { x: Math.round(c.x - i.x), y: Math.round(c.y - i.y) };
  };
  const centre = async (loc) => {
    const b = await loc.boundingBox();
    return { x: b.x + b.width / 2, y: b.y + b.height / 2 };
  };
  const drag = async (key, dx, dy) => {
    const a = await centre(card(key));
    await page.mouse.move(a.x, a.y);
    await page.mouse.down();
    await page.mouse.move(a.x + dx / 2, a.y + dy / 2, { steps: 4 });
    await page.mouse.move(a.x + dx, a.y + dy, { steps: 4 });
    await page.mouse.up();
  };
  const path = () => new URL(page.url()).pathname;
  const waitPos = async (key, want, what) => {
    for (const until = Date.now() + 10_000; Date.now() < until; await page.waitForTimeout(100)) {
      const p = savedPos(key);
      if (want === undefined ? p === undefined : p && p.x === want.x && p.y === want.y) return console.log(`PASS ${what}`);
    }
    throw new Error(`${what}: the daemon has ${JSON.stringify(savedPos(key))}`);
  };

  await open();
  await card("MOV-3").waitFor();

  // Start and the end of the diagram.
  const start = page.locator("[data-testid=start-node]");
  const end = page.locator("[data-testid=end-node]");
  await expectText(start, "Start", "the diagram begins with a Start node");
  await expectText(start, "Make the thing work", "the Start node shows the chain's goal");
  await expectText(end, "Finish", "a chain without a release ends with a Finish node");
  const [s, a, f] = [await start.boundingBox(), await card("MOV-1").boundingBox(), await end.boundingBox()];
  check(s.x < a.x && a.x < f.x, "Start is left of the cards and Finish right of them");
  // Alpha and Charlie have nothing before them; Bravo and Charlie nothing after them: four lines.
  const implicit = await page.locator("[data-implicit]").evaluateAll((els) => els.map((e) => e.getAttribute("data-implicit")).sort());
  check(
    JSON.stringify(implicit) === JSON.stringify(["MOV-2>Finish", "MOV-3>Finish", "Start>MOV-1", "Start>MOV-3"]),
    "implicit lines join Start to the first cards and the last cards to Finish", JSON.stringify(implicit),
  );
  const widths = await page.evaluate(() => ({
    implicit: getComputedStyle(document.querySelector(".edge.implicit .line")).strokeWidth,
    real: getComputedStyle(document.querySelector(".edge:not(.implicit) .line")).strokeWidth,
  }));
  check(parseFloat(widths.implicit) < parseFloat(widths.real), "implicit lines are thinner than dependencies", JSON.stringify(widths));
  check((await page.locator("[data-testid=edge-hit]").count()) === 1, "only the real dependency can be selected");
  const line = page.locator(".edge.implicit .line").first();
  const lb = await line.boundingBox();
  await page.mouse.click(lb.x + lb.width / 2, lb.y + lb.height / 2, { button: "right" });
  check((await page.locator("[data-testid=menu-remove-edge]").count()) === 0, "the line menu does not open on an implicit line");
  await page.keyboard.press("Escape");
  check((await page.locator("[data-testid=arrange]").count()) === 0, "Arrange is not offered while no card was moved");
  await shot("start-and-end");

  // Drag a card by its body.
  const before = await where("MOV-3");
  await drag("MOV-3", 203, 154);
  const moved = await where("MOV-3");
  check(moved.x % 8 === 0 && moved.y % 8 === 0, "a dropped card snaps to the 8 px grid", JSON.stringify(moved));
  check(Math.abs(moved.x - before.x - 203) <= 4 && Math.abs(moved.y - before.y - 154) <= 4, "the card follows the pointer", `${JSON.stringify(before)} -> ${JSON.stringify(moved)}`);
  check(!path().includes("/cards/"), "a drag does not open the card", path());
  await waitPos("MOV-3", moved, "the daemon keeps the card's place");
  await page.locator("[data-testid=arrange]").waitFor();
  console.log("PASS Arrange appears once a card was moved");
  // The line to Finish follows the card: its end is at the card's right edge.
  await shot("moved-card");

  await open();
  await card("MOV-3").waitFor();
  const again = await where("MOV-3");
  check(again.x === moved.x && again.y === moved.y, "the place survives a reload", JSON.stringify(again));
  const other = await browser.newPage({ viewport: { width: 1280, height: 900 } });
  await other.goto(url.replace("/?", "/projects/MOV/chains/flow?"));
  const [oc, oi] = [await other.locator('a.node[data-key="MOV-3"]').boundingBox(), await other.locator(".canvas-inner").boundingBox()];
  check(Math.round(oc.x - oi.x) === moved.x && Math.round(oc.y - oi.y) === moved.y, "a second window shows the same place");
  await other.close();

  // A press that barely moves is a click.
  const c = await centre(card("MOV-2"));
  await page.mouse.move(c.x, c.y);
  await page.mouse.down();
  await page.mouse.move(c.x + 3, c.y + 2);
  await page.mouse.up();
  await page.waitForURL(/\/cards\/MOV-2/, { timeout: 5000 }).catch(() => {
    throw new Error(`a press under 5 px should open the card, at ${path()}`);
  });
  console.log("PASS a press under 5 px is a click and opens the card");
  check(savedPos("MOV-2") === undefined, "and moves nothing");

  // Arrow keys move the open card by a grid step, Shift by five.
  const home2 = await where("MOV-2");
  await page.keyboard.press("ArrowRight");
  await waitPos("MOV-2", { x: home2.x + 8, y: home2.y }, "an arrow key moves the open card one step");
  await page.keyboard.press("Shift+ArrowDown");
  await waitPos("MOV-2", { x: home2.x + 8, y: home2.y + 40 }, "Shift and an arrow key moves it five steps");

  // Arrange puts every card back.
  await page.locator("[data-testid=arrange]").click();
  await waitPos("MOV-3", undefined, "Arrange clears the place the daemon kept");
  await waitPos("MOV-2", undefined, "for every card");
  const back = await where("MOV-3");
  check(back.x === before.x && back.y === before.y, "and the card is back where the layout puts it", JSON.stringify(back));
  check((await page.locator("[data-testid=arrange]").count()) === 0, "Arrange goes away again");
  await shot("arranged");

  if (errors.length) throw new Error(`the page threw: ${errors.join("; ")}`);
  console.log("web canvas move: PASS");
} catch (e) {
  console.error(`web canvas move: FAIL: ${e.message.slice(0, 800)}`);
  process.exitCode = 1;
} finally {
  await browser?.close();
  await cleanup(tok, home);
}
