// Browser test of the roadmap against a real daemon.
// Usage: node e2e/roadmap.mjs <path to tok binary>   (cargo xtask e2e-web runs it)
//
// The project's page is its roadmap: two finished epics folded into one group (it opens on a click
// and is remembered), one active epic highlighted, two planned ones, all in roadmap order. A click
// opens the chain and the crumb comes back. An epic moves by its menu and by drag; the daemon's
// order is what the page shows. New epic opens the dialog with the version after the greatest
// planned one.

import { execFileSync } from "node:child_process";
import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { checks, cleanup, launch } from "./helpers.mjs";

const tokBin = process.argv[2];
if (!tokBin) {
  console.error("usage: node e2e/roadmap.mjs <tok binary>");
  process.exit(2);
}
const home = mkdtempSync(join(tmpdir(), "toknila-roadmap-"));
const tok = (...args) =>
  execFileSync(tokBin, args, { env: { ...process.env, TOKNILA_HOME: home }, encoding: "utf8" });

const chainFile = (name, cards) =>
  `[chain]\nname = "${name}"\nautostart = true\n` +
  cards
    .map((c) => `\n[[ticket]]\nid = "${c.id}"\ntitle = "${c.id}"\nprofile = "sim"\n${c.done ? "done = true\n" : ""}`)
    .join("");

