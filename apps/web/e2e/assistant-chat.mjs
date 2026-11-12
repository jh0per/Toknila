// Browser test of the assistant's chat against a real daemon and the workspace's `fake-agent` as the
// planner (never a real agent). The fake planner plays what the planner's tools do with its run
// token: it reads a line from its terminal, adds a chain and a card, says something about the card
// (`say`), edits it, opens a thread on it and suggests the next step.
// Usage: node e2e/assistant-chat.mjs <tok binary> <fake-agent binary>   (cargo xtask e2e-web runs it)
// needs: fake-agent
//
// The assistant's screen is a chat with a record: what the person sends shows as their bubble before
// the planner has answered; the planner's `say` is an assistant bubble with the card as its chip;
// what it did shows as Toknila's lines with links; every message has a Copy control that fills the
// clipboard; a card discussion is in the same conversation once, with the card as its context; the
// record survives a reload; the last page is loaded and older ones come as the chat scrolls up; the
// terminal is the second tab.
// SCREENSHOT_DIR, when set, receives the chat in light and dark.

import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { checks, cleanup, launch, shoot } from "./helpers.mjs";

const [tokArg, agentArg] = process.argv.slice(2);
if (!tokArg || !agentArg) {
  console.error("usage: node e2e/assistant-chat.mjs <tok binary> <fake-agent binary>");
  process.exit(2);
}
const tokBin = resolve(tokArg);
const fakeAgent = resolve(agentArg);
const home = mkdtempSync(join(tmpdir(), "toknila-chat-"));
const run = (cmd, args) => execFileSync(cmd, args, { encoding: "utf8" });
const tok = (...args) => execFileSync(tokBin, args, { env: { ...process.env, TOKNILA_HOME: home }, encoding: "utf8" });
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
    "print", "planner ready",
    "echo",
    // The person's bubble is on screen for the whole wait: the planner has not answered yet.
    "sleep", "2500",
    "post", "/planner/chains", JSON.stringify({ name: "roadmap" }),
    "post", "/planner/chains/roadmap/cards", JSON.stringify({ title: "Design the list", prompt: "p" }),
    "post", "/planner/say", JSON.stringify({ text: "Planned one card: the list.", about: "WEB-1" }),
    "patch", "/planner/cards/WEB-1", JSON.stringify({ prompt: "A better prompt" }),
    "post", "/planner/cards/WEB-1/threads", JSON.stringify({ kind: "design", title: "The list" }),
    "post", "/planner/suggestion", JSON.stringify({ text: "Start the design card", card: "WEB-1", action: "start" }),
    "print", "planner done",
    "hold",
  ];
  const list = steps.map((s) => `'${s}'`);
  writeFileSync(join(home, "profiles", "fake-planner.toml"), `role = 'planner'\ncommand = ['${fakeAgent}', ${list.join(", ")}]\nstartup_secs = 0\n`);
  tok("project", "new", "WEB", "--repo", repo, "--base", "develop");
  const { url } = JSON.parse(tok("open", "--json"));
  const origin = new URL(url).origin;
  const bearer = { authorization: `Bearer ${new URL(url).searchParams.get("token")}`, "content-type": "application/json" };
  const api = (path, body) => fetch(`${origin}/v1${path}`, { method: "POST", headers: bearer, body: JSON.stringify(body) });

  browser = await launch();
  const context = await browser.newContext({ viewport: { width: 1280, height: 1000 }, permissions: ["clipboard-read", "clipboard-write"] });
  const page = await context.newPage();
  const errors = [];
  page.on("pageerror", (e) => errors.push(e.message));
  const { expectText, expectPath } = checks(page);
  const t = (id) => page.locator(`[data-testid=${id}]`);
  const at = (path) => {
    const u = new URL(url);
    u.pathname = path;
    return u.href;
  };

  // 1. The assistant's screen opens on the chat. Not running: the past conversation (none yet) and
  //    the box; Start, Stop and the profile are on the Terminal tab.
  await page.goto(at("/projects/WEB/planner"));
  await expectPath("/projects/WEB/planner", "the assistant's screen");
  if ((await t("planner-tab-chat").getAttribute("aria-selected")) !== "true") throw new Error("the chat is not the default tab");
  await expectText(t("planner-idle"), "Sending a message starts it", "the idle chat says sending starts the assistant");
  if ((await t("planner-stop").count()) + (await t("planner-start").count()) + (await t("planner-profile").count()) !== 0) {
    throw new Error("Stop, Start or the profile show on the chat tab");
  }
  console.log("PASS Stop, Start and the profile are not on the chat tab");
  await t("planner-tab-terminal").click();
  await expectText(t("planner-start"), "Start", "Start is on the Terminal tab");
  await t("planner-profile").selectOption("fake-planner");
  await t("planner-tab-chat").click();

  // 2. Sending stores the words first: the person's bubble shows while the planner has not answered.
  await t("planner-ask-input").fill("Plan the list\nplease");
  await t("planner-ask-send").click();
  const person = t("chat-message").and(page.locator("[data-author=person]"));
  await expectText(person, "Plan the list", "the sent text is the person's bubble");
  if ((await t("chat-text").first().innerText()) !== "Plan the list\nplease") throw new Error("the bubble lost the line break");
  await expectText(t("chat-waiting"), "The assistant is answering", "the chat says the assistant is answering");
  if ((await page.locator("[data-author=assistant]").count()) !== 0 || (await t("chat-did").count()) !== 0) {
    throw new Error("the planner's answer was there before the person's bubble was looked at");
  }
  if (!(await t("planner-ask-input").isDisabled())) throw new Error("the box does not wait while the assistant answers");
  console.log("PASS the person's bubble is there before the planner has answered, and the box waits");
  await shoot(page, "assistant-chat-waiting");

  // 3. The planner's say is an assistant bubble with the card as its chip; what it did is lines.
  const said = page.locator("[data-author=assistant]");
  await expectText(said, "Planned one card: the list.", "the planner's say is an assistant bubble");
  await expectText(said.locator("[data-testid=chat-chip]"), "WEB-1", "with the card as its chip");
  await expectText(t("chat-did"), "suggested: Start the design card", "the suggestion is a line of Toknila's");
  const lines = await t("chat-did").allInnerTexts();
  const words = lines.map((l) => l.split("\n")[0].replace(/\s+/g, " ").trim());
  // A line that does not name its card gets the card's link after the words.
  const want = ["added chain roadmap", "added card WEB-1", "edited WEB-1", "opened thread 1 on WEB-1", "suggested: Start the design card · WEB-1"];
  if (JSON.stringify(words) !== JSON.stringify(want)) throw new Error(`Toknila's lines: expected ${JSON.stringify(want)}, got ${JSON.stringify(words)}`);
  console.log("PASS what the assistant did is Toknila's lines: added chain, added card, edited, opened thread, suggested");
  // Oldest first, newest at the bottom: the lines of its work around its words, in the order they happened.
  const order = await t("chat-items").locator("> li").evaluateAll((els) => els.map((el) => el.getAttribute("data-testid") + ":" + (el.getAttribute("data-author") ?? "")));
  const expectedOrder = ["chat-message:person", "chat-did:", "chat-did:", "chat-message:assistant", "chat-did:", "chat-did:", "chat-did:"];
  if (JSON.stringify(order) !== JSON.stringify(expectedOrder)) throw new Error(`the order of the record: ${JSON.stringify(order)}`);
  console.log("PASS the record is oldest first with the newest at the bottom");
  await t("chat-waiting").waitFor({ state: "detached" });
  if (await t("planner-ask-input").isDisabled()) throw new Error("the box still waits after the answer");
  await shoot(page, "assistant-chat");

  // 4. The lines are links: the card's key opens its page.
  await t("chat-did").nth(1).locator("a").click();
  await expectPath("/projects/WEB/chains/roadmap/cards/WEB-1", "a line's link opens the card");
  await page.goBack();
  await expectPath("/projects/WEB/planner", "back on the assistant's screen");
  await said.first().waitFor({ timeout: 10_000 });

  // 5. Copy puts a message's text on the clipboard.
  await page.evaluate(() => navigator.clipboard.writeText("stale"));
  await said.first().locator("[data-testid=chat-copy]").click();
  await expectText(said.first().locator("[data-testid=chat-copy]"), "Copied", "Copy says it copied");
  const clip = await page.evaluate(() => navigator.clipboard.readText());
  if (clip !== "Planned one card: the list.") throw new Error(`the clipboard holds ${JSON.stringify(clip)}`);
  console.log("PASS Copy puts the message's text on the clipboard");
  // The text itself is plain, selectable text.
  const select = await said.first().locator("[data-testid=chat-text]").evaluate((el) => getComputedStyle(el).userSelect);
  if (select === "none") throw new Error("a message's text is not selectable");
  console.log("PASS a message's text is selectable");

  // 6. A card discussion is part of the conversation, with the card as context, and stored once.
  const talk = await api("/tickets/WEB-1/discussion", { text: "Why this card first?" });
  if (!talk.ok) throw new Error(`discussion: ${talk.status}`);
  const onCard = t("chat-message").filter({ hasText: "Why this card first?" });
  await onCard.waitFor({ timeout: 10_000 });
  await expectText(onCard.locator("[data-testid=chat-chip]"), "WEB-1", "the card discussion's message has the card as its chip");
  await page.goto(at("/projects/WEB/chains/roadmap/cards/WEB-1"));
  await t("assistant-toggle").click();
  const inPanel = t("assistant-panel").locator("[data-testid=chat-message]").filter({ hasText: "Why this card first?" });
  await expectText(inPanel, "Why this card first?", "the panel beside the card has the discussion");
  if ((await inPanel.count()) !== 1) throw new Error("the message is in the panel more than once");
  const all = await (await fetch(`${origin}/v1/projects/WEB/conversation`, { headers: bearer })).json();
  const copies = all.messages.filter((m) => m.text === "Why this card first?");
  if (copies.length !== 1 || copies[0].about?.key !== "WEB-1") throw new Error(`the discussion in the conversation: ${JSON.stringify(copies)}`);
  console.log("PASS a card discussion is in the conversation once, with the card as context");

  // 7. The record survives a reload, and the last 50 are loaded with more on scroll up.
  for (let i = 1; i <= 60; i++) {
    const r = await api("/projects/WEB/conversation", { text: `filler ${String(i).padStart(2, "0")}` });
    if (!r.ok) throw new Error(`filler ${i}: ${r.status}`);
  }
  await page.goto(at("/projects/WEB/planner"));
  await expectText(t("chat-message"), "filler 60", "after a reload the newest message is there");
  const loaded = await t("chat-items").locator("> li").count();
  if (loaded !== 50) throw new Error(`the window should load the last 50, loaded ${loaded}`);
  if ((await t("chat-start").count()) !== 0) throw new Error("the chat says it is at the start with older messages unread");
  console.log("PASS the window loads the last 50");
  await t("chat-log").evaluate((el) => { el.scrollTop = 0; });
  await t("chat-start").waitFor({ timeout: 10_000 });
  const total = await t("chat-items").locator("> li").count();
  const record = all.messages.length + 60;
  if (total !== record) throw new Error(`scrolling up should load the rest: ${total} of ${record}`);
  await expectText(t("chat-message").first(), "Plan the list", "the oldest message is first after loading the rest");
  console.log("PASS scrolling to the top loads older messages");
  // The newest is still the last of the record.
  const last = await t("chat-message").last().innerText();
  if (!last.includes("filler 60")) throw new Error(`the newest message is not at the bottom: ${last.slice(0, 80)}`);

  // 8. The terminal is the second tab: the planner's own screen, with Stop.
  await t("planner-tab-terminal").click();
  await expectText(t("term-host"), "planner done", "the Terminal tab is the planner's screen");
  await expectText(t("planner-stop"), "Stop", "Stop is on the Terminal tab");
  await t("planner-stop").click();
  await expectText(t("planner-state"), "not running", "Stop ends the planner");
  await t("planner-tab-chat").click();
  await expectText(t("chat-message").last(), "filler 60", "the past conversation stays when the assistant is not running");
  await expectText(t("planner-idle"), "Sending a message starts it", "and the chat says sending starts it");
  if (errors.length) throw new Error(`the page threw: ${errors.join("; ")}`);
  console.log("web assistant chat: PASS");
} catch (e) {
  console.error(`web assistant chat: FAIL: ${e.message.slice(0, 800)}`);
  process.exitCode = 1;
} finally {
  await browser?.close();
  await cleanup(tok, home);
}
