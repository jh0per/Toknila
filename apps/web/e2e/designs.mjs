// Browser test of a project's designs against a real daemon.
// Usage: node e2e/designs.mjs <path to tok binary>   (cargo xtask e2e-web runs it)
//
// A png dropped on the Designs section appears as a card with a thumbnail that really loaded,
// its name, kind and day; the daemon serves the same bytes; after the daemon restarts the card is
// still there. A pasted Figma frame link is kept and, with no token, says to export the frame.
// The design opens large, is renamed and removed. SCREENSHOT_DIR, when set, receives a picture.

import { execFileSync } from "node:child_process";
import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { checks, cleanup, launch } from "./helpers.mjs";

const tokBin = process.argv[2];
if (!tokBin) {
  console.error("usage: node e2e/designs.mjs <tok binary>");
  process.exit(2);
}
const home = mkdtempSync(join(tmpdir(), "toknila-designs-"));
const tok = (...args) =>
  execFileSync(tokBin, args, { env: { ...process.env, TOKNILA_HOME: home }, encoding: "utf8" });

// A 1x1 png.
const PNG = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/q842iQAAAABJRU5ErkJggg==";

let browser;
try {
  tok("project", "new", "DES", "--sim");
  const { url } = JSON.parse(tok("open", "--json"));
  const at = (path) => url.replace("/?", `${path}?`);
  browser = await launch();
  const page = await browser.newPage({ viewport: { width: 1280, height: 800 } });
  const errors = [];
  page.on("pageerror", (e) => errors.push(e.message));
  const { expectText } = checks(page);
  const t = (id) => page.locator(`[data-testid=${id}]`);
  const card = (name) => page.locator(`[data-testid=design-card][data-name="${name}"]`);

  await page.goto(at("/projects/DES/settings/designs"));
  await expectText(t("designs-none"), "No designs yet", "a new project has no designs");

  // Drop a png on the section, the way a file manager does.
  await t("designs-drop").evaluate((zone, b64) => {
    const bytes = Uint8Array.from(atob(b64), (c) => c.charCodeAt(0));
    const dt = new DataTransfer();
    dt.items.add(new File([bytes], "home.png", { type: "image/png" }));
    zone.dispatchEvent(new DragEvent("drop", { dataTransfer: dt, bubbles: true, cancelable: true }));
  }, PNG);
  await card("home.png").waitFor({ timeout: 10_000 });
  await expectText(card("home.png").locator("[data-testid=design-meta]"), "PNG", "the card says its kind");
  const loaded = async () =>
    card("home.png").locator("[data-testid=design-thumb]").evaluate((img) => img.complete && img.naturalWidth === 1);
  await page.waitForFunction(() => {
    const img = document.querySelector("[data-testid=design-thumb]");
    return img && img.complete && img.naturalWidth === 1;
  }, undefined, { timeout: 10_000 });
  if (!(await loaded())) throw new Error("the thumbnail did not load");
  console.log("PASS the dropped png is a card with a thumbnail the daemon served");
  if (process.env.SCREENSHOT_DIR) await page.screenshot({ path: join(process.env.SCREENSHOT_DIR, "designs.png") });

  // A Figma link without a token is kept, with the hint.
  await t("figma-url").fill("https://www.figma.com/design/AbC123/App?node-id=1-2");
  await t("figma-add").click();
  await card("figma AbC123").waitFor({ timeout: 10_000 });
  await expectText(card("figma AbC123"), "Export the frame from Figma", "a link without a token shows the export hint");

  // A refusal is shown next to the section.
  await t("figma-url").fill("https://example.com/x");
  await t("figma-add").click();
  await expectText(t("designs-error"), "not a figma.com link", "a link from elsewhere is refused");

  // Open large.
  await card("home.png").locator("[data-testid=design-open]").click();
  await t("design-large").waitFor({ timeout: 10_000 });
  await page.keyboard.press("Escape");
  await t("design-viewer").waitFor({ state: "detached" });
  console.log("PASS the design opens large and closes");

  // Survives a daemon restart.
  tok("daemon", "stop");
  tok("daemon", "start");
  const again = JSON.parse(tok("open", "--json")).url;
  await page.goto(again.replace("/?", "/projects/DES/settings/designs?"));
  await card("home.png").waitFor({ timeout: 20_000 });
  await page.waitForFunction(() => {
    const img = document.querySelector("[data-testid=design-thumb]");
    return img && img.complete && img.naturalWidth === 1;
  }, undefined, { timeout: 10_000 });
  console.log("PASS the png and its thumbnail survive a daemon restart");

  // Rename and remove.
  await card("home.png").locator("[data-testid=design-rename]").click();
  await t("design-rename-input").fill("start.png");
  await t("design-rename-input").press("Enter");
  await card("start.png").waitFor({ timeout: 10_000 });
  console.log("PASS renamed to start.png");
  await card("start.png").locator("[data-testid=design-remove]").click();
  await card("start.png").locator("[data-testid=design-remove-sure]").click();
  await card("start.png").waitFor({ state: "detached", timeout: 10_000 });
  console.log("PASS removed");

  if (errors.length) throw new Error(`the page threw: ${errors.join("; ")}`);
  console.log("web designs: PASS");
} catch (e) {
  console.error(`web designs: FAIL: ${e.message.slice(0, 800)}`);
  process.exitCode = 1;
} finally {
  await browser?.close();
  await cleanup(tok, home);
}
