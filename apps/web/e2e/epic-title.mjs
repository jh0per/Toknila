// Browser test of the New epic dialog against a real daemon.
// Usage: node e2e/epic-title.mjs <path to tok binary>   (cargo xtask e2e-web runs it)
//
// The dialog asks for a title and shows the slug it gives, updating as the person types. An epic
// made through it shows its title on the roadmap, in the sidebar and in the crumb, while its slug
// is the name of its folders and branches. A second epic with the same title gets another slug.

import { execFileSync } from "node:child_process";
import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { checks, cleanup, launch, session } from "./helpers.mjs";

const tokBin = process.argv[2];
if (!tokBin) {
  console.error("usage: node e2e/epic-title.mjs <tok binary>");
  process.exit(2);
}
const { home } = session();
const tok = (...args) =>
  execFileSync(tokBin, args, { env: { ...process.env, TOKNILA_HOME: home }, encoding: "utf8" });

let browser;
try {
  tok("project", "new", "ETL", "--sim");
  const { url } = JSON.parse(tok("open", "--json"));
  browser = await launch();
  const page = await browser.newPage({ viewport: { width: 1280, height: 800 } });
  const errors = [];
  page.on("pageerror", (e) => errors.push(e.message));
  const { expectText } = checks(page);
  const t = (id) => page.locator(`[data-testid=${id}]`);

  await page.goto(url.replace("/?", "/projects/ETL?"));
  await t("new-chain-plus").click();
  await t("chain-title").fill("MIDI controllers");
  await expectText(t("chain-slug-value"), "midi-controllers", "the slug line shows the slug of the title");
  await t("chain-title").fill("MIDI controllers pro");
  await expectText(t("chain-slug-value"), "midi-controllers-pro", "the slug line follows the title as it is typed");
  await t("chain-title").fill("MIDI controllers");
  await t("create-chain").click();

  await t("start-node").waitFor({ timeout: 10_000 });
  await expectText(t("epic-title"), "MIDI controllers", "Start shows the title");
  await expectText(t("chain-name-top"), "MIDI controllers", "the crumb shows the title");
  await expectText(page.locator("[data-chain-item=midi-controllers] .chain-name"), "MIDI controllers", "the sidebar row shows the title");
  await t("crumb-roadmap").click();
  await t("roadmap").waitFor({ timeout: 10_000 });
  await expectText(
    page.locator("[data-testid=roadmap-node][data-chain=midi-controllers]"),
    "MIDI controllers",
    "the roadmap row shows the title",
  );
  const shown = JSON.parse(tok("chain", "show", "midi-controllers", "--json"));
  if (shown.chain.title !== "MIDI controllers") throw new Error(`the daemon holds title ${shown.chain.title}`);
  console.log("PASS the daemon holds the title and the slug midi-controllers");

  // The slug is edited on request and then stays as typed.
  await t("new-chain-plus").click();
  await t("chain-title").fill("Other words");
  await t("chain-slug-change").click();
  await t("chain-name").fill("my-slug");
  await t("chain-title").fill("Still other words");
  if ((await t("chain-name").inputValue()) !== "my-slug") throw new Error("the slug did not stay as typed");
  await t("create-chain").click();
  await t("start-node").waitFor({ timeout: 10_000 });
  const second = JSON.parse(tok("chain", "show", "my-slug", "--json"));
  if (second.chain.title !== "Still other words") throw new Error(`title ${second.chain.title}`);
  console.log("PASS an edited slug stays as typed and is sent with the title");

  if (errors.length) throw new Error(`the page threw: ${errors.join("; ")}`);
  console.log("web epic-title: PASS");
} catch (e) {
  console.error(`web epic-title: FAIL: ${e.message.slice(0, 800)}`);
  process.exitCode = 1;
} finally {
  await browser?.close();
  await cleanup(tok, home);
}
