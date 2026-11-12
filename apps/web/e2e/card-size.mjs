// Browser test of the size of cards against a real daemon.
// Usage: node e2e/card-size.mjs <path to tok binary>   (cargo xtask e2e-web runs it)
//
// A card is as wide as its title needs (196 to 320 px): a long title shows two lines and no
// ellipsis, the status pill shows the whole fine status ("review") and does not touch the key.
// The grip at a card's bottom-right corner resizes it; the size is saved, survives a reload and
// is cleared by Arrange. SCREENSHOT_DIR, when set, receives pictures.

import { execFileSync } from "node:child_process";
import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { checks, cleanup, launch } from "./helpers.mjs";

const tokBin = process.argv[2];
if (!tokBin) {
  console.error("usage: node e2e/card-size.mjs <tok binary>");
  process.exit(2);
}
const home = mkdtempSync(join(tmpdir(), "toknila-size-"));
const tok = (...args) =>
  execFileSync(tokBin, args, { env: { ...process.env, TOKNILA_HOME: home }, encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] });
const saved = (key) => JSON.parse(tok("chain", "show", "sizes", "--json")).tickets.find((t) => t.key === key);
const LONG = "Shell edits are refused, not asked about, so the agent never waits";

let browser;
try {
  tok("project", "new", "CSZ", "--sim");
  tok("chain", "new", "sizes", "--project", "CSZ", "--goal", "Fit the text", "--no-autostart");
  tok("ticket", "new", "sizes", LONG, "--profile", "sim");
  tok("ticket", "new", "sizes", "Short", "--prompt", "# Say what the card is for\n\nA second paragraph.");
  tok("ticket", "new", "sizes", "A second long title that wraps onto two lines in a card", "--after", "CSZ-1");
  tok("start", "CSZ-1");
  tok("sim", "CSZ-1", "finish", "done");
  const { url } = JSON.parse(tok("open", "--json"));

  browser = await launch();
  const page = await browser.newPage({ viewport: { width: 1600, height: 900 } });
  const errors = [];
  page.on("pageerror", (e) => errors.push(e.message));
  const { expectText } = checks(page);
  const shot = async (name) => {
    if (process.env.SCREENSHOT_DIR) await page.screenshot({ path: join(process.env.SCREENSHOT_DIR, `${name}.png`) });
  };
  const open = () => page.goto(url.replace("/?", "/projects/CSZ/chains/sizes?"));
  const card = (key) => page.locator(`a.node[data-key="${key}"]`);
  const check = (ok, what, detail = "") => {
    if (!ok) throw new Error(`${what} ${detail}`);
    console.log(`PASS ${what}`);
  };
  const box = async (key) => {
    const [c, i] = [await card(key).boundingBox(), await page.locator(".canvas-inner").boundingBox()];
    if (!c || !i) throw new Error(`${key} is not on the page`);
    return { x: Math.round(c.x - i.x), y: Math.round(c.y - i.y), w: Math.round(c.width), h: Math.round(c.height) };
  };
  const waitSaved = async (key, want, what) => {
    for (const until = Date.now() + 10_000; Date.now() < until; await page.waitForTimeout(100)) {
      const s = saved(key)?.size;
      if (want === undefined ? s === undefined : s && s.w === want.w && s.h === want.h) return console.log(`PASS ${what}`);
    }
    throw new Error(`${what}: the daemon has ${JSON.stringify(saved(key)?.size)}`);
  };

  await open();
  await card("CSZ-1").waitFor();

  // The text fits.
  const title = card("CSZ-1").locator("[data-testid=title]");
  const lines = await title.locator(".tl").allTextContents();
  check(lines.length === 2, "a long title wraps to two lines", JSON.stringify(lines));
  check(!lines.join("").includes("…"), "and is not cut with an ellipsis", JSON.stringify(lines));
  check(lines.join("") === LONG, "the two lines read back as the title", JSON.stringify(lines));
  const clipped = await title.locator(".tl").evaluateAll((els) => els.some((e) => e.scrollWidth > e.clientWidth));
  check(!clipped, "no title line is wider than the card");
  const pill = card("CSZ-1").locator("[data-testid=status]");
  check((await pill.textContent())?.trim() === "review", "the pill shows the whole fine status", await pill.textContent());
  check(
    await pill.evaluate((e) => e.scrollWidth <= e.clientWidth),
    "the pill is not clipped",
  );
  const [pb, kb] = [await pill.boundingBox(), await card("CSZ-1").locator(".key").boundingBox()];
  check(pb.x + pb.width <= kb.x, "the pill ends before the key", `${pb.x + pb.width} > ${kb.x}`);
  const long = await box("CSZ-1");
  const short = await box("CSZ-2");
  check(long.w > 196 && long.w <= 320, "a long title makes the card wider, up to 320 px", JSON.stringify(long));
  check(short.w === 196, "a short title keeps the minimum width", JSON.stringify(short));
  check(long.h > 56 && short.h < long.h, "the height follows the content", `${long.h} / ${short.h}`);
  const third = await box("CSZ-3");
  check(third.x >= long.x + long.w, "the next column starts after the widest card", `${third.x} < ${long.x + long.w}`);
  await shot("fitted");

  // Resize with the grip.
  check((await page.locator("[data-testid=arrange]").count()) === 0, "Arrange is not offered while no card was moved or resized");
  await card("CSZ-2").hover();
  const grip = card("CSZ-2").locator("[data-testid=grip]");
  const g = await grip.boundingBox();
  const from = { x: g.x + g.width / 2, y: g.y + g.height / 2 };
  await page.mouse.move(from.x, from.y);
  await page.mouse.down();
  await page.mouse.move(from.x + 60, from.y + 30, { steps: 4 });
  await page.mouse.move(from.x + 120, from.y + 60, { steps: 4 });
  await page.mouse.up();
  const grown = await box("CSZ-2");
  check(grown.w >= short.w + 112 && grown.w <= short.w + 128, "dragging the grip widens the card", JSON.stringify(grown));
  check(grown.h >= short.h + 52 && grown.h <= short.h + 68, "and makes it taller", JSON.stringify(grown));
  check(!new URL(page.url()).pathname.includes("/cards/"), "a resize does not open the card");
  await waitSaved("CSZ-2", { w: grown.w, h: grown.h }, "the daemon keeps the card's size");
  check(saved("CSZ-2").pos === undefined, "a resize does not move the card");
  const moved = await box("CSZ-3");
  check(moved.x >= grown.x + grown.w, "the cards to the right make room", `${moved.x} < ${grown.x + grown.w}`);
  await page.locator("[data-testid=arrange]").waitFor();
  console.log("PASS Arrange appears once a card was resized");
  await shot("resized");

  // The limits: 480 wide and 320 tall.
  // On the lowest card: the canvas keeps room for the corner buttons, so its grip is free.
  const pull = async (key, dx, dy) => {
    await card(key).hover();
    const b = await card(key).locator("[data-testid=grip]").boundingBox();
    const at = { x: b.x + b.width / 2, y: b.y + b.height / 2 };
    await page.mouse.move(at.x, at.y);
    await page.mouse.down();
    await page.mouse.move(at.x + dx, at.y + dy, { steps: 6 });
    await page.mouse.up();
  };
  await pull("CSZ-2", 400, 400);
  const huge = await box("CSZ-2");
  check(huge.w === 480 && huge.h === 320, "a card is never resized past 480 x 320", JSON.stringify(huge));
  const desc = card("CSZ-2").locator("[data-testid=desc]");
  check((await desc.textContent()) === "Say what the card is for", "a tall card shows the first line of its description", await desc.textContent());
  check(!(await card("CSZ-1").locator("[data-testid=desc]").count()), "a card without a description shows none");
  // Scrolled to the end, the lowest grip is clear of every corner button.
  await page.locator("[data-testid=canvas]").evaluate((e) => { e.scrollTop = e.scrollHeight; });
  const gb = await card("CSZ-2").locator("[data-testid=grip]").boundingBox();
  const buttons = page.locator(".canvas-corner .btn");
  for (let i = 0; i < (await buttons.count()); i++) {
    const b = await buttons.nth(i).boundingBox();
    const hits = gb.x < b.x + b.width && gb.x + gb.width > b.x && gb.y < b.y + b.height && gb.y + gb.height > b.y;
    check(!hits, "the lowest grip is not under a corner button", `${JSON.stringify(gb)} vs ${JSON.stringify(b)}`);
  }
  await waitSaved("CSZ-2", { w: 480, h: 320 }, "the daemon keeps the card at the limit");
  await shot("huge");
  // And never narrower than its content: back to the smallest.
  await pull("CSZ-2", -huge.w + 10, -huge.h + 10);
  await waitSaved("CSZ-2", { w: 196, h: short.h }, "the daemon keeps the smallest size");
  const small = await box("CSZ-2");
  check(small.w === 196 && small.h === short.h, "nor narrower than 196 wide or shorter than its content", JSON.stringify(small));
  check(!(await desc.count()), "and a card no taller than its title shows no description");
  // Back to a size to keep for the reload.
  const g4 = await grip.boundingBox();
  const f4 = { x: g4.x + g4.width / 2, y: g4.y + g4.height / 2 };
  await page.mouse.move(f4.x, f4.y);
  await page.mouse.down();
  await page.mouse.move(f4.x + 100, f4.y + 50, { steps: 6 });
  await page.mouse.up();
  const kept = await box("CSZ-2");
  await waitSaved("CSZ-2", { w: kept.w, h: kept.h }, "the daemon has the last size");

  await open();
  await card("CSZ-2").waitFor();
  const again = await box("CSZ-2");
  check(again.w === kept.w && again.h === kept.h, "the size survives a reload", JSON.stringify(again));

  // The connector still drags a line from the edge of a resized card.
  const handle = page.locator("[data-testid=handle-CSZ-2]");
  const hb = await handle.boundingBox();
  const cb = await card("CSZ-2").boundingBox();
  check(Math.abs(hb.x + hb.width / 2 - (cb.x + cb.width)) <= 2, "the connector sits on the right edge of the resized card");

  // Double-click on the grip lets the card fit its text again.
  await card("CSZ-2").hover();
  await grip.dblclick();
  await waitSaved("CSZ-2", undefined, "double-clicking the grip clears the size");
  check((await box("CSZ-2")).w === 196, "and the card fits its text again");

  // Arrange clears every size.
  await card("CSZ-3").hover();
  const g5 = await card("CSZ-3").locator("[data-testid=grip]").boundingBox();
  await page.mouse.move(g5.x + g5.width / 2, g5.y + g5.height / 2);
  await page.mouse.down();
  await page.mouse.move(g5.x + g5.width / 2 + 80, g5.y + g5.height / 2 + 40, { steps: 6 });
  await page.mouse.up();
  const before = await box("CSZ-3");
  await page.locator("[data-testid=arrange]").waitFor();
  await page.locator("[data-testid=arrange]").click();
  await waitSaved("CSZ-3", undefined, "Arrange clears the sizes in the daemon");
  await page.waitForFunction(() => !document.querySelector("[data-testid=arrange]"), null, { timeout: 10_000 });
  const after = await box("CSZ-3");
  check(after.w < before.w, "and the card is as wide as its content again", `${JSON.stringify(before)} -> ${JSON.stringify(after)}`);
  await expectText(card("CSZ-1").locator("[data-testid=status]"), "review", "the pill still says review");
  await shot("arranged");

  if (errors.length) throw new Error(`page errors: ${errors.join("; ")}`);
  console.log("web card size: PASS");
} catch (e) {
  console.error(`web card size: FAIL: ${String(e.message).slice(0, 800)}`);
  process.exitCode = 1;
} finally {
  await browser?.close();
  await cleanup(tok, home);
}
