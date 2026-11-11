// Browser test of a chain's Designs view against a real daemon.
// Usage: node e2e/design-board.mjs <path to tok binary>   (cargo xtask e2e-web runs it)
//
// A chain with two designed cards and a third without designs: the Designs view shows both
// mockups with every card's region outlined in its status colour; a region's label opens its
// card; zoom, pan and Fit work; dragging a region's handle onto another card moves the reference
// (and a drop on nothing moves nothing); a chain without designs says so. SCREENSHOT_DIR, when
// set, receives a picture.

import { execFileSync } from "node:child_process";
import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { deflateSync } from "node:zlib";
import { checks, cleanup, launch } from "./helpers.mjs";

const tokBin = process.argv[2];
if (!tokBin) {
  console.error("usage: node e2e/design-board.mjs <tok binary>");
  process.exit(2);
}
const home = mkdtempSync(join(tmpdir(), "toknila-board-"));
const tok = (...args) =>
  execFileSync(tokBin, args, { env: { ...process.env, TOKNILA_HOME: home }, encoding: "utf8" });

const CRC = Array.from({ length: 256 }, (_, n) => {
  let c = n;
  for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
  return c >>> 0;
});
const crc32 = (buf) => {
  let c = 0xffffffff;
  for (const b of buf) c = CRC[(c ^ b) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
};
/// A solid `w` x `h` png (RGB), made here so the two designs differ in size and bytes.
function png(w, h, [r, g, b]) {
  const chunk = (type, data) => {
    const head = Buffer.alloc(4);
    head.writeUInt32BE(data.length);
    const body = Buffer.concat([Buffer.from(type), data]);
    const tail = Buffer.alloc(4);
    tail.writeUInt32BE(crc32(body));
    return Buffer.concat([head, body, tail]);
  };
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(w, 0);
  ihdr.writeUInt32BE(h, 4);
  ihdr.set([8, 2, 0, 0, 0], 8);
  const row = Buffer.concat([Buffer.from([0]), Buffer.from(Array.from({ length: w }, () => [r, g, b]).flat())]);
  const raw = Buffer.concat(Array.from({ length: h }, () => row));
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk("IHDR", ihdr), chunk("IDAT", deflateSync(raw)), chunk("IEND", Buffer.alloc(0)),
  ]);
}

