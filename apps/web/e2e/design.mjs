// Browser test of "design first", against a real daemon and the workspace's `fake-agent` (never
// a real agent), in a real git ship.
// Usage: node e2e/design.mjs <tok binary> <fake-agent binary>   (cargo xtask e2e-web runs it)
//
// A card made with "Show me a design before the code" runs with an agent that opens a design
// thread and posts a diagram: the agent's instructions carry the "Design first" section, the
// status sentence says "Waiting for you to approve the design" with a link to Threads, the card
// cannot be handed in, and after Approve the sentence goes back to the plain one and the hand-in
// works. A card that has not started shows "Ask for a design" in the Threads tab, the action bar
// and the right-click menu; one click sets the flag and the button gives way to a line saying so.
// On a running card the same action sends the agent the instruction.
// SCREENSHOT_DIR, when set, receives two pictures: the empty Threads tab and the status sentence.

import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { checks, cleanup, launch } from "./helpers.mjs";

const [tokArg, agentArg] = process.argv.slice(2);
if (!tokArg || !agentArg) {
  console.error("usage: node e2e/design.mjs <tok binary> <fake-agent binary>");
  process.exit(2);
}
const tokBin = resolve(tokArg);
const fakeAgent = resolve(agentArg);
const home = mkdtempSync(join(tmpdir(), "toknila-design-"));
const run = (cmd, args) => execFileSync(cmd, args, { encoding: "utf8" });
const tok = (...args) => execFileSync(tokBin, args, { env: { ...process.env, TOKNILA_HOME: home }, encoding: "utf8" });
const git = (dir, ...args) =>
  run("git", ["-C", dir, "-c", "user.name=t", "-c", "user.email=t@example.com", "-c", "commit.gpgsign=false", ...args]);
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const logs = (key) => tok("logs", key);
const show = (key) => JSON.parse(tok("show", key, "--json"));

