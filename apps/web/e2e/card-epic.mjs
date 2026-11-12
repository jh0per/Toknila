// Browser test of moving a card to another epic of its project, against a real daemon.
// Usage: node e2e/card-epic.mjs <path to tok binary>   (cargo xtask e2e-web runs it)
//
// The card menu's "Move to epic…" lists the project's epics with the card's own greyed and moves
// the card to the end of the one picked; a card dragged from the diagram onto an epic's row of the
// sidebar goes there too (the row shows as a drop target while the card is over it); a card that
// has started is refused with the daemon's own words; `tok card mv` does the same from the shell.
// SCREENSHOT_DIR, when set, receives pictures.

import { execFileSync } from "node:child_process";
import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { checks, cleanup, launch } from "./helpers.mjs";

const tokBin = process.argv[2];
if (!tokBin) {
  console.error("usage: node e2e/card-epic.mjs <tok binary>");
  process.exit(2);
}
const home = mkdtempSync(join(tmpdir(), "toknila-epic-"));
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
/// The keys of an epic's cards in its order, as the daemon has them.
const keysOf = (chain) => {
  const c = JSON.parse(tok("chain", "show", chain, "--json"));
  const key = new Map(c.tickets.map((t) => [t.id, t.key]));
  return c.order.filter((n) => n.kind === "ticket").map((n) => key.get(n.id));
};

let browser;
try {
  tok("project", "new", "EPI", "--sim");
  tok("chain", "new", "soundpacks", "--project", "EPI", "--no-autostart");
  tok("chain", "new", "android", "--project", "EPI", "--no-autostart");
  for (const t of ["Pack", "Android (next)", "Busy"]) tok("ticket", "new", "soundpacks", t);
  tok("ticket", "new", "android", "Port");
  tok("start", "EPI-3");
  const { url } = JSON.parse(tok("open", "--json"));

  browser = await launch();
  const page = await browser.newPage({ viewport: { width: 1280, height: 900 } });
  const errors = [];
  page.on("pageerror", (e) => errors.push(e.message));
  const { expectText } = checks(page);
  const shot = async (name) => {
    if (process.env.SCREENSHOT_DIR) await page.screenshot({ path: join(process.env.SCREENSHOT_DIR, `${name}.png`) });
  };
  const card = (key) => page.locator(`a.node[data-key="${key}"]`);
  const row = (name) => page.locator(`[data-testid=chains] li[data-chain-item="${name}"]`);
  const check = (ok, what, detail = "") => {
    if (!ok) throw new Error(`${what} ${detail}`);
    console.log(`PASS ${what}`);
  };
  const sameKeys = (got, want) => JSON.stringify(got) === JSON.stringify(want);
  const centre = async (loc) => {
    const b = await loc.boundingBox();
    if (!b) throw new Error("not on the page");
    return { x: b.x + b.width / 2, y: b.y + b.height / 2 };
  };
  const waitKeys = async (chain, want, what) => {
    for (const until = Date.now() + 10_000; Date.now() < until; await page.waitForTimeout(100)) {
      if (sameKeys(keysOf(chain), want)) return console.log(`PASS ${what}`);
    }
    throw new Error(`${what}: ${chain} has ${JSON.stringify(keysOf(chain))}, wanted ${JSON.stringify(want)}`);
  };

  await page.goto(url.replace("/?", "/projects/EPI/chains/soundpacks?"));
  await card("EPI-2").waitFor();

  // The card menu: Move to epic… lists the project's epics, the card's own greyed.
  const at = await centre(card("EPI-2"));
  await page.mouse.click(at.x, at.y, { button: "right" });
  await page.locator("[data-testid=menu-card-move-epic]").click();
  await page.locator("[data-testid=menu-move-to-android]").waitFor();
  check(
    (await page.locator("[data-testid=menu-move-to-soundpacks]").getAttribute("aria-disabled")) === "true",
    "the card's own epic is greyed in the chooser",
  );
  check((await page.locator("[data-testid=menu-move-to-android]").getAttribute("aria-disabled")) === null, "the other epic can be picked");
  await shot("card-epic-chooser");
  await page.locator("[data-testid=menu-move-to-soundpacks]").click({ force: true });
  check((await page.locator("[data-testid=context-menu]").count()) === 1, "picking the greyed epic does nothing");
  await page.locator("[data-testid=menu-move-to-android]").click();
  await card("EPI-2").waitFor({ state: "detached" });
  await waitKeys("android", ["EPI-4", "EPI-2"], "the card goes to the end of the other epic through the menu");
  check(sameKeys(keysOf("soundpacks"), ["EPI-1", "EPI-3"]), "and leaves the old epic");

  // A drag onto an epic's row of the sidebar: the row shows as a target while the card is over it.
  const a = await centre(card("EPI-1"));
  const to = await centre(row("android").locator(".row"));
  await page.mouse.move(a.x, a.y);
  await page.mouse.down();
  await page.mouse.move((a.x + to.x) / 2, (a.y + to.y) / 2, { steps: 6 });
  await page.mouse.move(to.x, to.y, { steps: 6 });
  await row("android").and(page.locator("[data-drop]")).waitFor();
  check(
    (await row("soundpacks").getAttribute("data-drop")) === null,
    "only the row under the card is a drop target",
  );
  await shot("card-epic-drop-target");
  await page.mouse.up();
  await card("EPI-1").waitFor({ state: "detached" });
  await waitKeys("android", ["EPI-4", "EPI-2", "EPI-1"], "a card dropped on an epic's row goes to the end of that epic");
  check((await row("android").getAttribute("data-drop")) === null, "the row is no target once the card is dropped");

  // A card that has started is refused, in the daemon's words, and stays.
  const why = refusal("card", "mv", "EPI-3", "--chain", "android");
  const b = await centre(card("EPI-3"));
  const r = await centre(row("android").locator(".row"));
  await page.mouse.move(b.x, b.y);
  await page.mouse.down();
  await page.mouse.move((b.x + r.x) / 2, (b.y + r.y) / 2, { steps: 6 });
  await page.mouse.move(r.x, r.y, { steps: 6 });
  await page.mouse.up();
  await expectText(page.locator("[data-testid=edge-error]"), why, "a card that has started is refused with the daemon's message");
  check(sameKeys(keysOf("soundpacks"), ["EPI-3"]), "and stays in its epic");
  await shot("card-epic-refused");

  // The shell does the same, with a place. A card that waits for another of its epic names it and
  // stays until the dependency is removed.
  const linked = refusal("card", "mv", "EPI-1", "--chain", "soundpacks");
  check(linked.includes("EPI-2"), "a card with a dependency in its epic is refused, naming the other card", linked);
  tok("dep", "rm", "EPI-2", "EPI-1");
  tok("card", "mv", "EPI-1", "--chain", "soundpacks");
  await waitKeys("soundpacks", ["EPI-3", "EPI-1"], "tok card mv moves a card back, to the end");
  tok("dep", "rm", "EPI-4", "EPI-2");
  tok("card", "mv", "EPI-2", "--chain", "soundpacks", "--after", "EPI-3");
  await waitKeys("soundpacks", ["EPI-3", "EPI-2", "EPI-1"], "tok card mv --after puts it next to a card");

  if (errors.length) throw new Error(`the page threw: ${errors.join("; ")}`);
  console.log("web card epic: PASS");
} catch (e) {
  console.error(`web card epic: FAIL: ${e.message.slice(0, 800)}`);
  process.exitCode = 1;
} finally {
  await browser?.close();
  await cleanup(tok, home);
}
