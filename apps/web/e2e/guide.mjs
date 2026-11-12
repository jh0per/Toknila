// Browser test of the Guide page against a real daemon.
// Usage: node e2e/guide.mjs <path to tok binary>   (cargo xtask e2e-web runs it)
//
// /guide opens the Guide. The onboarding page (no project), a project's empty state and the
// sidebar's foot each link to it. The page has the eight sections in order, each with a picture
// that loaded and a Show me link: off, with a reason, without a project; with one, "Plan with the
// assistant" opens that project's assistant screen. The page asks for nothing but the daemon.

import { execFileSync } from "node:child_process";
import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { checks, cleanup, launch } from "./helpers.mjs";

const tokBin = process.argv[2];
if (!tokBin) {
  console.error("usage: node e2e/guide.mjs <tok binary>");
  process.exit(2);
}
const home = mkdtempSync(join(tmpdir(), "toknila-guide-"));
const tok = (...args) => execFileSync(tokBin, args, { env: { ...process.env, TOKNILA_HOME: home }, encoding: "utf8" });
const IDS = ["add-project", "designs", "plan", "start", "inbox", "review", "release", "where"];

let browser;
try {
  const { url } = JSON.parse(tok("open", "--json"));
  const origin = new URL(url).origin;
  const at = (path) => url.replace("/?", `${path}?`);
  browser = await launch();
  const page = await browser.newPage({ viewport: { width: 1280, height: 800 } });
  const errors = [];
  const foreign = [];
  page.on("pageerror", (e) => errors.push(e.message));
  page.on("request", (r) => {
    const u = r.url();
    if (!u.startsWith("data:") && !u.startsWith("blob:") && new URL(u).origin !== origin) foreign.push(u);
  });
  const { expectText, expectPath } = checks(page);
  const t = (id) => page.locator(`[data-testid=${id}]`);

  // No project: the onboarding page links to the Guide, whose Show me links are off with a reason.
  await page.goto(url);
  await t("onboarding").waitFor({ timeout: 10_000 });
  await expectText(t("onboarding-guide"), "How Toknila works", "the onboarding page offers the Guide");
  await t("onboarding-guide").click();
  await expectPath("/guide", "the onboarding link opens the Guide");
  await t("guide").waitFor({ timeout: 10_000 });
  const ids = await t("guide-section").evaluateAll((s) => s.map((x) => x.dataset.section));
  if (JSON.stringify(ids) !== JSON.stringify(IDS)) throw new Error(`the sections are ${ids}`);
  console.log("PASS the Guide has the eight sections in order");
  for (const id of IDS) {
    const link = t(`guide-show-${id}`);
    if ((await link.getAttribute("aria-disabled")) !== "true") throw new Error(`Show me of ${id} is not off without a project`);
    await expectText(t(`guide-show-${id}-why`), "Add a project first", `Show me of ${id} says why it is off`);
  }
  const loaded = await page.waitForFunction(
    () => [...document.querySelectorAll("[data-testid^=guide-picture-]")].filter((i) => i.complete && i.naturalWidth > 0).length === 8,
    null, { timeout: 10_000 },
  ).then(() => true, () => false);
  if (!loaded) throw new Error("not every section has a picture that loaded");
  console.log("PASS every section has a picture that loaded");
  await expectText(t("guide-toc"), "Release a finished chain", "the table of sections lists the last section");

  // A project: Show me opens its places; the empty state and the sidebar's foot link here.
  tok("project", "new", "GDE", "--sim");
  await page.goto(at("/projects/GDE"));
  await t("no-chains").waitFor({ timeout: 10_000 });
  await t("empty-guide").click();
  await expectPath("/guide", "the empty state links to the Guide");
  await t("guide-show-plan").click();
  await expectPath("/projects/GDE/planner", "Show me of Plan with the assistant opens the project's assistant screen");
  await page.goto(at("/"));
  await t("guide-link").click();
  await expectPath("/guide", "the sidebar's foot links to the Guide");
  if ((await t("guide-show-plan").getAttribute("aria-disabled")) === "true") throw new Error("Show me is off with a project");
  console.log("PASS Show me is on with a project");

  if (foreign.length) throw new Error(`requests beyond the daemon: ${foreign.join(", ")}`);
  console.log("PASS the page asked for nothing beyond the daemon");
  if (errors.length) throw new Error(`page errors: ${errors.join("; ")}`);
  console.log("PASS no page errors");
} catch (e) {
  console.error(`FAIL ${e.message}`);
  process.exitCode = 1;
} finally {
  await browser?.close();
  await cleanup(tok, home);
}
