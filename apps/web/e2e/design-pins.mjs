// Browser test of the assistant's pins on designs, against a real daemon. No real agent is
// launched: the planner profile runs `fake-agent`, which pins a question with its run token.
// Usage: node e2e/design-pins.mjs <tok binary> <fake-agent binary>   (cargo xtask e2e-web runs it)
//
// The fake planner pins a question on a design a card references. The inbox lists it as a
// question of the assistant; the card page shows the numbered marker on the mockup with the text;
// the person answers in place; the row leaves the inbox; the card's prompt carries the question and
// the answer under "From the design"; the planner's plan shows the answer; the project page shows
// the pin on the design opened large.

import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { checks, cleanup, launch } from "./helpers.mjs";

const [tokArg, agentArg] = process.argv.slice(2);
if (!tokArg || !agentArg) {
  console.error("usage: node e2e/design-pins.mjs <tok binary> <fake-agent binary>");
  process.exit(2);
}
const tokBin = resolve(tokArg);
const fakeAgent = resolve(agentArg);
const home = mkdtempSync(join(tmpdir(), "toknila-pins-"));
const run = (cmd, args, opts = {}) => execFileSync(cmd, args, { encoding: "utf8", ...opts });
const tok = (...args) => run(tokBin, args, { env: { ...process.env, TOKNILA_HOME: home } });
const git = (dir, ...args) =>
  run("git", ["-C", dir, "-c", "user.name=t", "-c", "user.email=t@example.com", "-c", "commit.gpgsign=false", ...args]);

// A 1x1 png.
const PNG = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/q842iQAAAABJRU5ErkJggg==";
const QUESTION = "Is the logo fixed?";
const ANSWER = "Yes, fixed at the top left.";

