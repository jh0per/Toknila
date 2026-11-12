// Browser test of a card's designs against a real daemon.
// Usage: node e2e/card-designs.mjs <path to tok binary>   (cargo xtask e2e-web runs it)
//
// A design from the project's library is set on a card from the card page (picked, a region drawn
// with the mouse); the page shows the mockup large with the region outlined and zoom; the card's
// node on the diagram shows a thumbnail whose picture really loaded; both survive a daemon
// restart; removing the design takes the thumbnail away. SCREENSHOT_DIR, when set, receives pictures.

import { execFileSync } from "node:child_process";
import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { checks, cleanup, launch } from "./helpers.mjs";

const tokBin = process.argv[2];
if (!tokBin) {
  console.error("usage: node e2e/card-designs.mjs <tok binary>");
  process.exit(2);
}
const home = mkdtempSync(join(tmpdir(), "toknila-cdesigns-"));
const tok = (...args) =>
  execFileSync(tokBin, args, { env: { ...process.env, TOKNILA_HOME: home }, encoding: "utf8" });

// A 1x1 png.
const PNG = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/q842iQAAAABJRU5ErkJggg==";

let browser;
try {
  tok("project", "new", "CDS", "--sim");
  writeFileSync(join(home, "c.toml"), `[chain]\nname = "work"\nautostart = false\n\n[[ticket]]\nid = "A"\ntitle = "Login screen"\nprofile = "sim"\n`);
  tok("chain", "import", join(home, "c.toml"), "--project", "CDS");
  const { url } = JSON.parse(tok("open", "--json"));
  const origin = new URL(url).origin;
  const bearer = new URL(url).searchParams.get("token");
  const up = await fetch(`${origin}/v1/projects/CDS/designs?name=login.png`, {
    method: "POST",
    headers: { Authorization: `Bearer ${bearer}`, "Content-Type": "application/octet-stream" },
    body: Buffer.from(PNG, "base64"),
  });
  if (up.status !== 201) throw new Error(`the upload answered ${up.status}`);

  browser = await launch();
  const page = await browser.newPage({ viewport: { width: 1280, height: 900 } });
  const errors = [];
  page.on("pageerror", (e) => errors.push(e.message));
  const { expectText } = checks(page);
  const t = (id) => page.locator(`[data-testid=${id}]`);
  const loaded = (sel) =>
    page.waitForFunction((s) => {
      const img = document.querySelector(s);
      return img && img.complete && img.naturalWidth === 1;
    }, sel, { timeout: 10_000 });
  const at = (path) => url.replace("/?", `${path}?`);

  await page.goto(at("/projects/CDS/chains/work/cards/CDS-1"));
  await t("card-designs-none").waitFor({ timeout: 10_000 });
  console.log("PASS a card without designs says so and offers Add design");

  // Pick the design and draw a region on it with the mouse.
  await t("add-design").click();
  await t("picker-item").first().click();
  const stage = t("picker-stage");
  await stage.waitFor();
  await loaded("[data-testid=picker-image]");
  const box = await stage.boundingBox();
  await page.mouse.move(box.x + box.width * 0.2, box.y + box.height * 0.1);
  await page.mouse.down();
  await page.mouse.move(box.x + box.width * 0.6, box.y + box.height * 0.4, { steps: 4 });
  await page.mouse.up();
  await expectText(t("picker-region-text"), "Region: x 20%, y 10%, 40% wide, 30% tall", "the drawn region is shown in percent");
  await t("picker-save").click();

  // The mockup is large on the page, with the region outlined and zoom.
  await t("mockup-image").waitFor({ timeout: 10_000 });
  await loaded("[data-testid=mockup-image]");
  const outline = await t("mockup-region").evaluate((el) => ({ left: el.style.left, top: el.style.top, width: el.style.width, height: el.style.height }));
  if (JSON.stringify(outline) !== JSON.stringify({ left: "20%", top: "10%", width: "40%", height: "30%" })) {
    throw new Error(`the outline is ${JSON.stringify(outline)}`);
  }
  console.log("PASS the mockup is on the card page with the region outlined");
  await t("mockup-zoom-in").click();
  await expectText(t("mockup-zoom-level"), "150%", "zoom in");
  const width = await t("mockup-stage").evaluate((el) => el.style.width);
  if (width !== "150%") throw new Error(`the picture is ${width} wide after zooming in`);
  await t("mockup-zoom-fit").click();
  await expectText(t("mockup-zoom-level"), "100%", "Fit goes back to 100%");
  if (process.env.SCREENSHOT_DIR) await page.screenshot({ path: join(process.env.SCREENSHOT_DIR, "card-designs.png") });

  // The card's node on the diagram has the thumbnail.
  await page.goto(at("/projects/CDS/chains/work"));
  await t("node-thumb").waitFor({ timeout: 10_000 });
  await loaded("[data-testid=node-thumb-image]");
  console.log("PASS the card's node shows a thumbnail of its first design");
  if (process.env.SCREENSHOT_DIR) await page.screenshot({ path: join(process.env.SCREENSHOT_DIR, "card-designs-node.png") });

  // Survives a daemon restart.
  tok("daemon", "stop");
  tok("daemon", "start");
  const again = JSON.parse(tok("open", "--json")).url;
  await page.goto(again.replace("/?", "/projects/CDS/chains/work/cards/CDS-1?"));
  await t("mockup-region").waitFor({ timeout: 20_000 });
  await loaded("[data-testid=mockup-image]");
  console.log("PASS the reference and its region survive a daemon restart");

  // Removing it takes the thumbnail away.
  await t("design-unlink").click();
  await t("card-designs-none").waitFor({ timeout: 10_000 });
  await page.goto(again.replace("/?", "/projects/CDS/chains/work?"));
  await t("end-node").waitFor();
  if ((await t("node-thumb").count()) !== 0) throw new Error("the thumbnail stays after the design was removed");
  console.log("PASS removing the design removes the thumbnail");

  if (errors.length) throw new Error(`the page threw: ${errors.join("; ")}`);
  console.log("web card designs: PASS");
} catch (e) {
  console.error(`web card designs: FAIL: ${e.message.slice(0, 800)}`);
  process.exitCode = 1;
} finally {
  await browser?.close();
  await cleanup(tok, home);
}