let browser;
try {
  tok("project", "new", "RMP");
  tok("review-mode", "human", "--project", "RMP");
  writeFileSync(join(home, "f1.toml"), chainFile("first", [{ id: "A1", done: true }]));
  writeFileSync(join(home, "f2.toml"), chainFile("second", [{ id: "B1", done: true }]));
  writeFileSync(join(home, "now.toml"), chainFile("now", [{ id: "N1" }, { id: "N2" }]));
  tok("chain", "import", join(home, "f1.toml"), "--project", "RMP");
  tok("chain", "import", join(home, "f2.toml"), "--project", "RMP");
  tok("chain", "import", join(home, "now.toml"), "--project", "RMP");
  tok("chain", "new", "later", "--project", "RMP", "--version", "0.6.0");
  tok("chain", "new", "last", "--project", "RMP", "--version", "0.7.0");
  const order = () => JSON.parse(tok("chain", "ls", "--json")).map((c) => c.chain.name);
  const want = (list) => {
    if (JSON.stringify(order()) !== JSON.stringify(list)) throw new Error(`the daemon's order is ${order()}, wanted ${list}`);
  };
  want(["first", "second", "now", "later", "last"]);
  const { url } = JSON.parse(tok("open", "--json"));

  browser = await launch();
  const page = await browser.newPage({ viewport: { width: 1280, height: 800 } });
  const errors = [];
  page.on("pageerror", (e) => errors.push(e.message));
  const { expectText } = checks(page);
  const t = (id) => page.locator(`[data-testid=${id}]`);
  const names = async () => page.locator("[data-testid=roadmap-node]").evaluateAll((ns) => ns.map((n) => n.dataset.chain));
  const shows = async (list, what) => {
    await page.waitForFunction(
      (l) => JSON.stringify([...document.querySelectorAll("[data-testid=roadmap-node]")].map((n) => n.dataset.chain)) === JSON.stringify(l),
      list, { timeout: 10_000 },
    ).catch(async () => {
      throw new Error(`${what}: the page shows ${JSON.stringify(await names())}, wanted ${JSON.stringify(list)}`);
    });
    console.log(`PASS ${what}`);
  };
  const node = (name) => page.locator(`[data-testid=roadmap-node][data-chain=${name}]`);

  await page.goto(url.replace("/?", "/projects/RMP?"));
  await t("roadmap").waitFor({ timeout: 10_000 });
  await expectText(t("roadmap-title"), "Roadmap", "the project's page is the roadmap");
  await shows(["now", "later", "last"], "the finished epics are folded; the others follow the roadmap order");
  await expectText(t("roadmap-finished"), "2 finished", "the folded group counts the finished epics");
  if ((await node("now").getAttribute("data-active")) === null) throw new Error("the active epic is not highlighted");
  if ((await node("later").getAttribute("data-active")) !== null) throw new Error("a planned epic is highlighted");
  await expectText(node("now").locator("[data-testid=roadmap-phase]"), "active", "the active epic reads active");
  await expectText(node("later").locator("[data-testid=roadmap-phase]"), "planned", "a planned epic reads planned");
  await expectText(node("later").locator("[data-testid=roadmap-version]"), "v0.6.0", "a node shows its version");
  await expectText(node("now").locator("[data-testid=roadmap-count]"), "0/2", "a node shows done/total");

  await t("roadmap-finished-toggle").click();
  await shows(["first", "second", "now", "later", "last"], "a click opens the finished group");
  await expectText(node("first").locator("[data-testid=roadmap-phase]"), "finished", "a finished epic reads finished");
  await page.reload();
  await shows(["first", "second", "now", "later", "last"], "the open group is remembered");
  await t("roadmap-finished-toggle").click();
  await shows(["now", "later", "last"], "a click folds it again");

  // The menu: Move later, and the first epic cannot move earlier.
  await node("now").locator("[data-testid=roadmap-menu-button]").click();
  if (!(await t("roadmap-move-earlier").isDisabled())) throw new Error("the first epic can move earlier");
  await node("later").locator("[data-testid=roadmap-menu-button]").click();
  await t("roadmap-move-later").click();
  await shows(["now", "last", "later"], "Move later from the menu reorders the epics");
  want(["first", "second", "now", "last", "later"]);
  console.log("PASS the daemon keeps the new order");

  // Drag: the last node onto the first one.
  await node("later").dragTo(node("now"));
  await shows(["later", "now", "last"], "dragging a node onto another reorders the epics");
  want(["first", "second", "later", "now", "last"]);
  console.log("PASS the daemon keeps the dragged order");

  // New epic: the dialog suggests the version after the greatest planned one.
  await t("roadmap-new-epic").click();
  await page.waitForFunction(() => document.querySelector("[data-testid=chain-version]")?.value === "0.7.1", null, { timeout: 10_000 }).catch(() => {
    throw new Error("the dialog did not suggest 0.7.1");
  });
  console.log("PASS New epic opens the dialog with the suggested version 0.7.1");
  await t("chain-name").fill("fresh");
  await t("create-chain").click();
  await t("chain-version-top").waitFor({ timeout: 10_000 });
  want(["first", "second", "later", "now", "last", "fresh"]);
  console.log("PASS a new epic goes to the end of the roadmap");

  // A click opens the chain, and the crumb leads back to the roadmap.
  await t("crumb-roadmap").click();
  await t("roadmap").waitFor({ timeout: 10_000 });
  await node("last").locator("[data-testid=roadmap-open]").click();
  await t("chain-name-top").waitFor({ timeout: 10_000 });
  await expectText(t("chain-name-top"), "last", "a click on a node opens its chain");
  await t("crumb-roadmap").click();
  await t("roadmap").waitFor({ timeout: 10_000 });
  console.log("PASS the crumb of a chain links back to the roadmap");

  // A narrow window: the sequence is vertical.
  await page.setViewportSize({ width: 560, height: 800 });
  const [a, b] = await Promise.all([node("later").boundingBox(), node("now").boundingBox()]);
  if (!a || !b || !(b.y > a.y + a.height - 1) || Math.abs(a.x - b.x) > 2) throw new Error(`the nodes do not stack: ${JSON.stringify([a, b])}`);
  console.log("PASS under 640px the roadmap is a vertical sequence");

  if (errors.length) throw new Error(`page errors: ${errors.join("; ")}`);
  console.log("web roadmap: PASS");
} catch (e) {
  console.error(`web roadmap: FAIL: ${e.message}`);
  process.exitCode = 1;
} finally {
  await browser?.close();
  await cleanup(tok, home);
}
