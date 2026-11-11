// Browser test of the planner entry against a real daemon. No real agent is launched:
// the planner profile runs `fake-agent`, which plays what the planner's tools do with its run
// token (it creates a chain and two cards over the planner routes).
// Usage: node e2e/planner.mjs <tok binary> <fake-agent binary>   (cargo xtask e2e-web runs it)
//
// The sidebar has a "Plan with the assistant" entry at the top of the project's section; it opens
// the assistant's panel: not running, a centred box (no terminal yet). A project without chains
// asks "What do you want to build?"; typing there and pressing Enter starts the planner in the
// project's repository and the text reaches it. Its terminal is live, the diagram above shows the
// chain and cards the planner makes as they happen, the box under the terminal and the terminal
// itself both reach it. Stop ends it.

import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { checks, cleanup, launch } from "./helpers.mjs";

const [tokArg, agentArg] = process.argv.slice(2);
if (!tokArg || !agentArg) {
  console.error("usage: node e2e/planner.mjs <tok binary> <fake-agent binary>");
  process.exit(2);
}
const tokBin = resolve(tokArg);
const fakeAgent = resolve(agentArg);
const home = mkdtempSync(join(tmpdir(), "toknila-planner-"));
const run = (cmd, args, opts = {}) => execFileSync(cmd, args, { encoding: "utf8", ...opts });
const tok = (...args) => run(tokBin, args, { env: { ...process.env, TOKNILA_HOME: home } });
const git = (dir, ...args) =>
  run("git", ["-C", dir, "-c", "user.name=t", "-c", "user.email=t@example.com", "-c", "commit.gpgsign=false", ...args]);

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
    "post", "/planner/chains", JSON.stringify({ name: "roadmap", goal: "planned by the fake planner" }),
    "post", "/planner/chains/roadmap/cards", JSON.stringify({ title: "Design the list", prompt: "p", acceptance: ["a"] }),
    "post", "/planner/chains/roadmap/cards", JSON.stringify({ title: "Build the list", prompt: "p" }),
    "post", "/tickets/WEB-1/actions/start", "{}",
    // The replies are long and scroll; the line the page waits for comes last.
    "print", "planner ready",
    // After the third line the planner asks the window to show a card (the last thing it does).
    "echo", "echo", "echo",
    "post", "/planner/cards/WEB-1/show", "{}",
    "print", "planner done",
    "hold",
  ];
  const list = steps.map((s) => `'${s}'`);
  writeFileSync(join(home, "profiles", "fake-planner.toml"), `role = 'planner'\ncommand = ['${fakeAgent}', ${list.join(", ")}]\nstartup_secs = 0\n`);
  tok("project", "new", "WEB", "--repo", repo, "--base", "develop");
  // A second project, looked at by a second window: it must not follow the first one's assistant.
  const otherRepo = join(home, "other-repo");
  mkdirSync(otherRepo, { recursive: true });
  git(otherRepo, "init", "--quiet");
  git(otherRepo, "symbolic-ref", "HEAD", "refs/heads/develop");
  writeFileSync(join(otherRepo, "README.md"), "hi\n");
  git(otherRepo, "add", "README.md");
  git(otherRepo, "commit", "--quiet", "-m", "init");
  tok("project", "new", "OTH", "--repo", otherRepo, "--base", "develop");
  const { url } = JSON.parse(tok("open", "--json"));

  browser = await launch();
  const page = await browser.newPage();
  const errors = [];
  page.on("pageerror", (e) => errors.push(e.message));
  const elsewhere = await browser.newPage();
  elsewhere.on("pageerror", (e) => errors.push(e.message));
  const otherUrl = new URL(url);
  otherUrl.pathname = "/projects/OTH/planner";
  await elsewhere.goto(otherUrl.href);
  await checks(elsewhere).expectPath("/projects/OTH/planner", "the second window looks at the other project's assistant");
  const { expectText, expectPath } = checks(page);
  const term = page.locator("[data-testid=term-host]");
  const shot = async (name) => {
    if (process.env.SCREENSHOT_DIR) await page.screenshot({ path: join(process.env.SCREENSHOT_DIR, `${name}.png`) });
  };

  const webUrl = new URL(url);
  webUrl.pathname = "/projects/WEB";
  await page.goto(webUrl.href);
  // The entry is at the top of the project's section, above its chains.
  const entry = page.locator("[data-testid=chains] li").first();
  await expectText(entry, "Plan with the assistant", "the assistant entry is first in the project's section");
  if ((await page.locator("[data-testid=planner-entry]").count()) !== 1) throw new Error("expected one planner entry");

  await page.locator("[data-testid=planner-link]").click();
  await expectPath("/projects/WEB/planner", "selecting it opens the assistant's address");
  await expectText(page.locator("[data-testid=planner-state]"), "not running", "the planner is not running");
  await expectText(page.locator("[data-testid=planner-empty]"), "Plan so far", "the diagram is labelled");
  await expectText(page.locator("[data-testid=planner-idle]"), "Tell the assistant what you want to build or change", "a centred box asks what to build");
  if ((await page.locator("[data-testid=term-host]").count()) !== 0) throw new Error("the idle panel should not draw a terminal");
  console.log("PASS the idle panel draws no terminal");
  await shot("planner-idle");

  // Start with the planner profile of this test (the built-in one would launch the real agent).
  await page.locator("[data-testid=planner-profile]").selectOption("fake-planner");

  // The empty project asks what to build; Enter starts the planner and sends the text.
  await page.locator("[data-testid=projects] li[data-project=WEB] a").click();
  await expectPath("/projects/WEB", "the project opens");
  await expectText(page.locator("[data-testid=no-chains]"), "What do you want to build?", "an empty project leads with the question");
  await expectText(page.locator("[data-testid=empty-new-chain]"), "New chain", "New chain is a quieter link below");
  await expectText(page.locator("[data-testid=empty-import]"), "Import", "so is Import");
  await shot("empty-project");
  await page.locator("[data-testid=empty-ask-input]").fill("hello planner");
  await page.keyboard.press("Enter");
  await expectPath("/projects/WEB/planner", "sending moves to the assistant's screen");
  await expectText(page.locator("[data-testid=planner-state]"), "running", "the text started the planner");
  await expectText(page.locator("[data-testid=planner-stop]"), "Stop", "Stop replaces Start");
  await expectText(term, "planner ready", "the terminal shows the planner's live output");
  await expectText(term, "got:hello planner", "the text typed in the empty project reached the planner");
  await expectText(page.locator("[data-testid=term-status]"), "live", "the terminal is live");
  await shot("planner-running");

  // What the planner makes shows in the diagram above, live, and in the sidebar.
  await expectText(page.locator("[data-testid=planner-chain]"), "roadmap", "the diagram follows the chain the planner made");
  await expectText(page.locator("a[data-key=WEB-1]"), "Design the list", "its first card is on the diagram");
  await expectText(page.locator("a[data-key=WEB-2]"), "Build the list", "its second card is on the diagram");
  await expectText(page.locator("[data-testid=chains] li"), "roadmap", "the chain is in the sidebar");
  // The planner's attempt to start a card was refused: nothing runs.
  await expectText(page.locator("a[data-key=WEB-1] [data-testid=status]"), "ready", "the refused start left the card ready");

  // The box under the terminal is the way to talk to it; the terminal keeps working too.
  await page.locator("[data-testid=planner-ask-input]").fill("and a second message");
  await page.locator("[data-testid=planner-ask-send]").click();
  await expectText(term, "got:and a second message", "the box under the terminal reaches the planner");
  await term.click();
  await page.keyboard.type("typed in the terminal");
  await page.keyboard.press("Enter");
  // The planner shows a card right after this line, which takes the window off the terminal;
  // that this line reached it is checked after Back, in the scrollback.

  // The planner's last act is show_card: this window, on the assistant's screen, goes to the card
  // with a notice and Back; the window on the other project stays where it is.
  const notice = page.locator("[data-testid=opened-notice]");
  await expectPath("/projects/WEB/chains/roadmap/cards/WEB-1", "show_card opens the card's page");
  await expectText(page.locator("[data-testid=panel] .who"), "Design the list", "the card's panel shows it");
  await expectText(notice, "The assistant opened WEB-1", "a notice says the assistant opened it");
  await shot("planner-opened-card");
  const stream = new URL(url);
  const events = await (await fetch(`${stream.origin}/v1/events?since=0&limit=100000`, {
    headers: { authorization: `Bearer ${stream.searchParams.get("token")}` },
  })).json();
  const shown = events.filter((e) => e.entity === "window" && e.kind === "show_card");
  if (shown.length !== 1 || shown[0].payload.ticket !== "WEB-1" || shown[0].payload.project_key !== "WEB") {
    throw new Error(`expected one window.show_card for WEB-1 in the journal, got ${JSON.stringify(shown)}`);
  }
  console.log("PASS the event is in the journal: window.show_card WEB-1");
  if (new URL(elsewhere.url()).pathname !== "/projects/OTH/planner" || (await elsewhere.locator("[data-testid=opened-notice]").count()) !== 0) {
    throw new Error(`the window on the other project followed: ${elsewhere.url()}`);
  }
  console.log("PASS the window on the other project ignored it");
  await page.locator("[data-testid=opened-back]").click();
  await expectPath("/projects/WEB/planner", "Back returns to the assistant's screen");
  if ((await notice.count()) !== 0) throw new Error("the notice stayed after Back");
  console.log("PASS the notice is gone after Back");
  await expectText(term, "got:typed in the terminal", "what was typed in the terminal reached the planner");

  // A reload keeps the planner running and its output.
  await page.reload();
  await expectText(page.locator("[data-testid=planner-state]"), "running", "after a reload it still runs");
  await expectText(term, "planner done", "after a reload the scrollback has the output");

  // Stop ends it, and the panel offers Start again.
  await page.locator("[data-testid=planner-stop]").click();
  await expectText(page.locator("[data-testid=planner-state]"), "not running", "Stop ends the planner");
  await expectText(page.locator("[data-testid=term-status]"), "ended", "the terminal says it ended");
  await expectText(page.locator("[data-testid=planner-start]"), "Start", "Start is back");
  if (errors.length) throw new Error(`the page threw: ${errors.join("; ")}`);
  console.log("web planner: PASS");
} catch (e) {
  console.error(`web planner: FAIL: ${e.message.slice(0, 600)}`);
  process.exitCode = 1;
} finally {
  await browser?.close();
  await cleanup(tok, home);
}
