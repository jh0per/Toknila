// Browser test of removing a design that a card uses, against a real daemon.
// Usage: node e2e/design-delete.mjs <path to tok binary>   (cargo xtask e2e-web runs it)
//
// A card shows a design as its node's thumbnail. Removing the design from the library is
// refused with the card named; the window asks "Used by ... Remove anyway?" and, when the person
// says so, forces it: the library is empty, the card has no design, the node lost its thumbnail
// without a broken picture or an error on the page, and a system comment on the card says which
// design went. A second design nobody uses is removed without a question.

import { execFileSync } from "node:child_process";
import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { checks, cleanup, launch } from "./helpers.mjs";

const tokBin = process.argv[2];
if (!tokBin) {
  console.error("usage: node e2e/design-delete.mjs <tok binary>");
  process.exit(2);
}
const home = mkdtempSync(join(tmpdir(), "toknila-ddelete-"));
const tok = (...args) =>
  execFileSync(tokBin, args, { env: { ...process.env, TOKNILA_HOME: home }, encoding: "utf8" });

// A 1x1 png.
const PNG = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/q842iQAAAABJRU5ErkJggg==";

let browser;
try {
  tok("project", "new", "DDL");
  writeFileSync(join(home, "c.toml"), `[chain]\nname = "work"\nautostart = false\n\n[[ticket]]\nid = "A"\ntitle = "Login screen"\nprofile = "sim"\n`);
  tok("chain", "import", join(home, "c.toml"), "--project", "DDL");
  const { url } = JSON.parse(tok("open", "--json"));
  const origin = new URL(url).origin;
  const bearer = new URL(url).searchParams.get("token");
  const api = (path, init = {}) =>
    fetch(`${origin}/v1${path}`, { ...init, headers: { Authorization: `Bearer ${bearer}`, ...(init.headers ?? {}) } });
  const upload = async (name) => {
    const r = await api(`/projects/DDL/designs?name=${name}`, {
      method: "POST",
      headers: { "Content-Type": "application/octet-stream" },
      body: Buffer.from(PNG, "base64"),
    });
    if (r.status !== 201) throw new Error(`the upload of ${name} answered ${r.status}`);
    return (await r.json()).id;
  };
  const used = await upload("login.png");
  await upload("spare.png");
  const set = await api("/tickets/DDL-1", {
    method: "PATCH",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ designs: [{ design: used }] }),
  });
  if (!set.ok) throw new Error(`setting the card's design answered ${set.status}`);

  browser = await launch();
  const page = await browser.newPage({ viewport: { width: 1280, height: 900 } });
  const errors = [];
  page.on("pageerror", (e) => errors.push(e.message));
  // The refused delete (409) and the picture of a removed design (404) are logged by the browser
  // as failed loads; they are what this script provokes. Anything else the page logs is an error.
  page.on("console", (m) => {
    if (m.type() === "error" && !m.text().startsWith("Failed to load resource")) errors.push(m.text());
  });
  const { expectText } = checks(page);
  const t = (id) => page.locator(`[data-testid=${id}]`);
  const card = (name) => page.locator(`[data-testid=design-card][data-name="${name}"]`);
  const at = (path) => url.replace("/?", `${path}?`);

  // The node has its thumbnail, loaded.
  await page.goto(at("/projects/DDL/chains/work"));
  await t("node-thumb").waitFor({ timeout: 10_000 });
  await page.waitForFunction(() => {
    const img = document.querySelector("[data-testid=node-thumb-image]");
    return img && img.complete && img.naturalWidth === 1;
  }, undefined, { timeout: 10_000 });
  console.log("PASS the card's node shows the thumbnail of its design");

  // Removing the design asks, naming the card.
  await page.goto(at("/projects/DDL/settings/designs"));
  await card("login.png").waitFor({ timeout: 10_000 });
  await card("login.png").locator("[data-testid=design-remove]").click();
  await card("login.png").locator("[data-testid=design-remove-sure]").click();
  await expectText(t("design-used-text"), "Used by DDL-1. Remove anyway?", "the window names the card that uses the design");
  if ((await card("login.png").count()) !== 1) throw new Error("the design went before the person said so");
  // Cancel keeps everything; the question can be asked again.
  await t("design-used-cancel").click();
  await t("design-used").waitFor({ state: "detached", timeout: 10_000 });
  // The card stays armed ("Really remove"), so the question is asked again.
  await card("login.png").locator("[data-testid=design-remove-sure]").click();
  await expectText(t("design-used-text"), "Used by DDL-1. Remove anyway?", "the question is asked again");
  await t("design-force").click();
  await card("login.png").waitFor({ state: "detached", timeout: 10_000 });
  await t("design-used").waitFor({ state: "detached", timeout: 10_000 });
  console.log("PASS forcing it removes the design from the library");

  // A design nobody uses goes without a question.
  await card("spare.png").locator("[data-testid=design-remove]").click();
  await card("spare.png").locator("[data-testid=design-remove-sure]").click();
  await t("designs-none").waitFor({ timeout: 10_000 });
  if ((await t("design-used").count()) !== 0) throw new Error("an unused design asked a question");
  console.log("PASS a design nobody uses is removed without a question");

  // The node lost its thumbnail cleanly, and the card says which design went.
  await page.goto(at("/projects/DDL/chains/work"));
  await t("end-node").waitFor({ timeout: 10_000 });
  if ((await t("node-thumb").count()) !== 0) throw new Error("the node still has a thumbnail");
  if ((await page.locator("img").evaluateAll((all) => all.filter((i) => i.complete && i.naturalWidth === 0).length)) !== 0) {
    throw new Error("a broken picture is left on the diagram");
  }
  console.log("PASS the node lost its thumbnail, with no broken picture");
  await page.goto(at("/projects/DDL/chains/work/cards/DDL-1"));
  await t("card-designs-none").waitFor({ timeout: 10_000 });
  const detail = await (await api("/tickets/DDL-1")).json();
  const said = detail.comments.find((c) => c.author === "system");
  if (said?.body?.gist !== "Design login.png was removed from the library") {
    throw new Error(`the card's comment is ${JSON.stringify(said)}`);
  }
  console.log("PASS the card has no design and a system comment says which one went");

  if (errors.length) throw new Error(`the page threw: ${errors.join("; ")}`);
  console.log("web design delete: PASS");
} catch (e) {
  console.error(`web design delete: FAIL: ${e.message.slice(0, 800)}`);
  process.exitCode = 1;
} finally {
  await browser?.close();
  await cleanup(tok, home);
}
