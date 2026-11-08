// Browser test of the plan screen against a real daemon (D1).
// Usage: node e2e/plan-screen.mjs <path to tok binary>   (cargo xtask e2e-web runs it)
//
// Cards are coloured by their fine status (stripe and word), carry a subtasks bar, hide the runner
// chip when it is the project's default, and a proposed card has its own Approve and Reject. The
// chain pill, the legend and the hint; the Theme switch (remembered per viewer); the slot stepper
// in the sidebar writes the project's limit; the right-click menus (groups, shortcuts, arrow keys,
// type-ahead, the line's title, Add dependency); the open card's lines are strong and the others
// fade; connectors show on hover; Fit zooms a wide chain into view; the update note sits above the
// panel divider; the planner's empty state offers Start. The simulated agent profile runs cards.
// SCREENSHOT_DIR, when set, receives pictures.

import { execFileSync } from "node:child_process";
import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { checks, cleanup, launch } from "./helpers.mjs";

const tokBin = process.argv[2];
if (!tokBin) {
  console.error("usage: node e2e/plan-screen.mjs <tok binary>");
  process.exit(2);
}
const home = mkdtempSync(join(tmpdir(), "toknila-plan-screen-"));
const tok = (...args) =>
  execFileSync(tokBin, args, { env: { ...process.env, TOKNILA_HOME: home }, encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] });
const statusOf = (key) => {
  const j = JSON.parse(tok("show", key, "--json"));
  return (j.ticket ?? j).status;
};
const card = (id, title, extra = "", profile = "sim") => `\n[[ticket]]\nid = "${id}"\ntitle = "${title}"\nprofile = "${profile}"\n${extra}`;

