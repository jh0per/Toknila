// Browser test of the quick actions on the diagram against a real daemon.
// Usage: node e2e/canvas.mjs <path to tok binary>   (cargo xtask e2e-web runs it)
//
// A connector released on empty canvas opens a new-card field (the card follows the one the line
// came from); a double-click and N open it without a dependency; the right-click menus of empty
// canvas and of a card; Run chain / Stop chain in the header; N, S and Enter on the diagram.
// The simulated agent profile runs the cards. SCREENSHOT_DIR, when set, receives pictures.

import { execFileSync } from "node:child_process";
import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { checks, cleanup, launch } from "./helpers.mjs";

const tokBin = process.argv[2];
if (!tokBin) {
  console.error("usage: node e2e/canvas.mjs <tok binary>");
  process.exit(2);
}
const home = mkdtempSync(join(tmpdir(), "toknila-canvas-"));
const tok = (...args) =>
  execFileSync(tokBin, args, { env: { ...process.env, TOKNILA_HOME: home }, encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] });
const chainOf = (name) => JSON.parse(tok("chain", "show", name, "--json"));

let browser;
try {
  tok("project", "new", "CAN", "--sim");
  tok("chain", "new", "work", "--project", "CAN", "--no-autostart");
  for (const t of ["Alpha", "Bravo"]) tok("ticket", "new", "work", t);
  tok("project", "new", "RUN", "--sim");
  tok("chain", "new", "go", "--project", "RUN", "--no-autostart");
  tok("ticket", "new", "go", "Runner");
  tok("project", "new", "KEY", "--sim");
  tok("chain", "new", "keys", "--project", "KEY", "--no-autostart");
  for (const t of ["One", "Two"]) tok("ticket", "new", "keys", t);
  const { url } = JSON.parse(tok("open", "--json"));

  browser = await launch();
  const page = await browser.newPage({ viewport: { width: 1280, height: 1100 } });
  const errors = [];
  page.on("pageerror", (e) => errors.push(e.message));
  const { expectText, expectPath } = checks(page);
  const shot = async (name) => {
    if (process.env.SCREENSHOT_DIR) await page.screenshot({ path: join(process.env.SCREENSHOT_DIR, `${name}.png`) });
  };
  const open = (project, chain) => page.goto(url.replace("/?", `/projects/${project}/chains/${chain}?`));
  const handle = (key) => page.locator(`[data-testid=handle-${key}]`);
  const card = (key) => page.locator(`a.node[data-key="${key}"]`);
  const status = (key) => card(key).locator("[data-testid=status]");
  const quick = page.locator("[data-testid=quick-card]");
  const title = page.locator("[data-testid=quick-card-title]");
  const edge = (e) => page.locator(`[data-testid=edge-hit][data-edge="${e}"]`);
  const menuLabels = async () => (await page.locator("[data-testid=context-menu] .ctx-label").allTextContents()).map((t) => t.trim());
  const inner = async () => {
    const b = await page.locator(".canvas-inner").boundingBox();
    if (!b) throw new Error("no canvas");
    return b;
  };
  /// A point on empty canvas, to the right of the cards.
  const empty = async (dy = 0) => {
    const b = await inner();
    const at = { x: b.x + 760, y: b.y + 300 + dy };
    // The Finish node and the cards take their share of the diagram: go down to a free spot.
    const y = await page.evaluate(({ x, y }) => {
      for (let yy = y; yy < y + 480; yy += 8) {
        const el = document.elementFromPoint(x, yy);
        if (el?.closest(".canvas-wrap") && !el.closest(".node, .handle, .quick-card, .canvas-corner, .canvas-hint")) return yy;
      }
      return y;
    }, at);
    return { x: at.x, y };
  };
  const centre = async (loc) => {
    const b = await loc.boundingBox();
    if (!b) throw new Error("not on the page");
    return { x: b.x + b.width / 2, y: b.y + b.height / 2 };
  };
  const dragTo = async (from, p) => {
    const a = await centre(from);
    await page.mouse.move(a.x, a.y);
    await page.mouse.down();
    await page.mouse.move((a.x + p.x) / 2, (a.y + p.y) / 2, { steps: 5 });
    await page.mouse.move(p.x, p.y, { steps: 5 });
    await page.mouse.up();
  };
  const cardCount = () => page.locator("a.node").count();
  const waitCards = async (n, what) => {
    for (const until = Date.now() + 10_000; Date.now() < until; await page.waitForTimeout(100)) {
      if ((await cardCount()) === n) return console.log(`PASS ${what}: ${n} card(s)`);
    }
    throw new Error(`${what}: expected ${n} card(s), got ${await cardCount()}`);
  };

  await open("CAN", "work");
  await expectText(page.locator("[data-testid=chain-name-top]"), "work", "the chain is open");
  await waitCards(2, "two cards to begin with");

  // 1. A connector released on empty canvas: a field for a card after the one it came from.
  await dragTo(handle("CAN-1"), await empty());
  await quick.waitFor();
  await expectText(page.locator("[data-testid=quick-card-label]"), "New story that waits for CAN-1", "the drop opens the field");
  await shot("drop-menu");
  await page.keyboard.press("Escape");
  await quick.waitFor({ state: "detached" });
  await waitCards(2, "Escape cancels: no card was made");
  await dragTo(handle("CAN-1"), await empty());
  await title.fill("Charlie");
  await page.keyboard.press("Enter");
  await edge("CAN-1>CAN-3").waitFor({ state: "attached" });
  await expectText(card("CAN-3"), "Charlie", "Enter made the card");
  console.log("PASS the new card comes after CAN-1 (dependency CAN-1 > CAN-3)");
  if (await quick.count()) throw new Error("the field stayed open");

  // The daemon's refusal shows under the field as it said it.
  await dragTo(handle("CAN-1"), await empty(80));
  await page.keyboard.press("Enter");
  await expectText(page.locator("[data-testid=quick-card-error]"), "title is empty", "an empty title is refused with the daemon's words");
  await page.locator("[data-testid=quick-card-cancel]").click();
  await quick.waitFor({ state: "detached" });

  // 2. Double-click on empty canvas: a card at the end of the chain, no dependency named.
  const p = await empty(120);
  await page.mouse.dblclick(p.x, p.y);
  await expectText(page.locator("[data-testid=quick-card-label]"), "New story", "a double-click opens the field");
  await title.fill("Delta");
  await page.keyboard.press("Enter");
  await card("CAN-4").waitFor();
  await expectText(card("CAN-4"), "Delta", "Enter made the card at the end");
  if (!(await edge("CAN-3>CAN-4").count()) && !(await edge("CAN-2>CAN-4").count())) throw new Error("the new card is not at the end of the chain");
  console.log("PASS the card is appended to the end of the chain");

  // 3. Right-click on empty canvas.
  const q = await empty(-100);
  await page.mouse.click(q.x, q.y, { button: "right" });
  await page.locator("[data-testid=context-menu]").waitFor();
  const canvasMenu = await menuLabels();
  if (canvasMenu.join("|") !== "New story|New epic|Import a plan|Plan with the assistant") throw new Error(`canvas menu: ${canvasMenu}`);
  console.log(`PASS the canvas menu: ${canvasMenu.join(", ")}`);
  await shot("canvas-menu");
  await page.locator("[data-testid=menu-canvas-new-card]").click();
  await quick.waitFor();
  await page.keyboard.press("Escape");
  await page.mouse.click(q.x, q.y, { button: "right" });
  await page.locator("[data-testid=menu-canvas-new-chain]").click();
  await page.locator("[data-testid=new-chain-dialog]").waitFor();
  console.log("PASS New chain opens its dialog");
  await page.keyboard.press("Escape");
  await page.locator("[data-testid=new-chain-dialog]").waitFor({ state: "detached" });
  await page.mouse.click(q.x, q.y, { button: "right" });
  await page.locator("[data-testid=menu-canvas-import]").click();
  await page.locator("[data-testid=import-dialog]").waitFor();
  console.log("PASS Import a plan opens its dialog");
  await page.keyboard.press("Escape");
  await page.locator("[data-testid=import-dialog]").waitFor({ state: "detached" });
  await page.mouse.click(q.x, q.y, { button: "right" });
  await page.locator("[data-testid=menu-canvas-planner]").click();
  await page.locator("[data-testid=assistant-panel]").waitFor({ timeout: 10_000 });
  await expectText(page.locator("[data-testid=context-chip]"), "the story CAN-4", "Plan with the assistant opens the panel where the person is, on the open card");
  await page.locator("[data-testid=assistant-close]").click();
  await page.locator("[data-testid=assistant-panel]").waitFor({ state: "detached" });

  // 4. Right-click on a card: the actions that apply to its state.
  await open("CAN", "work");
  await waitCards(4, "back on the chain");
  await card("CAN-2").click({ button: "right" });
  await page.locator("[data-testid=menu-card-start]").waitFor();
  const readyMenu = await menuLabels();
  if (readyMenu.join("|") !== "Open|Edit|Open terminal|Start|Pause|Ask for a design|Add story before|Add story after|Add dependency…|Move to epic…|Cancel") throw new Error(`ready card menu: ${readyMenu}`);
  console.log(`PASS a ready card's menu: ${readyMenu.join(", ")}`);
  await shot("card-menu");
  await page.locator("[data-testid=menu-card-add-before]").click();
  await expectText(page.locator("[data-testid=quick-card-label]"), "New story that CAN-2 waits for", "Add story before opens the field");
  await title.fill("Prelude");
  await page.keyboard.press("Enter");
  await edge("CAN-5>CAN-2").waitFor({ state: "attached" });
  console.log("PASS the new card became a dependency of CAN-2 (CAN-5 > CAN-2)");
  await card("CAN-2").click({ button: "right" });
  await page.locator("[data-testid=menu-card-add-before]").waitFor();
  const waitingMenu = await menuLabels();
  console.log(`PASS a card that waits: ${waitingMenu.join(", ")}`);
  await page.locator("[data-testid=menu-card-add-after]").click();
  await expectText(page.locator("[data-testid=quick-card-label]"), "New story that waits for CAN-2", "Add story after opens the field");
  await title.fill("Epilogue");
  await page.keyboard.press("Enter");
  await edge("CAN-2>CAN-6").waitFor({ state: "attached" });
  console.log("PASS Add card after made CAN-6 wait for CAN-2");

  // Start from the menu, then the menu of a running card has no Start and no Add card before.
  await card("CAN-1").click({ button: "right" });
  await page.locator("[data-testid=menu-card-start]").click();
  await expectText(status("CAN-1"), "simulated", "Start in the menu started the card");
  await card("CAN-1").click({ button: "right" });
  await page.locator("[data-testid=menu-card-cancel]").waitFor();
  const runningMenu = await menuLabels();
  if (runningMenu.includes("Start") || runningMenu.includes("Add story before")) throw new Error(`running card menu: ${runningMenu}`);
  console.log(`PASS a running card's menu: ${runningMenu.join(", ")}`);
  // Cancel asks first: Keep it changes nothing, the confirmation cancels.
  await page.locator("[data-testid=menu-card-cancel]").click();
  await page.locator("[data-testid=cancel-confirm]").waitFor();
  if ((await page.locator("[data-testid=cancel-reason]").inputValue()) !== "Cancelled in the window") throw new Error("the reason is not prefilled");
  console.log("PASS the confirmation has the reason prefilled");
  await page.locator("[data-testid=cancel-keep]").click();
  await page.waitForTimeout(300);
  await expectText(status("CAN-1"), "simulated", "Keep it left the card running");
  await card("CAN-1").click({ button: "right" });
  await page.locator("[data-testid=menu-card-cancel]").click();
  await shot("cancel-asks");
  await page.locator("[data-testid=cancel-send]").click();
  await expectText(status("CAN-1"), "cancelled", "the confirmed Cancel cancelled the card");
  // Open
  await card("CAN-3").click({ button: "right" });
  await page.locator("[data-testid=menu-card-open]").click();
  await expectPath("/projects/CAN/chains/work/cards/CAN-3", "Open opens the card");

  // The card bar's Cancel opens the same confirmation, with the reason editable.
  await page.locator("[data-testid=actions] button.danger").click();
  await page.locator("[data-testid=cancel-confirm]").waitFor();
  const reason = page.locator("[data-testid=cancel-reason]");
  if ((await reason.inputValue()) !== "Cancelled in the window") throw new Error("the bar's reason is not prefilled");
  await reason.fill("changed my mind");
  await page.locator("[data-testid=cancel-send]").click();
  await expectText(status("CAN-3"), "cancelled", "Cancel in the card bar cancelled the card");
  if (await page.locator("[data-testid=cancel-confirm]").count()) throw new Error("the confirmation stayed open");
  const said = JSON.stringify(JSON.parse(tok("ticket", "--json", "show", "CAN-3")));
  if (!said.includes("changed my mind")) throw new Error(`the edited reason did not reach the daemon: ${said.slice(0, 300)}`);
  console.log("PASS the edited reason reached the daemon");

  // 5. Run chain / Stop chain.
  await open("RUN", "go");
  const run = page.locator("[data-testid=run-chain]");
  await expectText(run, "Run epic", "an epic without autostart offers Run epic");
  const hint = (await run.getAttribute("title")) ?? "";
  if (!/ready stories up to the free agent slots/.test(hint) || !/already running keep running/.test(hint)) throw new Error(`hint: ${hint}`);
  console.log(`PASS the hint says both: ${hint}`);
  await shot("run-chain");
  await run.click();
  await expectText(status("RUN-1"), "simulated", "Run chain started the ready card");
  await expectText(run, "Stop epic", "the button reads Stop epic while autostart is on");
  if (chainOf("go").chain.autostart !== true) throw new Error("the daemon does not have autostart on");
  await shot("stop-chain");
  await run.click();
  await expectText(run, "Run epic", "Stop epic turns it off");
  if (chainOf("go").chain.autostart !== false) throw new Error("the daemon still has autostart on");
  await expectText(status("RUN-1"), "simulated", "the running card keeps running");

  // 6. Keys: N, S, Enter.
  await open("KEY", "keys");
  await waitCards(2, "the keys chain");
  await page.locator("[data-testid=canvas]").click({ position: { x: 700, y: 400 } });
  await page.keyboard.press("n");
  await quick.waitFor();
  await expectText(page.locator("[data-testid=quick-card-label]"), "New story", "N opens the field");
  await title.type("nsn");
  if ((await title.inputValue()) !== "nsn") throw new Error("the keys were taken while typing in the field");
  console.log("PASS no key is taken while a text field has focus");
  await page.keyboard.press("Enter");
  await card("KEY-3").waitFor();
  await expectText(card("KEY-3"), "nsn", "Enter made the card");
  await open("KEY", "keys");
  await waitCards(3, "the chain again");
  await card("KEY-1").focus();
  await page.locator("[data-testid=canvas]").waitFor();
  await page.waitForTimeout(400);
  await page.keyboard.press("s");
  await expectText(status("KEY-1"), "simulated", "S started the focused card");
  await card("KEY-2").focus();
  await page.keyboard.press("Enter");
  await expectPath("/projects/KEY/chains/keys/cards/KEY-2", "Enter opened the focused card");
  await page.waitForTimeout(400);
  await page.keyboard.press("s");
  await expectText(status("KEY-2"), "simulated", "S started the open card");

  if (errors.length) throw new Error(`the page threw: ${errors.join("; ")}`);
  console.log("web canvas: PASS");
} catch (e) {
  console.error(`web canvas: FAIL: ${e.message.slice(0, 800)}`);
  process.exitCode = 1;
} finally {
  await browser?.close();
  await cleanup(tok, home);
}