let browser;
try {
  const repo = join(home, "repo");
  mkdirSync(repo, { recursive: true });
  git(repo, "init", "--quiet");
  git(repo, "symbolic-ref", "HEAD", "refs/heads/develop");
  writeFileSync(join(repo, "README.md"), "hi\n");
  git(repo, "add", "README.md");
  git(repo, "commit", "--quiet", "-m", "init");
  mkdirSync(join(home, "profiles"), { recursive: true });
  const steps = [
    "post", "/planner/pins", JSON.stringify({ design: "login.png", region: { x: 12, y: 30, w: 40, h: 20 }, text: QUESTION, card: "PIN-1" }),
    "print", "pinned",
    "hold",
  ];
  const list = steps.map((s) => `'${s}'`);
  writeFileSync(join(home, "profiles", "fake-planner.toml"), `role = 'planner'\ncommand = ['${fakeAgent}', ${list.join(", ")}]\nstartup_secs = 0\n`);
  tok("project", "new", "PIN", "--repo", repo, "--base", "develop");
  writeFileSync(join(home, "c.toml"), `[chain]\nname = "work"\nautostart = false\n\n[[ticket]]\nid = "A"\ntitle = "Login screen"\nprompt = "Build the login screen."\nprofile = "sim"\n`);
  tok("chain", "import", join(home, "c.toml"), "--project", "PIN");
  const { url } = JSON.parse(tok("open", "--json"));
  const origin = new URL(url).origin;
  const bearer = new URL(url).searchParams.get("token");
  const api = async (method, path, body, type = "application/json") => {
    const r = await fetch(`${origin}/v1${path}`, {
      method,
      headers: { Authorization: `Bearer ${bearer}`, "Content-Type": type },
      body: body === undefined ? undefined : type === "application/json" ? JSON.stringify(body) : body,
    });
    if (!r.ok) throw new Error(`${method} ${path} answered ${r.status}: ${await r.text()}`);
    return r.status === 204 ? undefined : r.json();
  };
  const design = await api("POST", "/projects/PIN/designs?name=login.png", Buffer.from(PNG, "base64"), "application/octet-stream");
  await api("PATCH", "/tickets/PIN-1", { designs: [{ design: design.id }] });

  browser = await launch();
  const page = await browser.newPage({ viewport: { width: 1280, height: 900 } });
  const errors = [];
  page.on("pageerror", (e) => errors.push(e.message));
  const { expectText } = checks(page);
  const t = (id) => page.locator(`[data-testid=${id}]`);
  const at = (path) => url.replace("/?", `${path}?`);

  // The inbox is open while the planner pins: the row arrives by itself.
  await page.goto(at("/inbox"));
  await api("POST", "/projects/PIN/planner/start", { profile: "fake-planner" });
  await expectText(t("design-question"), "login.png: Is the logo fixed?", "the open pin is a question of the assistant in the inbox");
  await expectText(page.locator("[data-testid=inbox-kind]"), "The assistant asks about a design", "the row says whose question it is");
  if (process.env.SCREENSHOT_DIR) await page.screenshot({ path: join(process.env.SCREENSHOT_DIR, "pins-inbox.png") });

  // The card page shows the marker on the mockup, with the text.
  await page.goto(at("/projects/PIN/chains/work/cards/PIN-1"));
  await t("pin-marker").waitFor({ timeout: 10_000 });
  const marker = await t("pin-marker").evaluate((el) => ({ n: el.dataset.number, state: el.dataset.state, left: el.style.left, top: el.style.top }));
  if (JSON.stringify(marker) !== JSON.stringify({ n: "1", state: "open", left: "12%", top: "30%" })) throw new Error(`the marker is ${JSON.stringify(marker)}`);
  await expectText(t("pin-text"), QUESTION, "the card page shows the question under the mockup");
  await t("pin-marker").click();
  const outline = await t("pin-region").evaluate((el) => ({ left: el.style.left, top: el.style.top, width: el.style.width, height: el.style.height }));
  if (JSON.stringify(outline) !== JSON.stringify({ left: "12%", top: "30%", width: "40%", height: "20%" })) throw new Error(`the pin's region is ${JSON.stringify(outline)}`);
  console.log("PASS the pin is on the card's mockup, numbered, with its region");
  if (process.env.SCREENSHOT_DIR) await page.screenshot({ path: join(process.env.SCREENSHOT_DIR, "pins-card.png") });

  // The person answers in place.
  await t("pin-input").fill(ANSWER);
  await t("pin-send").click();
  await expectText(t("pin-answer"), ANSWER, "the answer shows in place");
  if ((await t("pin-marker").evaluate((el) => el.dataset.state)) !== "answered") throw new Error("the marker stays open after the answer");

  // The row leaves the inbox; the prompt carries the answer; the plan shows it to the planner.
  await page.goto(at("/inbox"));
  await page.waitForFunction(() => document.querySelectorAll("[data-testid=design-question]").length === 0, null, { timeout: 10_000 });
  console.log("PASS an answered pin leaves the inbox");
  const card = await api("GET", "/tickets/PIN-1");
  const section = `## From the design\n- login.png #1 (x 12%, y 30%, 40% wide, 20% tall): ${QUESTION} - ${ANSWER}`;
  if (!card.ticket.prompt.startsWith("Build the login screen.") || !card.ticket.prompt.endsWith(section)) {
    throw new Error(`the prompt is ${JSON.stringify(card.ticket.prompt)}`);
  }
  console.log("PASS the card's prompt carries the question and the answer under From the design");

  // The project page shows the pin on the design opened large.
  await page.goto(at("/projects/PIN/settings/designs"));
  await t("design-open").click();
  await t("design-large").waitFor({ timeout: 10_000 });
  await t("pin-marker").waitFor({ timeout: 10_000 });
  await expectText(t("pin-text"), QUESTION, "the project page shows the pin on the design");

  await api("POST", "/projects/PIN/planner/stop", {});
  if (errors.length) throw new Error(`the page threw: ${errors.join("; ")}`);
  console.log("web design pins: PASS");
} catch (e) {
  console.error(`web design pins: FAIL: ${e.message.slice(0, 800)}`);
  process.exitCode = 1;
} finally {
  await browser?.close();
  await cleanup(tok, home);
}
