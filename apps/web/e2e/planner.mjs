// Browser test of the planner entry against a real daemon. No real agent is launched:
// the planner profile runs `fake-agent`, which plays what the planner's tools do with its run
// token (it creates a chain and two cards over the planner routes).
// Usage: node e2e/planner.mjs <tok binary> <fake-agent binary>   (cargo xtask e2e-web runs it)
// needs: fake-agent
//
// The sidebar's Assistant row opens the roadmap with the assistant's panel beside it and the
// roadmap chip on (not running, no terminal yet); the panel's expand control opens the same chat at
// full width at /projects/KEY/planner, with its Terminal tab and no "Plan so far" diagram. A
// project without epics asks "What do you want to build?"; typing there and pressing Enter opens the
// panel, starts the planner in the project's repository and the text reaches it. Its terminal is live,
// the epic and cards it makes show in the sidebar and on the epic's page, the box of the chat and the
// terminal both reach it. Stop ends it.

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
    "post", "/planner/suggestion", JSON.stringify({ text: "Start the design card", card: "WEB-1", action: "start" }),
    // The replies are long and scroll; the line the page waits for comes last.
    "print", "planner ready",
    // The planner says its next step before it is greeted, replaces it after the second line, and
    // after the third line asks the window to show a card (the last thing it does).
    // It answers the first line with `say`, which the person reads in the chat.
    "echo",
    "post", "/planner/say", JSON.stringify({ text: "Planned two cards", about: "WEB-1" }),
    "echo",
    "post", "/planner/suggestion", JSON.stringify({ text: "Nothing waits for you" }),
    "echo",
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
  // The entry is the project's Assistant row, above its Chains row (the tree: Roadmap, Assistant, Chains).
  const entry = page.locator("[data-testid=planner-entry]");
  await expectText(entry, "Assistant", "the assistant entry is a row of the project's node");
  const entryBefore = await page.evaluate(() => {
    const e = document.querySelector("[data-testid=planner-entry]");
    const c = document.querySelector("[data-testid=chains]");
    return Boolean(e && c && e.compareDocumentPosition(c) & Node.DOCUMENT_POSITION_FOLLOWING);
  });
  if (!entryBefore) throw new Error("the assistant row should come before the chains");
  if ((await page.locator("[data-testid=planner-entry]").count()) !== 1) throw new Error("expected one planner entry");

  const hint = await page.locator("[data-testid=planner-link]").getAttribute("title");
  if (hint !== "plans epics on the roadmap, cards inside an epic") throw new Error(`the Assistant row's hint is: ${hint}`);
  if ((await page.locator("[data-testid=planner-link]").getAttribute("href")).includes("/planner")) throw new Error("the Assistant row still links to the screen");
  await page.locator("[data-testid=planner-link]").click();
  await expectPath("/projects/WEB", "the Assistant row opens the roadmap");
  await expectText(page.locator("[data-testid=assistant-state]"), "not running", "the panel says the planner is not running");
  if ((await page.locator("[data-testid=assistant-panel] [data-testid=context-chip][data-context=roadmap]").count()) !== 1) throw new Error("the roadmap chip is not on");
  // The panel's expand control opens the chat at full width: no diagram, the chat tab first.
  await page.locator("[data-testid=assistant-expand]").click();
  await expectPath("/projects/WEB/planner", "the expand control opens the full-width chat");
  await expectText(page.locator("[data-testid=planner-state]"), "not running", "the planner is not running");
  for (const id of ["planner-chain", "planner-empty", "canvas"]) {
    if ((await page.locator(`[data-testid=${id}]`).count()) !== 0) throw new Error(`the assistant's screen still draws ${id}`);
  }
  console.log("PASS the full-width chat has no Plan-so-far diagram");
  if ((await page.locator("[data-testid=planner-tab-chat][aria-selected=true]").count()) !== 1) throw new Error("the chat is not the default tab");
  await expectText(page.locator("[data-testid=planner-idle]"), "Sending a message starts it", "the idle chat says sending starts the assistant");
  await expectText(page.locator("[data-testid=placeholder]"), "Tell the assistant what you want to build or change", "an empty chat asks what to build");
  if ((await page.locator("[data-testid=term-host]").count()) !== 0) throw new Error("the chat tab should not draw a terminal");
  console.log("PASS the chat tab draws no terminal");
  await shot("planner-idle");

  // Start, Stop and the profile are on the Terminal tab. Pick the planner profile of this test
  // (the built-in one would launch the real agent).
  await page.locator("[data-testid=planner-tab-terminal]").click();
  await expectText(page.locator("[data-testid=planner-start]"), "Start", "Start is on the Terminal tab");
  await page.locator("[data-testid=planner-profile]").selectOption("fake-planner");

  // The empty project asks what to build; Enter starts the planner and sends the text.
  await page.locator("[data-testid=projects] li[data-project=WEB] > .row a").click();
  await expectPath("/projects/WEB", "the project opens");
  await expectText(page.locator("[data-testid=no-chains]"), "What do you want to build?", "an empty project leads with the question");
  await expectText(page.locator("[data-testid=empty-new-chain]"), "New chain", "New chain is a quieter link below");
  await expectText(page.locator("[data-testid=empty-import]"), "Import", "so is Import");
  await shot("empty-project");
  await page.locator("[data-testid=empty-ask-input]").fill("hello planner");
  await page.keyboard.press("Enter");
  await page.locator("[data-testid=assistant-panel]").waitFor({ timeout: 10_000 });
  await expectText(page.locator("[data-testid=assistant-state]"), "running", "the text opened the panel and started the planner");
  // The words are in the chat as the person's bubble (stored before the planner saw them).
  await expectText(page.locator("[data-testid=chat-message][data-author=person]"), "hello planner", "the sent text is the person's bubble");
  await expectText(page.locator("[data-testid=chat-message][data-author=assistant]"), "Planned two cards", "the planner's say is its bubble");
  // The terminal is on the full-width screen.
  await page.locator("[data-testid=assistant-expand]").click();
  await expectPath("/projects/WEB/planner", "expanding opens the full-width chat");
  await page.locator("[data-testid=planner-tab-terminal]").click();
  await expectText(page.locator("[data-testid=planner-stop]"), "Stop", "Stop replaces Start on the Terminal tab");
  await expectText(term, "planner ready", "the terminal shows the planner's live output");
  // The terminal opened after the planner had answered, so the line may have scrolled off the small
  // screen: the daemon's scrollback is the proof the text reached it.
  const base = new URL(url);
  for (const stop = Date.now() + 15_000; ; await new Promise((r) => setTimeout(r, 150))) {
    const back = await (await fetch(`${base.origin}/v1/projects/WEB/planner/scrollback`, {
      headers: { authorization: `Bearer ${base.searchParams.get("token")}` },
    })).text();
    // The panel was open on the roadmap, so the line carries its context.
    if (back.includes("got:About the roadmap: hello planner")) break;
    if (Date.now() > stop) throw new Error("the text typed in the empty project did not reach the planner");
  }
  console.log("PASS the text typed in the empty project reached the planner");
  await expectText(page.locator("[data-testid=term-status]"), "live", "the terminal is live");
  // The assistant's terminal has the copy controls too (the card's are checked in terminal-copy).
  for (const id of ["term-copy", "term-copy-more"]) {
    if ((await page.locator(`[data-testid=term-status] [data-testid=${id}]`).count()) !== 1) throw new Error(`the assistant's terminal has no ${id}`);
  }
  console.log("PASS the assistant's terminal has the Copy controls");
  await shot("planner-running");

  // What the planner makes shows in the sidebar, and on the epic's page.
  await expectText(page.locator("[data-testid=chains] li"), "roadmap", "the epic is in the sidebar");
  await page.locator("[data-testid=chains] li a").first().click();
  await expectPath("/projects/WEB/chains/roadmap", "the epic opens from the sidebar");
  await expectText(page.locator("a[data-key=WEB-1]"), "Design the list", "its first card is on the diagram");
  await expectText(page.locator("a[data-key=WEB-2]"), "Build the list", "its second card is on the diagram");
  // The planner's attempt to start a card was refused: nothing runs.
  await expectText(page.locator("a[data-key=WEB-1] [data-testid=status]"), "ready", "the refused start left the card ready");
  // Back to the full-width chat through the panel (the Assistant row opens it beside the roadmap).
  await page.locator("[data-testid=planner-link]").click();
  await page.locator("[data-testid=assistant-expand]").click();
  await expectPath("/projects/WEB/planner", "the chat at full width again");

  // The planner's next step is on its screen with the button of its action, and the first row of
  // the inbox; the person clicks, the planner never does (its own start above was refused).
  const next = page.locator("[data-testid=suggestion]");
  await expectText(next, "Start the design card", "the planner's next step is on its screen");
  await expectText(page.locator("[data-testid=suggestion-button]"), "Start WEB-1", "with a button for its action");
  // The inbox in a window of its own, so this one keeps its terminal.
  const inbox = await browser.newPage();
  inbox.on("pageerror", (e) => errors.push(e.message));
  const inboxUrl = new URL(url);
  inboxUrl.pathname = "/inbox";
  await inbox.goto(inboxUrl.href);
  await checks(inbox).expectPath("/inbox", "the inbox opens");
  await checks(inbox).expectText(inbox.locator("[data-testid=inbox] > [data-testid=suggestion]"), "Start the design card", "the next step is in the inbox");
  const firstChild = await inbox.locator("[data-testid=inbox]").evaluate((el) => el.children[1]?.getAttribute("data-testid"));
  if (firstChild !== "suggestion") throw new Error(`the suggestion is not the first row of the inbox: ${firstChild}`);
  console.log("PASS the suggestion is the first row of the inbox");
  await shot("planner-next-step");
  // Doing its action ends it: the card starts (the simulated agent), the suggestion is cleared.
  await page.locator("[data-testid=suggestion-button]").click();
  await next.waitFor({ state: "detached" });
  await page.locator("[data-testid=chains] li a").first().click();
  await expectText(page.locator("a[data-key=WEB-1] [data-testid=status]"), "running", "the button started the card");
  await page.locator("[data-testid=planner-link]").click();
  await page.locator("[data-testid=assistant-expand]").click();
  await inbox.locator("[data-testid=suggestion]").waitFor({ state: "detached" });
  console.log("PASS the suggestion is gone once its action is done, from the inbox too");
  const tokenOf = new URL(url);
  const journal = async () => (await fetch(`${tokenOf.origin}/v1/events?since=0&limit=100000`, {
    headers: { authorization: `Bearer ${tokenOf.searchParams.get("token")}` },
  })).json();
  const kinds = (await journal()).filter((e) => e.entity === "window").map((e) => e.kind);
  if (kinds.join() !== "suggest,suggestion_cleared") throw new Error(`expected suggest, suggestion_cleared in the journal, got ${kinds}`);

  // The box of the chat is the way to talk to it; the terminal keeps working too.
  await page.locator("[data-testid=planner-tab-chat]").click();
  await page.locator("[data-testid=planner-ask-input]").fill("and a second message");
  await page.locator("[data-testid=planner-ask-send]").click();
  // The planner answers that line with its next step (and its reply to the post scrolls the echo
  // off the small terminal), so the next step is the proof the box reached it.
  // Its next suggestion says nothing waits for the person: text, no button.
  await expectText(next, "Nothing waits for you", "the box of the chat reaches the planner, which says its next step");
  if ((await page.locator("[data-testid=suggestion-button]").count()) !== 0 || (await next.count()) !== 1) {
    throw new Error("a suggestion without an action should be one row with no button");
  }
  await page.locator("[data-testid=planner-tab-terminal]").click();
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
  // The planner shows the card only after reading the typed line, so the card opening above is the
  // proof that the terminal reached it (its echo scrolls off the small terminal by now).

  // A reload keeps the planner running and its output.
  await page.reload();
  await expectText(page.locator("[data-testid=planner-state]"), "running", "after a reload it still runs");
  await page.locator("[data-testid=planner-tab-terminal]").click();
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