async function until(what, f) {
  for (const stop = Date.now() + 20_000; Date.now() < stop; await sleep(100)) {
    if (f()) return;
  }
  throw new Error(`timed out waiting for ${what}`);
}

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
  const steps = (...s) => s.map((x) => `'${x}'`).join(", ");
  const post = (json) => ["post", "/tickets/{ticket}/events", JSON.stringify(json)];
  // The agent prints its instructions, opens a design thread with a diagram, and holds.
  writeFileSync(
    join(home, "profiles", "designer.toml"),
    `command = ['${fakeAgent}', ${steps(
      "print", "ready", "flagfile", "--instr",
      ...post({ event: "open_thread", kind: "design", title: "Login screen" }),
      ...post({ event: "thread_version", thread: "1", format: "mermaid", body: "graph TD; Form --> Home" }),
      "hold",
    )}]\ninstructions_flag = "--instr"\n`,
  );
  writeFileSync(join(home, "profiles", "plain.toml"), `command = ['${fakeAgent}', ${steps("print", "working", "hold")}]\n`);
  tok("project", "new", "DS", "--repo", repo, "--base", "develop", "--profile", "plain");
  tok("project", "set", "DS", "--parallel", "6");
  tok("chain", "new", "work", "--project", "DS", "--no-autostart");
  // DS-1 asks for a design when it is made; DS-2 and DS-3 have not started; DS-4 will run.
  tok("ticket", "new", "work", "Login screen", "--profile", "designer", "--design-first");
  tok("ticket", "new", "work", "Settings page");
  tok("ticket", "new", "work", "Help page");
  tok("ticket", "new", "work", "Running page");
  const { url } = JSON.parse(tok("open", "--json"));

  browser = await launch();
  const page = await browser.newPage({ viewport: { width: 1280, height: 900 } });
  const errors = [];
  page.on("pageerror", (e) => errors.push(e.message));
  const { expectText } = checks(page);
  const shot = async (name) => {
    if (process.env.SCREENSHOT_DIR) await page.screenshot({ path: join(process.env.SCREENSHOT_DIR, `${name}.png`) });
  };
  const t = (id) => page.locator(`[data-testid=${id}]`);
  const open = (key) => page.goto(url.replace("/?", `/projects/DS/chains/work/cards/${key}?`));

  // 1. The card made with the choice runs, and its agent is told to show a design first.
  if (show("DS-1").ticket.design_first !== true) throw new Error("DS-1 should be design-first");
  tok("start", "DS-1");
  await until("the agent printed its instructions", () => logs("DS-1").includes("file:## Design first"));
  console.log("PASS the instructions of a design-first card carry the Design first section");
  await until("a design waits", () => show("DS-1").design_waiting === true);

  // 2. The status sentence says what the person has to do, with a link to Threads.
  await open("DS-1");
  await expectText(t("status-sentence"), "Waiting for you to approve the design", "the status sentence");
  await expectText(t("status-next"), "approve it or ask for changes", "with the next step");
  await shot("design-status");
  await t("status-tab-link").click();
  await expectText(t("thread-state"), "open", "the link opens the Threads tab");
  await page.locator("[data-testid=mermaid] svg").waitFor({ timeout: 20_000 });
  console.log("PASS the design is a diagram the person can judge");

  // 3. The hand-in is refused until a version is approved (the ship has a commit, so only the
  // missing approval stands in the way).
  const ship = show("DS-1").ship.path;
  writeFileSync(join(ship, "login.txt"), "login\n");
  git(ship, "add", "login.txt");
  git(ship, "commit", "--quiet", "-m", "login");
  const refused = (() => {
    try {
      tok("sim", "DS-1", "finish", "done");
      return "";
    } catch (e) {
      return String(e.stderr ?? e.message);
    }
  })();
  if (!refused.includes("hand in")) throw new Error(`the hand-in should be refused, got: ${refused}`);
  await page.getByRole("button", { name: "Approve" }).click();
  await expectText(t("thread-state"), "approved", "the design is approved");
  await page.getByRole("tab", { name: "Details" }).click();
  await expectText(t("status-sentence"), "An agent is working on it", "after Approve the plain sentence is back");
  tok("sim", "DS-1", "finish", "done");
  await until("DS-1 in review", () => show("DS-1").ticket.status === "review");
  console.log("PASS the hand-in works after the approval");

  // 4. A card that has not started: the Threads tab asks for a design.
  await open("DS-2");
  await page.getByRole("tab", { name: "Threads" }).click();
  await expectText(t("no-threads"), "waits for your approval before it writes any code", "the empty Threads tab explains");
  await shot("design-empty-threads");
  await t("ask-design").click();
  await until("DS-2 asks for a design", () => show("DS-2").ticket.design_first === true);
  await expectText(t("no-threads"), "A design is asked for", "the button gives way to a line");
  if (await t("ask-design").count()) throw new Error("the button should be gone once a design is asked for");
  console.log("PASS Ask for a design in the Threads tab sets design_first");

  // 5. The action bar and the right-click menu offer it too.
  await open("DS-3");
  await expectText(t("actions"), "Ask for a design", "the action bar offers it");
  await page.locator("a.node[data-key=DS-3]").click({ button: "right" });
  await t("menu-card-ask_design").waitFor({ timeout: 10_000 });
  await t("menu-card-ask_design").click();
  await until("DS-3 asks for a design", () => show("DS-3").ticket.design_first === true);
  await page.locator("a.node[data-key=DS-3]").click({ button: "right" });
  await t("menu-card-cancel").waitFor();
  if (await t("menu-card-ask_design").count()) throw new Error("the menu should not offer it twice");
  await page.keyboard.press("Escape");
  console.log("PASS the action bar and the menu offer it, once");

  // 6. A running card gets the instruction; a card in review is not offered it.
  tok("start", "DS-4");
  await until("DS-4 running", () => show("DS-4").ticket.status === "running");
  await open("DS-4");
  await expectText(t("actions"), "Ask for a design", "a running card offers it");
  await t("actions").getByRole("button", { name: "Ask for a design" }).click();
  await until("the instruction was written", () =>
    show("DS-4").comments.some((c) => (c.body.gist ?? "").startsWith("Before you go further, open a design thread")),
  );
  console.log("PASS on a running card it sends the agent an instruction");
  await open("DS-1");
  if ((await t("actions").getByRole("button", { name: "Ask for a design" }).count()) !== 0) {
    throw new Error("a card in review must not offer it");
  }
  console.log("PASS a card in review is not offered it");

  // 7. New card: the choice is off by default and is sent when ticked.
  await open("DS-2");
  await page.locator("button[data-testid=new-card]").click();
  const box = t("card-design-first");
  if (await box.isChecked()) throw new Error("the choice must be off by default");
  await t("card-prompt").fill("A new screen\nShow it.");
  await box.check();
  await t("create-card").click();
  await until("DS-5 exists with the flag", () => show("DS-5").ticket.design_first === true);
  console.log("PASS New card: Show me a design before the code is off by default and is sent when ticked");

  if (errors.length) throw new Error(`the page threw: ${errors.join("; ")}`);
  console.log("web design: PASS");
} catch (e) {
  console.error(`web design: FAIL: ${e.message.slice(0, 600)}`);
  if (process.env.SCREENSHOT_DIR) {
    await browser?.contexts()[0]?.pages()[0]?.screenshot({ path: join(process.env.SCREENSHOT_DIR, "design-failed.png") }).catch(() => {});
  }
  process.exitCode = 1;
} finally {
  await browser?.close();
  await cleanup(tok, home);
}