let browser;
try {
  tok("project", "new", "PLN");
  const work = join(home, "work.toml");
  writeFileSync(
    work,
    `[chain]\nname = "work"\nautostart = false\n` +
      card("a", "Alpha", "done = true\n") +
      card("b", "Bravo", 'after = ["a"]\nsubtasks = ["one", "two", "three", "four"]\ndone_subtasks = [1, 2]\n') +
      card("c", "Charlie", 'after = ["a"]\n') +
      card("d", "Delta", 'after = ["a"]\n') +
      card("e", "Echo", 'after = ["b", "c"]\n') +
      card("f", "Foxtrot", 'after = ["a"]\n', "claude"),
  );
  tok("chain", "import", work, "--project", "PLN");
  tok("start", "PLN-2");
  tok("sim", "PLN-3", "start");
  tok("sim", "PLN-3", "ask", "which one?");
  tok("propose", "work", "Refresh tokens", "--prompt", "Keep sessions alive.", "--after", "PLN-1", "--before", "PLN-5", "--from", "PLN-2");
  tok("propose", "work", "Reject me", "--prompt", "Not wanted.", "--from", "PLN-2");
  const wide = join(home, "wide.toml");
  writeFileSync(
    wide,
    `[chain]\nname = "wide"\nautostart = false\n` +
      ["w1", "w2", "w3", "w4", "w5", "w6", "w7"].map((id, i, all) => card(id, id.toUpperCase(), i ? `after = ["${all[i - 1]}"]\n` : "")).join(""),
  );
  tok("chain", "import", wide, "--project", "PLN");
  const { url } = JSON.parse(tok("open", "--json"));
  const open = (path) => page.goto(url.replace("/?", `${path}?`));

  browser = await launch();
  const page = await browser.newPage({ viewport: { width: 1440, height: 900 }, colorScheme: "light" });
  const errors = [];
  page.on("pageerror", (e) => errors.push(e.message));
  const { expectText } = checks(page);
  const t = (id) => page.locator(`[data-testid=${id}]`);
  const node = (key) => page.locator(`a.node[data-key="${key}"]`);
  const st = (key) => node(key).locator("[data-testid=status]");
  const css = (loc, prop) => loc.evaluate((el, p) => getComputedStyle(el)[p], prop);
  const colourOf = (name) =>
    page.evaluate((n) => {
      const s = document.createElement("span");
      s.style.color = `var(${n})`;
      document.body.appendChild(s);
      const c = getComputedStyle(s).color;
      s.remove();
      return c;
    }, name);
  const shot = async (name) => {
    if (process.env.SCREENSHOT_DIR) await page.screenshot({ path: join(process.env.SCREENSHOT_DIR, `${name}.png`) });
  };
  const must = (ok, what) => {
    if (!ok) throw new Error(what);
    console.log(`PASS ${what}`);
  };
  /// The open menu as text: entries by label, `|` where a separator is drawn.
  const menuShape = async () =>
    (await page.locator("[data-testid=context-menu] .ctx-item").evaluateAll((items) =>
      items.map((i) => `${i.querySelector(".ctx-sep") ? "|" : ""}${i.querySelector(".ctx-label")?.textContent}`))).join(",");
  const focusedTestId = () => page.evaluate(() => document.activeElement?.getAttribute("data-testid"));

  await open("/projects/PLN/chains/work");
  await expectText(t("chain-name-top"), "work", "the chain is open");
  await node("PLN-5").waitFor();

  // 1. Colour per fine status: the class, the stripe and the word.
  must((await node("PLN-1").getAttribute("class")).includes("s-merged"), "a done card has its own class (s-merged)");
  must((await node("PLN-2").getAttribute("class")).includes("s-running"), "a running card has its own class (s-running)");
  must((await node("PLN-3").getAttribute("class")).includes("s-waiting"), "a waiting card has its own class (s-waiting)");
  const prog = await colourOf("--prog");
  const attn = await colourOf("--block");
  const done = await colourOf("--done");
  const todo = await colourOf("--todo");
  must((await css(st("PLN-2"), "color")) === prog, "the running word is teal");
  must((await css(st("PLN-3"), "color")) === attn, "a waiting card (a person is needed) reads amber");
  must((await css(st("PLN-1"), "color")) === done, "the done word is green");
  must((await css(st("PLN-4"), "color")) === todo, "a ready card's word is grey");
  must((await css(node("PLN-2"), "borderTopColor")) === prog, "the running card's stripe is teal");
  must((await css(node("PLN-3"), "borderTopColor")) === attn, "a waiting card's stripe is amber");
  must((await css(node("PLN-1"), "borderTopColor")) === done, "the done card's stripe is green");
  await shot("plan-screen-light");

  // 2. Subtasks bar, runner chip, usage chip.
  await expectText(node("PLN-2").locator("[data-testid=subtasks]"), "2/4", "a card with subtasks shows how far they are");
  const width = await node("PLN-2").locator("[data-testid=subtasks] i").evaluate((el) => el.style.width);
  must(width === "50%", `the bar is half full (${width})`);
  must((await node("PLN-4").locator("[data-testid=subtasks]").count()) === 0, "a card without subtasks has no bar");
  must((await node("PLN-4").locator(".runner").count()) === 0, "the runner chip stays away when it is the project's default");
  await expectText(node("PLN-6").locator(".runner"), "claude", "a card on another runner shows it");

  // 3. A proposed card: Approve and Reject on the card itself.
  const proposed = page.locator("a.node.s-proposed");
  await proposed.first().locator("[data-testid=card-approve]").waitFor();
  must((await proposed.count()) === 2, "both proposals are on the diagram");
  must((await proposed.first().locator("[data-testid=card-reject]").count()) === 1, "a proposal has Reject next to Approve");
  await expectText(proposed.first().locator("[data-testid=status]"), "awaiting approval", "a proposal reads awaiting approval");
  await proposed.first().locator("[data-testid=card-approve]").click();
  for (const until = Date.now() + 10_000; statusOf("PLN-7") === "proposed"; await page.waitForTimeout(100)) {
    if (Date.now() > until) throw new Error("Approve on the card did not approve it");
  }
  must(statusOf("PLN-7") !== "proposed", "Approve on the card approved it in the daemon");
  await expectText(st("PLN-7"), "todo", "the approved card reads todo");
  must((await node("PLN-7").locator("[data-testid=card-approve]").count()) === 0, "the approved card has no proposal buttons");
  const second = page.locator("a.node.s-proposed");
  await second.locator("[data-testid=card-reject]").click();
  must((await t("reason-send").isDisabled()) === true, "Reject asks for the person's words first (Send waits for them)");
  await shot("plan-screen-reject");
  await t("reason-text").fill("not for this release");
  await t("reason-send").click();
  for (const until = Date.now() + 10_000; statusOf("PLN-8") !== "cancelled"; await page.waitForTimeout(100)) {
    if (Date.now() > until) throw new Error(`Reject on the card did not reject it (${statusOf("PLN-8")})`);
  }
  console.log("PASS Reject on the card, with a reason, rejected it in the daemon");

  // 4. The top bar, the legend and the hint.
  await expectText(t("chain-status"), "needs human", "the chain pill says what the chain needs");
  must((await t("chain-status").getAttribute("class")).includes("tone-attn"), "the pill of a chain that needs a person has the amber tone");
  must((await css(t("chain-status"), "color")) === attn, "the chain pill is amber");
  must((await t("run-chain").getAttribute("class")).includes("primary"), "Run chain is the primary button");
  await expectText(t("canvas-hint"), "Order is priority. Left to right, no dates.", "the hint is on the canvas");
  for (const word of ["todo", "in progress", "needs you", "done"]) await expectText(t("canvas-legend"), word, "the legend has a word");

  // 5. Theme: System, Light, Dark; remembered for this viewer.
  const bg = () => css(page.locator("body"), "backgroundColor");
  const light = await bg();
  await t("theme-button").click();
  await page.locator("[data-testid=context-menu] [data-testid=theme-dark]").click();
  must((await page.locator("html").getAttribute("data-theme")) === "dark", "Dark puts data-theme=dark on <html>");
  const dark = await bg();
  must(dark !== light && dark === "rgb(18, 23, 28)", `the page is dark (${dark})`);
  must((await page.evaluate(() => localStorage.getItem("toknila.theme"))) === "dark", "the choice is kept in this browser");
  await shot("plan-screen-dark");
  await page.reload();
  await t("theme-button").waitFor();
  must((await page.locator("html").getAttribute("data-theme")) === "dark", "Dark is still on after a reload");
  await t("theme-button").click();
  must((await page.locator("[data-testid=theme-dark]").getAttribute("aria-checked")) === "true", "the menu marks the current theme");
  await page.locator("[data-testid=theme-light]").click();
  must((await page.locator("html").getAttribute("data-theme")) === "light" && (await bg()) === light, "Light forces the light colours");
  await t("theme-button").click();
  await page.locator("[data-testid=theme-system]").click();
  must((await page.locator("html").getAttribute("data-theme")) === null, "System leaves the choice to the computer");
  await page.emulateMedia({ colorScheme: "dark" });
  must((await bg()) === dark, "System follows the computer's dark scheme");
  await page.emulateMedia({ colorScheme: "light" });
  must((await bg()) === light, "System follows the computer's light scheme");

  // 6. The slot stepper writes the project's limit.
  const limit = () => JSON.parse(tok("project", "ls", "--json")).find((p) => p.key === "PLN").max_parallel;
  await expectText(t("slot-limit"), "2", "the stepper shows the project's limit");
  await t("slots-more").click();
  await expectText(t("slot-limit"), "3", "+ raises it");
  must(limit() === 3, "the daemon's limit is 3");
  await expectText(t("slots"), "/3 agents", "the top bar follows");
  await t("slots-fewer").click();
  await t("slots-fewer").click();
  await expectText(t("slot-limit"), "1", "− lowers it");
  must(limit() === 1, "the daemon's limit is 1");
  must(await t("slots-fewer").isDisabled(), "the stepper stops at 1");
  await t("slots-more").click();
  await expectText(t("slot-limit"), "2", "back to 2");
  await shot("plan-screen-stepper");

  // 7. The open card's lines are strong, the others fade; connectors show on hover or selection.
  const focusOf = (edge) => page.locator(`g.edge[data-edge="${edge}"]`).getAttribute("data-focus");
  must((await focusOf("PLN-2>PLN-5")) === "" && (await focusOf("PLN-1>PLN-3")) === "", "with no card open no line is faded");
  await node("PLN-2").click();
  await page.locator("g.edge[data-focus=hl]").first().waitFor();
  must((await focusOf("PLN-1>PLN-2")) === "hl" && (await focusOf("PLN-2>PLN-5")) === "hl", "the open card's lines are highlighted");
  must((await focusOf("PLN-1>PLN-3")) === "dim" && (await focusOf("PLN-3>PLN-5")) === "dim", "the other lines fade");
  const strong = await page.locator("g.edge[data-edge='PLN-1>PLN-2'] .line").evaluate((el) => getComputedStyle(el).strokeWidth);
  // The fade is a short transition: wait for it to finish.
  await page.waitForFunction(() => Number.parseFloat(getComputedStyle(document.querySelector("g.edge[data-edge='PLN-1>PLN-3'] .line")).opacity) < 0.5);
  const faded = await page.locator("g.edge[data-edge='PLN-1>PLN-3'] .line").evaluate((el) => getComputedStyle(el).opacity);
  must(Number.parseFloat(strong) > 2 && Number.parseFloat(faded) < 0.5, `strong lines are thicker (${strong}) and faded ones lighter (${faded})`);
  await shot("plan-screen-focus");
  // The connector fades in and out over a tenth of a second: wait for the end value.
  const connector = async (key, value, what) => {
    await page.waitForFunction(
      ([k, v]) => getComputedStyle(document.querySelector(`[data-testid=handle-${k}]`)).opacity === v,
      [key, value],
      { timeout: 5000 },
    ).catch(async () => {
      throw new Error(`${what}: opacity is ${await css(t(`handle-${key}`), "opacity")}, expected ${value}`);
    });
    console.log(`PASS ${what}`);
  };
  await connector("PLN-2", "1", "the open card's connector shows");
  await connector("PLN-4", "0", "another card's connector is hidden");
  await node("PLN-4").hover();
  await connector("PLN-4", "1", "hovering a card shows its connector");
  await page.mouse.move(1000, 700);
  await connector("PLN-4", "0", "leaving it hides the connector again");

  // 8. Right-click menus: groups, shortcuts, keys.
  await node("PLN-4").click({ button: "right" });
  await t("menu-card-start").waitFor();
  const ready = await menuShape();
  must(
    ready === "Open,Edit,Open terminal,|Start,Pause,|Add card before,Add card after,Add dependency…,|Cancel",
    `a ready card's menu is grouped (${ready})`,
  );
  must((await t("menu-card-open").locator("kbd").textContent()) === "Enter", "Open shows its key");
  must((await t("menu-card-start").locator("kbd").textContent()) === "S", "Start shows its key");
  must((await focusedTestId()) === "menu-card-open", "the first entry has the focus");
  await page.keyboard.press("ArrowDown");
  must((await focusedTestId()) !== "menu-card-open", "ArrowDown moves on");
  await page.keyboard.press("ArrowUp");
  await page.keyboard.press("ArrowUp");
  must((await focusedTestId()) === "menu-card-cancel", "ArrowUp from the first entry wraps to the last");
  await page.keyboard.press("Home");
  must((await focusedTestId()) === "menu-card-open", "Home goes to the first entry");
  await page.keyboard.press("End");
  must((await focusedTestId()) === "menu-card-cancel", "End goes to the last entry");
  await page.keyboard.press("Home");
  await page.keyboard.press("p");
  must((await focusedTestId()) === "menu-card-pause", "typing a letter jumps to the entry that starts with it");
  must((await page.locator("[data-testid=context-menu] .danger").count()) === 1, "one entry is red");
  must((await page.locator("[data-testid=context-menu] .ctx-item:last-child .danger").count()) === 1, "and it is last");
  await shot("plan-screen-menu");
  await page.keyboard.press("Escape");
  await t("context-menu").waitFor({ state: "detached" });
  must((await page.evaluate(() => document.activeElement?.getAttribute("data-key"))) === "PLN-4", "Escape closes it and returns the focus to the card");

  // The daemon's actions of other states: Pause, Cancel and Add dependency are not offered to a running card.
  await node("PLN-2").click({ button: "right" });
  await t("menu-card-cancel").waitFor();
  const busy = await menuShape();
  must(busy.endsWith("|Cancel") && !busy.includes("Start") && !busy.includes("Add card before"), `a running card's menu follows the daemon (${busy})`);
  await page.keyboard.press("Escape");

  // The menu of a line: titled with its two cards; Delete is its key.
  const edgesBefore = await page.locator("[data-testid=edge-hit]").count();
  // Six lines leave PLN-1 and their wide click areas overlap, so the right-click is sent to this line itself.
  await page.locator("[data-testid=edge-hit][data-edge='PLN-1>PLN-4']").evaluate((el) => {
    const p = el.getPointAtLength(el.getTotalLength() / 2);
    const m = el.getScreenCTM();
    el.dispatchEvent(new MouseEvent("contextmenu", {
      bubbles: true, cancelable: true, button: 2, clientX: m.a * p.x + m.c * p.y + m.e, clientY: m.b * p.x + m.d * p.y + m.f,
    }));
  });
  await expectText(t("context-menu-title"), "PLN-1 → PLN-4", "the line's menu is titled with its two cards");
  must((await t("menu-remove-edge").locator("kbd").textContent()) === "Delete", "Remove dependency shows Delete");
  await page.keyboard.press("Delete");
  for (const until = Date.now() + 10_000; (await page.locator("[data-testid=edge-hit]").count()) !== edgesBefore - 1; await page.waitForTimeout(100)) {
    if (Date.now() > until) throw new Error("Delete in the line's menu did not remove it");
  }
  console.log("PASS Delete in the line's menu removed the dependency");

  // The empty canvas menu: New card with N.
  await page.mouse.click(1330, 300, { button: "right" });
  await t("menu-canvas-new-card").waitFor();
  const canvasMenu = await menuShape();
  must(canvasMenu === "New card,|New chain,Import a plan,|Ask the planner", `the canvas menu has three groups (${canvasMenu})`);
  must((await t("menu-canvas-new-card").locator("kbd").textContent()) === "N", "New card shows N");
  await page.keyboard.press("Escape");

  // Add dependency: pick the card to wait for.
  await node("PLN-6").click({ button: "right" });
  await t("menu-card-add-dependency").click();
  await expectText(t("linking-hint"), "Click the card PLN-6 should wait for", "Add dependency says what to click");
  await node("PLN-4").click();
  await page.locator("g.edge[data-edge='PLN-4>PLN-6']").waitFor({ state: "attached" });
  console.log("PASS clicking a card made PLN-6 wait for PLN-4");
  must((await t("linking-hint").count()) === 0, "the hint is gone afterwards");

  // 9. Fit: a chain wider than the window.
  await page.setViewportSize({ width: 1100, height: 700 });
  await open("/projects/PLN/chains/wide");
  await expectText(t("chain-name-top"), "wide", "the wide chain is open");
  await t("cut-hint").waitFor();
  console.log("PASS cards out of view are announced");
  const canvas = await t("canvas").boundingBox();
  const outside = async () => {
    let n = 0;
    for (let i = 1; i <= 7; i++) {
      const b = await node(`PLN-${8 + i}`).boundingBox();
      if (!b || b.x + b.width > canvas.x + canvas.width + 1 || b.y + b.height > canvas.y + canvas.height + 1) n++;
    }
    return n;
  };
  must((await outside()) > 0, "at full size some cards are out of the view");
  await shot("plan-screen-cut");
  await t("fit").click();
  await page.waitForFunction(() => Number(document.querySelector(".canvas-inner").dataset.zoom) < 1);
  must((await outside()) === 0, "Fit shows every card");
  await shot("plan-screen-fit");
  await t("fit").click();
  await page.waitForFunction(() => Number(document.querySelector(".canvas-inner").dataset.zoom) === 1);
  console.log("PASS the second click goes back to full size");
  await page.setViewportSize({ width: 1440, height: 900 });

  // 10. The update note sits above the panel divider, not over the panel's columns.
  await page.addInitScript(() => {
    window.__TAURI__ = {
      core: {
        invoke: async (c) =>
          c === "update_status"
            ? { current_version: "0.2.4", last_check: null, busy: false, banner: { title: "Toknila 0.2.5 is ready", notes: "A better diagram." }, error: null }
            : {},
      },
    };
  });
  await open("/projects/PLN/chains/work/cards/PLN-2");
  await t("update-banner").waitFor();
  await node("PLN-2").waitFor();
  await t("panel-divider").waitFor();
  const banner = await t("update-banner").boundingBox();
  const divider = await t("panel-divider").boundingBox();
  must(banner.y + banner.height <= divider.y, `the update note (bottom ${Math.round(banner.y + banner.height)}) is above the divider (${Math.round(divider.y)})`);
  await shot("plan-screen-update");

  // 11. The planner's empty state offers Start in the middle, with no empty terminal.
  await open("/projects/PLN/planner");
  await t("planner-start-centre").waitFor();
  must(!(await t("term-host").isVisible()), "the planner's empty terminal is not drawn");
  await expectText(t("planner-chain"), "Plan so far", "the diagram is labelled");
  await shot("plan-screen-planner");

  if (errors.length) throw new Error(`the page threw: ${errors.join("; ")}`);
  console.log("web plan screen: PASS");
} catch (e) {
  console.error(`web plan screen: FAIL: ${e.message.slice(0, 600)}`);
  process.exitCode = 1;
} finally {
  await browser?.close();
  await cleanup(tok, home);
}
