// Browser test of the Inbox and Agents nodes under each project of the sidebar, against a real
// daemon and the workspace's `fake-agent` (never a real agent), in a real git ship.
// Usage: node e2e/sidebar-nodes.mjs <tok binary> <fake-agent binary>   (cargo xtask e2e-web runs it)
// needs: fake-agent
//
// Two projects: the second has a card that asks for a permission (a question waits) and a card
// that runs. While the second is closed its row carries the count and a pulsing dot; open, its
// Inbox lists the question and its Agents lists the running card; the first shows none of it.
// The row for everything shows the total, a project's Inbox row opens the page of that project,
// and the slot stepper under Agents changes that project's slots.

import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { checks, cleanup, launch } from "./helpers.mjs";

const [tokArg, agentArg] = process.argv.slice(2);
if (!tokArg || !agentArg) {
  console.error("usage: node e2e/sidebar-nodes.mjs <tok binary> <fake-agent binary>");
  process.exit(2);
}
const tokBin = resolve(tokArg);
const fakeAgent = resolve(agentArg);
const home = mkdtempSync(join(tmpdir(), "toknila-nodes-"));
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
  const steps = (...s) => s.map((x) => `'${x}'`).join(", ");
  writeFileSync(
    join(home, "profiles", "ask.toml"),
    `command = ['${fakeAgent}', ${steps("post", "/tickets/{ticket}/activity", JSON.stringify({ signal: "hook:pre_tool_use", tool: "Bash: cargo test" }), "post", "/tickets/{ticket}/permission", "{}", "hold")}]\n`,
  );
  writeFileSync(join(home, "profiles", "plain.toml"), `command = ['${fakeAgent}', ${steps("print", "working", "hold")}]\n`);

  tok("project", "new", "NA", "--sim");
  tok("project", "new", "NB", "--repo", repo, "--base", "develop", "--profile", "plain");
  tok("project", "set", "NB", "--parallel", "4");
  tok("chain", "new", "beta", "--project", "NB");
  tok("ticket", "new", "beta", "Asking", "--profile", "ask");
  tok("ticket", "new", "beta", "Holding", "--profile", "plain");
  const { url } = JSON.parse(tok("open", "--json"));

  browser = await launch();
  const page = await browser.newPage({ viewport: { width: 1280, height: 800 } });
  const errors = [];
  page.on("pageerror", (e) => errors.push(e.message));
  const { expectText, expectPath } = checks(page);
  const must = (ok, what) => {
    if (!ok) throw new Error(what);
    console.log(`PASS ${what}`);
  };
  const node = (key) => page.locator(`[data-testid=projects] li[data-project=${key}]`);
  const sub = (key, id) => node(key).locator(`li[data-node=${id}]`);
  const toggle = (key, id) => sub(key, id).locator("> .row [data-toggle]").first();

  await page.goto(url.replace("/?", "/projects/NA?"));

  // A closed project tells what must not be missed.
  await expectText(node("NB").locator("> .row [data-testid=project-waiting]"), "1", "the closed project's row carries the waiting count");
  await node("NB").locator("> .row .dot.pulse").waitFor({ timeout: 20_000 });
  console.log("PASS the closed project's row carries a pulsing dot while an agent runs");

  // The first project's nodes show nothing waiting and nothing running.
  await expectText(sub("NA", "Inbox").locator("[data-testid=project-inbox-count]"), "0", "the first project's Inbox shows none");
  await expectText(page.locator("[data-testid=running]"), "None running", "the first project runs nothing");

  // The second project's nodes, opened.
  await node("NB").locator("> .row [data-toggle]").click();
  await toggle("NB", "Inbox").click();
  await expectText(sub("NB", "Inbox").locator("[data-testid=project-inbox-count]"), "1", "its Inbox counts the question");
  const item = sub("NB", "Inbox").locator("[data-testid=project-inbox-item]");
  await expectText(item, "NB-1", "its Inbox lists the card");
  await expectText(item, "Agent questions", "and what kind of thing waits");
  await toggle("NB", "Agents").click();
  await expectText(sub("NB", "Agents").locator("[data-testid=run-card]"), "NB-2", "its running card shows under its own Agents row");
  must((await sub("NA", "Agents").locator("[data-testid=run-card]").count()) === 0, "the card is not under the first project");
  must(/^\d+ \/ 4$/.test((await sub("NB", "Agents").locator("[data-testid=project-slots]").textContent()) ?? ""), "its Agents row says running out of 4 slots");

  // The row for everything, and the page of one project.
  await expectText(page.locator("[data-testid=inbox-count]"), "1", "the row for everything shows the total");
  await page.locator("[data-testid=inbox-link]").click();
  await expectPath("/inbox", "the row for everything opens the inbox of all projects");
  await sub("NB", "Inbox").locator("[data-testid=project-inbox-link]").click();
  await expectPath("/projects/NB/inbox", "a project's Inbox row opens its page");
  await expectText(page.locator("[data-testid=inbox-project]"), "NB", "the page says which project it shows");
  await expectText(page.locator("[data-testid=inbox]"), "NB-1", "the page lists its question");
  await page.goto(url.replace("/?", "/projects/NA/inbox?"));
  await expectText(page.locator("[data-testid=inbox-project]"), "NA", "the first project's page says which it shows");
  await expectText(page.locator("[data-testid=inbox]"), "Nothing needs a person", "the first project's page shows none of the second's");
  must((await page.locator("[data-testid=inbox] >> text=NB-1").count()) === 0, "the other project's card is not on the page");

  // The stepper under Agents changes that project's slots.
  await page.goto(url.replace("/?", "/projects/NB?"));
  await expectText(page.locator("[data-testid=slot-limit]"), "4", "the stepper shows the project's slots");
  await page.locator("[data-testid=slots-more]").click();
  await expectText(page.locator("[data-testid=slot-limit]"), "5", "+ raises them");
  await expectText(sub("NB", "Agents").locator("[data-testid=project-slots]"), "/ 5", "the Agents row follows");

  if (errors.length) throw new Error(`the page threw: ${errors.join("; ")}`);
  console.log("web sidebar-nodes: PASS");
} catch (e) {
  console.error(`web sidebar-nodes: FAIL: ${e.message.slice(0, 800)}`);
  process.exitCode = 1;
} finally {
  await browser?.close();
  await cleanup(tok, home);
}