let browser;
try {
  tok("project", "new", "BRD");
  writeFileSync(
    join(home, "c.toml"),
    `[chain]\nname = "work"\nautostart = false\n\n` +
      ["Login screen", "Home screen", "Settings screen"].map((t, i) => `[[ticket]]\nid = "T${i}"\ntitle = "${t}"\nprofile = "sim"\n`).join("\n"),
  );
  tok("chain", "import", join(home, "c.toml"), "--project", "BRD");
  writeFileSync(join(home, "e.toml"), `[chain]\nname = "bare"\nautostart = false\n\n[[ticket]]\nid = "E"\ntitle = "No mockup"\nprofile = "sim"\n`);
  tok("chain", "import", join(home, "e.toml"), "--project", "BRD");
  const { url } = JSON.parse(tok("open", "--json"));
  const origin = new URL(url).origin;
  const bearer = new URL(url).searchParams.get("token");
  const call = async (method, path, body, type = "application/json") => {
    const r = await fetch(`${origin}/v1${path}`, {
      method, headers: { Authorization: `Bearer ${bearer}`, "Content-Type": type }, body: type === "application/json" ? JSON.stringify(body) : body,
    });
    if (!r.ok) throw new Error(`${method} ${path} answered ${r.status}`);
    return r.json();
  };
  const upload = (name, bytes) => call("POST", `/projects/BRD/designs?name=${name}`, bytes, "application/octet-stream");
  const login = await upload("login.png", png(60, 40, [200, 80, 80]));
  const home_ = await upload("home.png", png(40, 60, [80, 80, 200]));
  const regionA = { x: 10, y: 10, w: 40, h: 30 };
  await call("PATCH", "/tickets/BRD-1", { designs: [{ design: login.id, region: regionA }] });
  await call("PATCH", "/tickets/BRD-2", { designs: [{ design: home_.id }] });
  const refsOf = async (key) => (await call("GET", `/tickets/${key}`)).ticket.designs ?? [];

  browser = await launch();
  const page = await browser.newPage({ viewport: { width: 1400, height: 1100 } });
  const errors = [];
  page.on("pageerror", (e) => errors.push(e.message));
  const { expectPath } = checks(page);
  const t = (id) => page.locator(`[data-testid=${id}]`);
  const at = (path) => url.replace("/?", `${path}?`);
  const zoomValue = async () => Number.parseInt(await t("zoom-value").textContent(), 10);
  const loaded = () =>
    page.waitForFunction(() => {
      const imgs = [...document.querySelectorAll("[data-testid=board-image]")];
      return imgs.length === 2 && imgs.every((i) => i.complete && i.naturalWidth > 0);
    }, undefined, { timeout: 10_000 });

  // The switch on the diagram leads to the board; the address keeps the view.
  await page.goto(at("/projects/BRD/chains/work"));
  await t("end-node").waitFor({ timeout: 10_000 });
  await t("view-designs").click();
  await expectPath("/projects/BRD/chains/work/designs", "the Designs view has an address of its own");
  await t("design-board").waitFor();
  await loaded();
  if ((await t("board-tile").count()) !== 2) throw new Error("the board does not show both designs");
  console.log("PASS the board shows both mockups");

  // Every card's region is outlined in percent of its picture, in the card's status colour, which
  // is the colour of its chip in the bar.
  const regions = await page.locator("[data-testid=board-region]").evaluateAll((els) =>
    els.map((el) => ({ card: el.dataset.card, left: el.style.left, top: el.style.top, width: el.style.width, height: el.style.height, color: getComputedStyle(el).borderTopColor })));
  const byCard = Object.fromEntries(regions.map((r) => [r.card, r]));
  if (JSON.stringify([byCard["BRD-1"]?.left, byCard["BRD-1"]?.top, byCard["BRD-1"]?.width, byCard["BRD-1"]?.height]) !== JSON.stringify(["10%", "10%", "40%", "30%"])) {
    throw new Error(`BRD-1's outline is ${JSON.stringify(byCard["BRD-1"])}`);
  }
  if (JSON.stringify([byCard["BRD-2"]?.left, byCard["BRD-2"]?.width, byCard["BRD-2"]?.height]) !== JSON.stringify(["0%", "100%", "100%"])) {
    throw new Error(`BRD-2 without a region is not outlined whole: ${JSON.stringify(byCard["BRD-2"])}`);
  }
  const chip = await page.locator("[data-testid=board-chip][data-card=BRD-1]").evaluate((el) => getComputedStyle(el).color);
  if (!byCard["BRD-1"].color || byCard["BRD-1"].color === "rgba(0, 0, 0, 0)" || byCard["BRD-1"].color !== chip) {
    throw new Error(`the outline is ${byCard["BRD-1"].color}, the card's status colour is ${chip}`);
  }
  console.log("PASS every card's region is outlined in its status colour");
  if (process.env.SCREENSHOT_DIR) await page.screenshot({ path: join(process.env.SCREENSHOT_DIR, "design-board.png") });

  // Fit shows every design; zoom steps; the board scrolls.
  const scroller = t("board-scroll");
  await t("fit").click();
  const fitted = await zoomValue();
  if (!(fitted < 100)) throw new Error(`Fit left the zoom at ${fitted}%`);
  const room = await scroller.evaluate((el) => el.scrollWidth <= el.clientWidth + 1 && el.scrollHeight <= el.clientHeight + 1);
  if (!room) throw new Error("after Fit some design is still out of view");
  console.log(`PASS Fit shows every design (${fitted}%)`);
  await t("fit").click();
  await t("zoom-in").click();
  if ((await zoomValue()) <= 100) throw new Error("zoom in did not grow the board");
  await t("zoom-out").click();
  await t("zoom-out").click();
  if ((await zoomValue()) >= 100) throw new Error("zoom out did not shrink the board");
  await t("fit").click();
  await t("fit").click();
  await t("zoom-in").click();
  await t("zoom-in").click();
  const moved = await scroller.evaluate((el) => {
    el.scrollLeft = 120;
    el.scrollTop = 80;
    return [el.scrollLeft, el.scrollTop];
  });
  if (moved[0] <= 0 || moved[1] <= 0) throw new Error(`the board does not pan: ${moved}`);
  console.log("PASS zoom and pan work");
  await t("fit").click();
  await t("fit").click();

  // A region's label opens its card; the board stays.
  await page.locator("[data-testid=board-label]", { hasText: "BRD-2" }).click();
  await expectPath("/projects/BRD/chains/work/cards/BRD-2/designs", "a region opens its card");
  await page.locator("[data-testid=board-region].current[data-card=BRD-2]").waitFor();
  console.log("PASS the open card's region is marked");

  // A drop on nothing moves nothing; a drop on a card without designs moves the reference there.
  const drag = async (fromLoc, to) => {
    const from = await fromLoc.boundingBox();
    await page.mouse.move(from.x + from.width / 2, from.y + from.height / 2);
    await page.mouse.down();
    await page.mouse.move(to.x, to.y, { steps: 6 });
    await t("board-drag").waitFor();
    await page.mouse.up();
  };
  const handle = (card) => page.locator(`[data-testid=board-region][data-card=${card}] [data-testid=board-handle]`);
  await drag(handle("BRD-1"), { x: 700, y: 600 });
  await new Promise((r) => setTimeout(r, 400));
  if ((await refsOf("BRD-1")).length !== 1 || (await refsOf("BRD-3")).length !== 0) throw new Error("a drop on nothing moved the reference");
  console.log("PASS a drop on nothing moves nothing");
  const chip3 = await page.locator("[data-testid=board-chip][data-card=BRD-3]").boundingBox();
  await drag(handle("BRD-1"), { x: chip3.x + chip3.width / 2, y: chip3.y + chip3.height / 2 });
  await page.waitForFunction(() => document.querySelector("[data-testid=board-region][data-card=BRD-3]"), undefined, { timeout: 10_000 });
  const [one, three] = [await refsOf("BRD-1"), await refsOf("BRD-3")];
  if (one.length !== 0 || three.length !== 1 || three[0].design !== login.id || JSON.stringify(three[0].region) !== JSON.stringify(regionA)) {
    throw new Error(`after the move BRD-1 has ${JSON.stringify(one)} and BRD-3 ${JSON.stringify(three)}`);
  }
  console.log("PASS dragging a handle onto a card moves the reference with its region");

  // Onto a region of another card: BRD-3's reference goes to BRD-2, which then has two.
  const target = await page.locator("[data-testid=board-region][data-card=BRD-2]").boundingBox();
  await drag(handle("BRD-3"), { x: target.x + target.width / 2, y: target.y + target.height - 10 });
  await page.waitForFunction(() => !document.querySelector("[data-testid=board-region][data-card=BRD-3]"), undefined, { timeout: 10_000 });
  if ((await refsOf("BRD-2")).length !== 2) throw new Error("the reference did not land on the card whose region it was dropped on");
  console.log("PASS dropping on another card's region moves the reference there");

  // A chain without designs says so.
  await page.goto(at("/projects/BRD/chains/bare/designs"));
  await t("designs-empty").waitFor({ timeout: 10_000 });
  if (!(await t("designs-empty").textContent()).includes("No designs yet")) throw new Error("the empty hint is missing");
  console.log("PASS a chain without designs shows the hint");

  // The three addresses of the view open it when typed (the daemon serves the page for each).
  for (const path of ["/chains/work/designs", "/projects/BRD/chains/work/cards/BRD-2/designs"]) {
    await page.goto(at(path));
    await t("design-board").waitFor({ timeout: 10_000 });
  }
  console.log("PASS the addresses of the view open it directly");

  if (errors.length) throw new Error(`the page threw: ${errors.join("; ")}`);
  console.log("web design board: PASS");
} catch (e) {
  console.error(`web design board: FAIL: ${e.message.slice(0, 800)}`);
  process.exitCode = 1;
} finally {
  await browser?.close();
  await cleanup(tok, home);
}
