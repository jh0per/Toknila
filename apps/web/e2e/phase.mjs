// Browser test of the phase state against a real daemon: one finished and one active chain.
// Usage: node e2e/phase.mjs <path to tok binary>   (cargo xtask e2e-web runs it)
//
// The active chain reads active in the sidebar row and the header chip, the finished one reads
// finished in the Finished (1) group, the header chip and the Finish node. The group is closed
// by default, opens for the selected finished chain and remembers its state per project.

import { execFileSync } from "node:child_process";
import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { checks, cleanup, launch } from "./helpers.mjs";

const tokBin = process.argv[2];
if (!tokBin) {
  console.error("usage: node e2e/phase.mjs <tok binary>");
  process.exit(2);
}
const home = mkdtempSync(join(tmpdir(), "toknila-phase-"));
const tok = (...args) =>
  execFileSync(tokBin, args, { env: { ...process.env, TOKNILA_HOME: home }, encoding: "utf8" });

const chainFile = (name, cards) =>
  `[chain]\nname = "${name}"\nautostart = true\n` +
  cards
    .map((c) => `\n[[ticket]]\nid = "${c.id}"\ntitle = "${c.id}"\nprofile = "sim"\n${c.done ? "done = true\n" : ""}`)
    .join("");

let browser;
try {
  tok("project", "new", "PHA");
  tok("review-mode", "human", "--project", "PHA");
  writeFileSync(join(home, "old.toml"), chainFile("old", [{ id: "O1", done: true }, { id: "O2", done: true }]));
  writeFileSync(join(home, "now.toml"), chainFile("now", [{ id: "N1" }, { id: "N2" }]));
  tok("chain", "import", join(home, "old.toml"), "--project", "PHA");
  tok("chain", "import", join(home, "now.toml"), "--project", "PHA");
  const phaseOf = (name) => JSON.parse(tok("chain", "show", name, "--json")).summary.phase;
  if (phaseOf("old") !== "finished") throw new Error(`old should be finished, is ${phaseOf("old")}`);
  if (phaseOf("now") !== "active") throw new Error(`now should be active, is ${phaseOf("now")}`);
  if (!tok("chain", "show", "now").startsWith("now  active")) throw new Error("tok chain show does not print the phase word");
  const { url } = JSON.parse(tok("open", "--json"));

  browser = await launch();
  const page = await browser.newPage({ viewport: { width: 1280, height: 800 } });
  const errors = [];
  page.on("pageerror", (e) => errors.push(e.message));
  const { expectText } = checks(page);
  const item = (name) => page.locator(`[data-testid=chains] li[data-chain-item="${name}"]`);
  const group = page.locator("[data-testid=finished-group]");

  await page.goto(url.replace("/?", "/projects/PHA/chains/now?"));
  await expectText(item("now"), "active", "the active chain's row says active");
  await expectText(page.locator("[data-testid=chain-phase]"), "active", "the header chip says active");
  await expectText(page.locator("[data-testid=end-node-text]"), "left", "the Finish node counts what is left");
  await expectText(group, "Finished (1)", "the finished chain sits in the Finished group");
  if ((await item("old").count()) !== 0) throw new Error("the finished chain is in the list while the group is closed");
  console.log("PASS the Finished group is closed by default");

  await page.locator("[data-testid=finished-toggle]").click();
  await expectText(item("old"), "finished", "the opened group lists the finished chain");
  if ((await item("old").locator(".dim").count()) !== 1) throw new Error("the finished row is not dimmed");
  console.log("PASS the finished row is dimmed");

  await page.reload();
  await item("old").waitFor();
  console.log("PASS the open group is remembered after a reload");

  await item("old").click();
  await expectText(page.locator("[data-testid=chain-phase]"), "finished", "the header chip says finished");
  await expectText(page.locator("[data-testid=end-node-text]"), "Finished ·", "the Finish node says Finished with the date");

  // Closed and remembered, but a selected finished chain keeps the group open.
  await page.locator("[data-testid=finished-toggle]").click();
  await item("old").waitFor();
  console.log("PASS the selected finished chain stays visible");
  await item("now").click();
  await item("old").waitFor({ state: "detached" });
  await page.reload();
  await expectText(group, "Finished (1)", "the group is back after a reload");
  if ((await item("old").count()) !== 0) throw new Error("the closed state was not remembered");
  console.log("PASS the closed state is remembered per project");

  if (errors.length) throw new Error(`the page threw: ${errors.join("; ")}`);
  console.log("web phase: PASS");
} catch (e) {
  console.error(`web phase: FAIL: ${e.message.slice(0, 800)}`);
  process.exitCode = 1;
} finally {
  await browser?.close();
  await cleanup(tok, home);
}
