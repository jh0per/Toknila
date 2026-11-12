// Browser test of copying from the card's terminal against a real daemon.
// Usage: node e2e/terminal-copy.mjs <tok binary> <fake-agent binary>   (cargo xtask e2e-web runs it)
// needs: fake-agent
//
// The fake agent prints lines and asks the terminal to copy "from the agent" (OSC 52). The
// clipboard then holds that text; the Copy control copies a selection (a triple click on a line),
// else the visible screen; "Copy all" in its menu copies the scrollback; the copy key copies the
// selection; and a selection survives the window's height changing. In a second window whose
// clipboard refuses writes, the agent's copy shows "Copied text ready" and its Copy button puts
// the text on the clipboard once clicked.

import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { checks, cleanup, launch } from "./helpers.mjs";

const [tokArg, agentArg] = process.argv.slice(2);
if (!tokArg || !agentArg) {
  console.error("usage: node e2e/terminal-copy.mjs <tok binary> <fake-agent binary>");
  process.exit(2);
}
const tokBin = resolve(tokArg);
const fakeAgent = resolve(agentArg);
const home = mkdtempSync(join(tmpdir(), "toknila-copy-"));
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
  const fromAgent = Buffer.from("from the agent ✓").toString("base64");
  const lines = [];
  for (let i = 1; i <= 80; i++) lines.push("print", `scroll line ${i}`);
  const steps = [...lines, "print", "marker alpha", "osc52", fromAgent, "print", "marker omega", "hold"];
  writeFileSync(join(home, "profiles", "fake.toml"), `command = [${[fakeAgent, ...steps].map((s) => `'${s}'`).join(", ")}]\n`);
  tok("project", "new", "WEB", "--repo", repo, "--base", "develop", "--profile", "fake");
  tok("chain", "new", "work", "--project", "WEB", "--no-autostart");
  tok("ticket", "new", "work", "Copy task", "--prompt", "Say hello.");
  const { url } = JSON.parse(tok("open", "--json"));
  const cardUrl = url.replace("/?", "/projects/WEB/chains/work/cards/WEB-1?");

  browser = await launch();
  const errors = [];
  const open = async (context) => {
    const page = await context.newPage();
    page.on("pageerror", (e) => errors.push(e.message));
    await page.goto(cardUrl);
    return page;
  };
  const clip = (page) => page.evaluate(() => navigator.clipboard.readText());
  const expectClip = async (page, want, what) => {
    let got;
    for (const until = Date.now() + 8000; Date.now() < until; await page.waitForTimeout(100)) {
      got = await clip(page);
      if (got === want || (want instanceof RegExp && want.test(got))) break;
    }
    const ok = want instanceof RegExp ? want.test(got) : got === want;
    if (!ok) throw new Error(`${what}: expected the clipboard to hold ${want}, got ${JSON.stringify(got).slice(0, 200)}`);
    console.log(`PASS ${what}`);
  };

  const context = await browser.newContext({ permissions: ["clipboard-read", "clipboard-write"], viewport: { width: 1280, height: 720 } });
  const page = await open(context);
  const { expectText } = checks(page);
  const term = page.locator("[data-testid=term-host]");
  await page.getByRole("button", { name: "Start", exact: true }).click();
  await page.getByRole("tab", { name: "Terminal" }).click();
  await expectText(term, "marker omega", "the terminal shows the agent's output");

  // The agent's own copy reaches the clipboard.
  await expectClip(page, "from the agent ✓", "OSC 52 from the agent is on the clipboard");

  // A selection: a triple click on a line (the agent has not turned the mouse on).
  // (the mouse is used at the row's coordinates: xterm's overlay sits over the rows)
  const selectAlpha = async () => {
    const box = await term.getByText("marker alpha").first().boundingBox();
    await page.mouse.click(box.x + 5, box.y + box.height / 2, { clickCount: 3 });
  };
  await selectAlpha();
  await page.locator("[data-testid=term-copy]").click();
  await expectClip(page, "marker alpha", "Copy copies the selection");

  // It survives the window getting shorter and taller again.
  await page.setViewportSize({ width: 1280, height: 600 });
  await page.waitForTimeout(500);
  await page.locator("[data-testid=term-copy]").click();
  await expectClip(page, "marker alpha", "the selection survives a resize");
  await page.setViewportSize({ width: 1280, height: 720 });
  await page.waitForTimeout(500); // the terminal refits before its rows are measured again

  // The copy key copies the selection: Cmd+C on a Mac, Ctrl+Shift+C elsewhere.
  await page.evaluate(() => navigator.clipboard.writeText("before"));
  const mac = await page.evaluate(() => /mac/i.test(navigator.platform));
  await selectAlpha();
  await page.keyboard.press(mac ? "Meta+c" : "Control+Shift+c");
  await expectClip(page, "marker alpha", "the copy key copies the selection");

  // Without a selection Copy takes the visible screen: the last lines, not the scrolled-out ones.
  await term.click();
  await page.evaluate(() => navigator.clipboard.writeText("before"));
  await page.evaluate(() => document.getSelection()?.removeAllRanges());
  await page.locator("[data-testid=term-copy]").click();
  await expectClip(page, /marker omega$/, "Copy without a selection copies the visible screen");
  const screen = await clip(page);
  if (screen.includes("scroll line 1\n")) throw new Error("the visible screen holds scrolled-out lines");
  console.log("PASS the visible screen leaves the scrollback out");

  // Copy all takes the scrollback too.
  await page.locator("[data-testid=term-copy-more]").click();
  await page.locator("[data-testid=term-copy-all]").click();
  await expectClip(page, /^scroll line 1\n[\s\S]*scroll line 80\nmarker alpha\n+marker omega$/, "Copy all copies the whole scrollback");

  // A window whose clipboard refuses without a gesture: "Copied text ready", then a click copies.
  const refusing = await browser.newContext({ permissions: ["clipboard-read", "clipboard-write"] });
  await refusing.addInitScript(() => {
    const real = navigator.clipboard.writeText.bind(navigator.clipboard);
    navigator.clipboard.writeText = (t) => (window.__allow ? real(t) : Promise.reject(new DOMException("no gesture", "NotAllowedError")));
  });
  const second = await open(refusing);
  await second.getByRole("tab", { name: "Terminal" }).click();
  await checks(second).expectText(second.locator("[data-testid=term-copy-ready]"), "Copied text ready", "a refused clipboard shows the Copy-ready control");
  await second.evaluate(() => { window.__allow = true; });
  await second.locator("[data-testid=term-copy-ready-button]").click();
  await expectClip(second, "from the agent ✓", "the Copy-ready control puts the agent's text on the clipboard");
  if ((await second.locator("[data-testid=term-copy-ready]").count()) !== 0) throw new Error("the Copy-ready control stayed after it was used");
  console.log("PASS the Copy-ready control goes away once used");

  if (errors.length) throw new Error(`the page threw: ${errors.join("; ")}`);
  console.log("web terminal copy: PASS");
} catch (e) {
  console.error(`web terminal copy: FAIL: ${e.message.slice(0, 600)}`);
  process.exitCode = 1;
} finally {
  await browser?.close();
  await cleanup(tok, home);
}
