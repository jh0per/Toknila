// Browser test of the sidebar's tree against a real daemon: two projects, each a node that opens and
// closes its own rows, the state remembered over a reload, and the keys of a tree.
// Usage: node e2e/sidebar-tree.mjs <path to tok binary>   (cargo xtask e2e-web runs it)

import { execFileSync } from "node:child_process";
import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { checks, cleanup, launch } from "./helpers.mjs";

const tokBin = process.argv[2];
if (!tokBin) {
  console.error("usage: node e2e/sidebar-tree.mjs <tok binary>");
  process.exit(2);
}
const home = mkdtempSync(join(tmpdir(), "toknila-tree-"));
const tok = (...args) =>
  execFileSync(tokBin, args, { env: { ...process.env, TOKNILA_HOME: home }, encoding: "utf8" });

let browser;
try {
  for (const [key, chain] of [["TRA", "alpha"], ["TRB", "beta"]]) {
    tok("project", "new", key);
    writeFileSync(join(home, `${chain}.toml`), `[chain]\nname = "${chain}"\n\n[[ticket]]\nid = "${key}1"\ntitle = "${key}1"\nprofile = "sim"\n`);
    tok("chain", "import", join(home, `${chain}.toml`), "--project", key);
  }
  const longName = "a-finished-chain-with-a-really-long-name-for-the-sidebar";
  writeFileSync(join(home, "long.toml"), `[chain]\nname = "${longName}"\n\n[[ticket]]\nid = "TRL1"\ntitle = "TRL1"\nprofile = "sim"\ndone = true\n`);
  tok("chain", "import", join(home, "long.toml"), "--project", "TRA");
  const { url } = JSON.parse(tok("open", "--json"));

  browser = await launch();
  const page = await browser.newPage({ viewport: { width: 1280, height: 800 } });
  const errors = [];
  page.on("pageerror", (e) => errors.push(e.message));
  const { expectPath, expectText } = checks(page);
  const node = (key) => page.locator(`[data-testid=projects] li[data-project=${key}]`);
  const rowOf = (key) => page.locator(`[data-testid=projects] li[data-project=${key}] > .row`);
  const chev = (key) => rowOf(key).locator("[data-toggle]");
  const chain = (name) => page.locator(`[data-testid=chains] li[data-chain-item="${name}"]`);
  const must = (ok, what) => {
    if (!ok) throw new Error(what);
    console.log(`PASS ${what}`);
  };
  const expanded = async (key) => node(key).getAttribute("aria-expanded");
  const waitExpanded = async (key, want, what) => {
    await page.waitForFunction(
      ([sel, w]) => document.querySelector(sel)?.getAttribute("aria-expanded") === w,
      [`[data-testid=projects] li[data-project=${key}]`, String(want)],
      { timeout: 10_000 },
    ).catch(() => {
      throw new Error(`${what}: expected aria-expanded=${want}`);
    });
    console.log(`PASS ${what}`);
  };
  const focused = () => page.evaluate(() => {
    const a = document.activeElement;
    return `${a?.textContent?.trim()}|${a?.closest("li[data-project]")?.getAttribute("data-project") ?? ""}`;
  });

  await page.goto(url.replace("/?", "/projects/TRA?"));
  await waitExpanded("TRA", true, "the project on the screen is open");
  await waitExpanded("TRB", false, "the other project is closed");
  must((await page.locator("[data-testid=projects] ul[role=tree]").count()) >= 1, "the projects are a tree");
  must((await page.locator("[data-testid=projects] [role=treeitem]").count()) > 4, "its rows are treeitems");
  const rows = (await node("TRA").locator("> ul > li").allTextContents()).map((t) => t.replace(/^[^A-Za-z]+/, "").match(/^[A-Za-z]+/)?.[0]);
  must(rows.join(",").startsWith("Roadmap,Assistant,Inbox,Agents,Epics"), `the rows under an open project are Roadmap, Assistant, Inbox, Agents, Epics... (${rows})`);
  must(rows.at(-2) === "Designs" && rows.at(-1) === "Settings", "Designs and Settings close the list");
  await chain("alpha").waitFor();
  must((await chain("beta").count()) === 0, "a closed project shows none of its chains");

  // A finished chain's row: its name (cut by an ellipsis, never missing), one line, no date, no repeated word.
  await page.locator("[data-testid=finished-toggle]").click();
  const fin = chain(longName);
  await fin.waitFor();
  const finText = (await fin.innerText()).replace(/\s+/g, " ");
  // The row reads the name made readable, not the slug.
  const longTitle = "A finished chain with a really long name for the sidebar";
  must(finText.includes(longTitle), `a finished row shows its chain's title (${finText})`);
  must(!/\d+ [A-Z][a-z]{2}/.test(finText) && !/finished/.test(finText.replace(longTitle, "")), `a finished row has no date and no phase word (${finText})`);
  const geo = await fin.locator("a.item").evaluate((a) => {
    const t = a.querySelector(".chain-name");
    const side = document.querySelector(".sidebar").getBoundingClientRect().right;
    const lh = parseFloat(getComputedStyle(a).lineHeight) || 20;
    return { h: a.getBoundingClientRect().height, right: a.getBoundingClientRect().right, side, lh, clipped: t.scrollWidth > t.clientWidth, face: getComputedStyle(t).fontFamily };
  });
  must(geo.h < geo.lh * 2, `a finished row is one line (${geo.h}px)`);
  must(geo.right <= geo.side && geo.clipped, "a long name ends in an ellipsis inside the sidebar");
  must(!/mono/i.test(geo.face), `a chain name is in the body face (${geo.face})`);

  // Each project opens its own rows; a chain of the second one is reachable without its page.
  await chev("TRB").click();
  await waitExpanded("TRB", true, "the chevron opens the closed project");
  await waitExpanded("TRA", true, "the first project stays open");
  await chain("beta").locator("a").click();
  await expectPath("/projects/TRB/chains/beta", "a chain of the second project opens from the tree");
  await waitExpanded("TRB", true, "the project of the open chain is open");

  // Closing and opening is remembered per project, over a reload; the project on the screen cannot close.
  await waitExpanded("TRA", false, "a project that was only on the screen closes when it leaves");
  await chev("TRA").click();
  await waitExpanded("TRA", true, "the chevron opens it and the choice is stored");
  await chev("TRA").click();
  await waitExpanded("TRA", false, "the chevron closes a project");
  await chev("TRB").click();
  await waitExpanded("TRB", true, "the project on the screen stays open");
  await page.reload();
  await waitExpanded("TRB", true, "the on-screen project is open after a reload");
  await waitExpanded("TRA", false, "the closed project is closed after a reload");
  await chev("TRA").click();
  await waitExpanded("TRA", true, "it opens again");
  await page.reload();
  await waitExpanded("TRA", true, "the opened project is open after a reload");

  // The project's own name opens its roadmap; so does the first row under it.
  await rowOf("TRA").locator("a").click();
  await expectPath("/projects/TRA", "the project's name opens its page");
  await page.locator("[data-testid=roadmap-link]").first().waitFor();
  await node("TRB").locator("[data-testid=planner-link]").click();
  await expectPath("/projects/TRB", "the Assistant row opens the roadmap");
  await page.locator("[data-testid=assistant-panel]").waitFor({ timeout: 10_000 });
  await expectText(page.locator("[data-testid=context-chip]"), "Roadmap", "with the assistant beside it and the roadmap chip on");
  await node("TRB").locator("a", { hasText: "Designs" }).click();
  await expectPath("/projects/TRB/settings/designs", "the Designs row opens the project's designs");
  await page.goto(url.replace("/?", "/projects/TRA?"));
  await waitExpanded("TRB", true, "TRB was opened and stays remembered");

  // The keys: Right opens, Down and Up move, Left closes or goes up.
  await chev("TRB").click();
  await waitExpanded("TRB", false, "TRB closed for the keys");
  await rowOf("TRB").locator("a").focus();
  await page.keyboard.press("ArrowRight");
  await waitExpanded("TRB", true, "Right opens the project");
  await page.keyboard.press("ArrowDown");
  must((await focused()) === "Roadmap|TRB", `Down moves to the first row (${await focused()})`);
  await page.keyboard.press("ArrowDown");
  must((await focused()).startsWith("Assistant"), `Down moves on (${await focused()})`);
  await page.keyboard.press("ArrowUp");
  must((await focused()).startsWith("Roadmap"), `Up moves back (${await focused()})`);
  await page.keyboard.press("ArrowLeft");
  must((await focused()).includes("TRB") && (await focused()).startsWith("TRB"), `Left on a leaf goes to its project (${await focused()})`);
  await page.keyboard.press("ArrowLeft");
  await waitExpanded("TRB", false, "Left closes the project");
  must((await expanded("TRA")) === "true", "the other project is untouched");
  const tabStops = await page.locator("[data-testid=projects] [data-row][tabindex='0']").count();
  must(tabStops === 1, `the tree has one tab stop (${tabStops})`);

  // The project switcher in the top bar: a menu of projects, filtered by typing, the screen kind kept.
  const t = (id) => page.locator(`[data-testid=${id}]`);
  await page.goto(url.replace("/?", "/projects/TRA?"));
  await t("project-switcher").waitFor();
  await expectText(t("project-switcher"), "TRA", "the crumb's project segment is the switcher button");
  await t("project-switcher").click();
  await t("switcher-menu").waitFor();
  must((await t("switcher-item-TRA").getAttribute("data-current")) === "true", "the project on the screen is marked in the menu");
  must((await t("switcher-item-TRB").getAttribute("data-current")) === null, "the other project is not marked");
  must((await t("switcher-all").count()) === 1 && (await t("switcher-add").count()) === 1, "the menu has All projects and Add project");
  await page.keyboard.press("Escape");
  await t("switcher-menu").waitFor({ state: "detached" });
  must(true, "Escape closes the menu");
  await t("project-switcher").click();
  await t("switcher-filter").fill("trb");
  must((await t("switcher-item-TRA").count()) === 0 && (await t("switcher-item-TRB").count()) === 1, "typing filters the projects");
  await page.keyboard.press("Enter");
  await expectPath("/projects/TRB", "choosing a project from the roadmap opens its roadmap");
  await t("switcher-menu").waitFor({ state: "detached" });

  // The key opens it from a screen without the crumb, and the same screen opens in the other project.
  await page.goto(url.replace("/?", "/projects/TRA/settings/designs?"));
  await page.keyboard.press("Control+k");
  await t("switcher-menu").waitFor();
  await t("switcher-filter").fill("trb");
  await t("switcher-item-TRB").click();
  await expectPath("/projects/TRB/settings/designs", "Ctrl+K opens the switcher and the settings screen is kept across projects");
  await page.keyboard.press("Control+k");
  await t("switcher-all").click();
  await expectPath("/settings", "All projects opens the machine's Projects list");
  await page.keyboard.press("Control+k");
  await t("switcher-add").click();
  await page.locator("[role=dialog]").first().waitFor({ timeout: 10_000 });
  must(true, "Add project opens the add-project dialog");
  await page.keyboard.press("Escape");

  if (errors.length) throw new Error(`the page threw: ${errors.join("; ")}`);
  console.log("web sidebar-tree: PASS");
} catch (e) {
  console.error(`web sidebar-tree: FAIL: ${e.message.slice(0, 800)}`);
  process.exitCode = 1;
} finally {
  await browser?.close();
  await cleanup(tok, home);
}
