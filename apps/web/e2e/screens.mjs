// Browser test of the three screens people work on most, in light and dark, against a real daemon
// and the workspace's `fake-agent`: a chain with its diagram, the Details tab of a card in review,
// and the Threads tab of a card with a design thread. Each must draw and carry its landmarks.
// Usage: node e2e/screens.mjs <tok binary> <fake-agent binary>   (cargo xtask e2e-web runs it)
// needs: fake-agent
// SCREENSHOT_DIR, when set, receives `<SCREENSHOT_PREFIX or "screen">-<view>-<light|dark>.png`.

import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { cleanup, launch, shipOf } from "./helpers.mjs";

const [tokArg, agentArg] = process.argv.slice(2);
if (!tokArg || !agentArg) {
  console.error("usage: node e2e/screens.mjs <tok binary> <fake-agent binary>");
  process.exit(2);
}
const tokBin = resolve(tokArg);
const fakeAgent = resolve(agentArg);
const home = mkdtempSync(join(tmpdir(), "toknila-screens-"));
const run = (cmd, args) => execFileSync(cmd, args, { encoding: "utf8" });
const tok = (...args) => execFileSync(tokBin, args, { env: { ...process.env, TOKNILA_HOME: home }, encoding: "utf8" });
const git = (dir, ...args) =>
  run("git", ["-C", dir, "-c", "user.name=t", "-c", "user.email=t@example.com", "-c", "commit.gpgsign=false", ...args]);
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
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
  writeFileSync(
    join(home, "profiles", "designer.toml"),
    `command = ['${fakeAgent}', ${steps(
      "print", "ready",
      ...post({ event: "open_thread", kind: "design", title: "Login screen" }),
      ...post({ event: "thread_version", thread: "1", format: "mermaid", body: "graph TD; Form --> Home" }),
      "hold",
    )}]\n`,
  );
  writeFileSync(join(home, "profiles", "plain.toml"), `command = ['${fakeAgent}', ${steps("print", "working", "hold")}]\n`);
  tok("project", "new", "DS", "--repo", repo, "--base", "develop", "--profile", "plain");
  tok("project", "set", "DS", "--parallel", "6");
  tok("chain", "new", "work", "--project", "DS", "--no-autostart");
  tok("ticket", "new", "work", "Login screen", "--profile", "designer", "--design-first", "--accept", "The form validates");
  tok("ticket", "new", "work", "Settings page", "--accept", "Every setting saves", "--accept", "Both themes");
  tok("ticket", "new", "work", "Help page with a long title that wraps onto a second line");
  tok("ticket", "new", "work", "Release notes", "--after", "DS-2", "--after", "DS-3");
  tok("ticket", "new", "work", "Polish the whole thing", "--after", "DS-4");
  tok("start", "DS-1");
  tok("start", "DS-2");
  tok("start", "DS-3");
  await until("a design waits", () => show("DS-1").design_waiting === true);
  await until("DS-3 running", () => show("DS-3").ticket.status === "running");
  const ship = shipOf(tok, "DS-3");
  writeFileSync(join(ship, "help.txt"), "help\n");
  git(ship, "add", "help.txt");
  git(ship, "commit", "--quiet", "-m", "help");
  tok(
    "sim", "DS-3", "finish",
    "**What changed:** the help page\n**Checks:**\n- PASS cargo xtask verify: test result: ok. 120 passed; 0 failed\n- NOT_RUN e2e: no browser here\n**Left:** nothing",
  );
  await until("DS-3 in review", () => show("DS-3").ticket.status === "review");
  const { url } = JSON.parse(tok("open", "--json"));

  browser = await launch();
  for (const scheme of ["light", "dark"]) {
    const ctx = await browser.newContext({ viewport: { width: 1440, height: 900 }, colorScheme: scheme });
    const page = await ctx.newPage();
    const errors = [];
    page.on("pageerror", (e) => errors.push(e.message));
    const at = (path) => page.goto(url.replace("/?", `/projects/DS/chains/work${path}?`));
    const shot = async (view) => {
      if (!process.env.SCREENSHOT_DIR) return;
      mkdirSync(process.env.SCREENSHOT_DIR, { recursive: true });
      const prefix = process.env.SCREENSHOT_PREFIX ?? "screen";
      await page.screenshot({ path: join(process.env.SCREENSHOT_DIR, `${prefix}-${view}-${scheme}.png`) });
    };

    await at("");
    await page.locator("a.node[data-key=DS-3]").waitFor({ timeout: 10_000 });
    await sleep(500);
    await shot("chain");
    console.log(`PASS the chain draws its cards (${scheme})`);

    await at("/cards/DS-3");
    await page.locator("[data-testid=status-sentence]").waitFor({ timeout: 10_000 });
    await sleep(300);
    await shot("details");
    console.log(`PASS the Details tab of a card in review (${scheme})`);

    await at("/cards/DS-1");
    await page.getByRole("tab", { name: "Threads" }).click();
    await page.locator("[data-testid=mermaid] svg").waitFor({ timeout: 20_000 });
    await shot("threads");
    console.log(`PASS the Threads tab of a design thread (${scheme})`);

    if (errors.length) throw new Error(`the page threw: ${errors.join("; ")}`);
    await ctx.close();
  }
  console.log("web screens: PASS");
} catch (e) {
  console.error(`web screens: FAIL: ${e.message.slice(0, 600)}`);
  process.exitCode = 1;
} finally {
  await browser?.close();
  await cleanup(tok, home);
}
