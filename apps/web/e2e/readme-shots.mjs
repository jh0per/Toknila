// Browser spec that makes the pictures of the README: a simulated bakery project (invented names
// only, fake agents, never a real one) with an epic whose board has a story running and one in
// review, a Review card with its hand-in and Needs you, the assistant's chat beside the roadmap,
// and the roadmap. Each view must draw its landmarks; with SCREENSHOT_DIR set, `shoot()` writes
// `readme-<view>-light.png` and `readme-<view>-dark.png` there (the README carries the light ones,
// under docs/screenshots/readme/).
// Usage: node e2e/readme-shots.mjs <tok binary> <fake-agent binary>   (cargo xtask e2e-web runs it)
// needs: fake-agent

import { execFileSync } from "node:child_process";
import { mkdirSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { cleanup, launch, session, shipOf, shoot } from "./helpers.mjs";

const [tokArg, agentArg] = process.argv.slice(2);
if (!tokArg || !agentArg) {
  console.error("usage: node e2e/readme-shots.mjs <tok binary> <fake-agent binary>");
  process.exit(2);
}
const tokBin = resolve(tokArg);
const fakeAgent = resolve(agentArg);
const { home, marker: fakeMarker } = session();
const run = (cmd, args) => execFileSync(cmd, args, { encoding: "utf8" });
const tok = (...args) => execFileSync(tokBin, args, { env: { ...process.env, TOKNILA_HOME: home }, encoding: "utf8" });
const git = (dir, ...args) =>
  run("git", ["-C", dir, "-c", "user.name=t", "-c", "user.email=t@example.com", "-c", "commit.gpgsign=false", ...args]);
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const show = (key) => JSON.parse(tok("show", key, "--json"));

async function until(what, f) {
  for (const stop = Date.now() + 20_000; Date.now() < stop; await sleep(100)) {
    if (f()) return;
  }
  throw new Error(`timed out waiting for ${what}`);
}

const chainFile = (name, title, version, cards) =>
  `[chain]\nname = "${name}"\ntitle = "${title}"\nversion = "${version}"\nautostart = false\n` +
  cards.map((c) => `\n[[ticket]]\nid = "${c}"\ntitle = "${c}"\nprofile = "sim"\ndone = true\n`).join("");

let browser;
try {
  const repo = join(home, "bakery");
  mkdirSync(repo, { recursive: true });
  git(repo, "init", "--quiet");
  git(repo, "symbolic-ref", "HEAD", "refs/heads/main");
  writeFileSync(join(repo, "README.md"), "Bakery shop\n");
  git(repo, "add", "README.md");
  git(repo, "commit", "--quiet", "-m", "init");
  mkdirSync(join(home, "profiles"), { recursive: true });
  const steps = (...s) => s.map((x) => `'${x}'`).join(", ");
  writeFileSync(join(home, "profiles", "plain.toml"), `command = ['${fakeAgent}', 'sleep', '${fakeMarker}',${steps("print", "working", "hold")}]\n`);
  const say = "I would split it in three: the menu with photos, then ordering online, then loyalty cards. Online ordering has four stories. Shall I add the epics?";
  writeFileSync(
    join(home, "profiles", "fake-planner.toml"),
    `role = 'planner'\ncommand = ['${fakeAgent}', 'sleep', '${fakeMarker}',${steps(
      "print", "planner ready", "echo",
      "post", "/planner/say", JSON.stringify({ text: say, about: "roadmap" }),
      "hold",
    )}]\nstartup_secs = 0\n`,
  );
  tok("project", "new", "BAKE", "--repo", repo, "--base", "main", "--profile", "plain");
  tok("project", "set", "BAKE", "--parallel", "2");
  tok("review-mode", "human", "--project", "BAKE");

  // The roadmap: one finished epic, the active one, two planned.
  writeFileSync(join(home, "menu.toml"), chainFile("menu", "Menu and photos", "0.1.0", ["Menu page", "Photo gallery"]));
  tok("chain", "import", join(home, "menu.toml"), "--project", "BAKE");
  tok("chain", "new", "ordering", "--project", "BAKE", "--title", "Online ordering", "--goal", "A customer can order bread for the next morning.", "--version", "0.2.0", "--no-autostart");
  tok("chain", "new", "loyalty", "--project", "BAKE", "--title", "Loyalty cards", "--goal", "Regulars collect stamps.", "--version", "0.3.0", "--no-autostart");
  tok("chain", "new", "delivery", "--project", "BAKE", "--title", "Delivery", "--goal", "Orders reach the door.", "--version", "0.4.0", "--no-autostart");
  tok("ticket", "new", "ordering", "Order form", "--accept", "A customer picks a loaf and a day", "--accept", "The form works on a phone");
  tok("ticket", "new", "ordering", "Cart page", "--accept", "The cart totals the prices");
  tok("ticket", "new", "ordering", "Pickup times", "--after", "BAKE-3");
  tok("ticket", "new", "ordering", "Order emails", "--after", "BAKE-4", "--after", "BAKE-5");
  tok("start", "BAKE-3");
  tok("start", "BAKE-4");
  await until("BAKE-3 running", () => show("BAKE-3").ticket.status === "running");
  await until("BAKE-4 running", () => show("BAKE-4").ticket.status === "running");
  const ship = shipOf(tok, "BAKE-4");
  writeFileSync(join(ship, "cart.txt"), "cart\n");
  git(ship, "add", "cart.txt");
  git(ship, "commit", "--quiet", "-m", "cart page");
  tok(
    "sim", "BAKE-4", "finish",
    "**What changed:** the cart page lists the loaves and totals the prices\n**Checks:**\n- PASS npm test: 18 tests passed, 0 failed\n- NOT_RUN browser check: no browser on this machine\n- PERSON open the cart on a phone and check that the total is readable\n**Left:** coupons are not part of this story",
  );
  await until("BAKE-4 in review", () => show("BAKE-4").ticket.status === "review");
  tok("planner", "start", "BAKE", "--profile", "fake-planner");
  const { url } = JSON.parse(tok("open", "--json"));
  const at = (path) => url.replace("/?", `${path}?`);

  browser = await launch();
  const ctx = await browser.newContext({ viewport: { width: 1360, height: 840 } });
  const page = await ctx.newPage();
  const errors = [];
  page.on("pageerror", (e) => errors.push(e.message));
  const t = (id) => page.locator(`[data-testid=${id}]`);

  // 1. The epic's board: BAKE-3 runs, BAKE-4 waits for review.
  await page.setViewportSize({ width: 1640, height: 540 });
  await page.goto(at("/projects/BAKE/chains/ordering"));
  await page.locator("a.node[data-key=BAKE-4]").waitFor({ timeout: 10_000 });
  await sleep(600);
  await shoot(page, "readme-board");
  console.log("PASS the epic's board draws a story running and one in review");

  // 2. The Review card with the hand-in and Needs you.
  await page.setViewportSize({ width: 1360, height: 1240 });
  await page.goto(at("/projects/BAKE/chains/ordering/cards/BAKE-4"));
  await t("review-block").waitFor({ timeout: 10_000 });
  await t("needs-you").waitFor({ timeout: 10_000 });
  await sleep(400);
  await shoot(page, "readme-review");
  console.log("PASS the Review card shows the hand-in and Needs you");

  // 3. The assistant beside the roadmap, with its answer in the chat.
  await page.setViewportSize({ width: 1360, height: 720 });
  await page.goto(at("/projects/BAKE"));
  await t("roadmap").waitFor({ timeout: 10_000 });
  await page.keyboard.press("Control+.");
  await t("planner-ask-input").waitFor({ timeout: 10_000 });
  await t("planner-ask-input").fill("Plan a bakery shop with a menu, online ordering and loyalty cards.");
  await t("planner-ask-send").click();
  await page.locator("[data-author=assistant]").filter({ hasText: "Online ordering has four stories" }).first().waitFor({ timeout: 20_000 });
  await sleep(600);
  await shoot(page, "readme-assistant");
  console.log("PASS the assistant's chat shows the question and the answer beside the roadmap");

  // 4. The roadmap alone.
  await page.keyboard.press("Control+.");
  await t("roadmap-title").waitFor({ timeout: 10_000 });
  await sleep(600);
  await shoot(page, "readme-roadmap");
  console.log("PASS the roadmap draws its epics");

  if (errors.length) throw new Error(`the page threw: ${errors.join("; ")}`);
  console.log("readme shots: PASS");
} catch (e) {
  console.error(`readme shots: FAIL: ${e.message.slice(0, 600)}`);
  process.exitCode = 1;
} finally {
  await browser?.close();
  await cleanup(tok, home);
}
