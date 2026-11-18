// Browser test of copying from the card's terminal against a real daemon.
// Usage: node e2e/terminal-copy.mjs <tok binary> <fake-agent binary>   (cargo xtask e2e-web runs it)
// needs: fake-agent
//
// The fake agent prints lines and asks the terminal to copy "from the agent" (OSC 52). The
// clipboard then holds that text; the Copy control copies a selection (a triple click on a line),
// else the visible screen; "Copy all" in its menu copies the scrollback, every line once and in
// order, after the window got shorter and taller again (on Windows ConPTY repaints its screen at
// each resize; see src/terminal/conpty.ts); the copy key copies the
// selection; and a selection survives the window's height changing. In a second window whose
// clipboard refuses writes, the agent's copy shows "Copied text ready" and its Copy button puts
// the text on the clipboard once clicked.

import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { checks, cleanup, launch, session } from "./helpers.mjs";

const [tokArg, agentArg] = process.argv.slice(2);
if (!tokArg || !agentArg) {
  console.error("usage: node e2e/terminal-copy.mjs <tok binary> <fake-agent binary>");
  process.exit(2);
}
const tokBin = resolve(tokArg);
const fakeAgent = resolve(agentArg);
const { home, marker: fakeMarker } = session();
const run = (cmd, args, opts = {}) => execFileSync(cmd, args, { encoding: "utf8", ...opts });
const tok = (...args) => run(tokBin, args, { env: { ...process.env, TOKNILA_HOME: home } });
const git = (dir, ...args) =>
  run("git", ["-C", dir, "-c", "user.name=t", "-c", "user.email=t@example.com", "-c", "commit.gpgsign=false", ...args]);

// Where the agent's lines went (TOKNILA-162): for each layer, its line count, first and last lines
// and which of the numbered lines it lacks. Raw terminal output is read without its escape
// sequences; a line is counted once however often a repaint drew it.
const LINES = 80;
const plain = (s) =>
  s
    .replace(/\x1b\[[0-?]*[ -/]*[@-~]/g, "")
    .replace(/\x1b\][^\x07\x1b]*(?:\x07|\x1b\\)/g, "")
    .replace(/\x1b[@-Z\\-_]/g, "");
const ranges = (ns) => {
  const out = [];
  for (let i = 0; i < ns.length; ) {
    let j = i;
    while (j + 1 < ns.length && ns[j + 1] === ns[j] + 1) j++;
    out.push(i === j ? `${ns[i]}` : `${ns[i]}-${ns[j]}`);
    i = j + 1;
  }
  return out.join(", ");
};
const layer = (name, text) => {
  const flat = plain(text);
  const lines = flat.split(/\r?\n/).map((l) => l.trimEnd()).filter((l) => l !== "");
  const seen = new Set([...flat.matchAll(/scroll line (\d+)(?!\d)/g)].map((m) => Number(m[1])));
  const missing = [];
  for (let i = 1; i <= LINES; i++) if (!seen.has(i)) missing.push(i);
  return `${name}: ${lines.length} lines, first ${JSON.stringify(lines[0] ?? "")}, last ${JSON.stringify(lines.at(-1) ?? "")}, numbered lines ${LINES - missing.length}/${LINES}, missing [${ranges(missing)}]`;
};
// The newest run log under the data folder, as the daemon wrote it.
const newestLog = (dir) => {
  let best = null;
  const walk = (d) => {
    for (const e of readdirSync(d, { withFileTypes: true })) {
      const p = join(d, e.name);
      if (e.isDirectory()) walk(p);
      else if (e.name.endsWith(".log") && (!best || statSync(p).mtimeMs > best.at)) best = { p, at: statSync(p).mtimeMs };
    }
  };
  walk(dir);
  return best?.p;
};

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
  writeFileSync(join(home, "profiles", "fake.toml"), `command = [${[fakeAgent, "sleep", fakeMarker, ...steps].map((s) => `'${s}'`).join(", ")}]\n`);
  tok("project", "new", "WEB", "--repo", repo, "--base", "develop", "--profile", "fake");
  tok("chain", "new", "work", "--project", "WEB", "--no-autostart");
  tok("ticket", "new", "work", "Copy task", "--prompt", "Say hello.");
  const { url } = JSON.parse(tok("open", "--json"));
  const cardUrl = url.replace("/?", "/projects/WEB/chains/work/cards/WEB-1?");

  browser = await launch();
  const errors = [];
  // The grids the page's terminal told the daemon (its fit to the panel), the last one last.
  const pageGrids = [];
  const open = async (context) => {
    const page = await context.newPage();
    page.on("pageerror", (e) => errors.push(e.message));
    page.on("request", (r) => {
      if (r.method() === "POST" && r.url().includes("/runner/size")) pageGrids.push(JSON.parse(r.postData() ?? "null"));
    });
    await page.goto(cardUrl);
    return page;
  };
  // Each layer between the agent and the clipboard: (a) the run log the daemon wrote, (b) the
  // scrollback route's answer (`tok logs`), (c) the page terminal's buffer as Copy all handed it
  // over; then the grids the page asked for, and with `bytes` what the terminal emitted after the
  // agent's last line (on Windows ConPTY's repaints, one per resize).
  const whereTheLinesWent = (copied, bytes) => {
    const out = [];
    const log = newestLog(join(home, "runs"));
    out.push(log ? layer("(a) run log", readFileSync(log, "utf8")) : "(a) run log: none under the data folder");
    const served = execFileSync(tokBin, ["logs", "WEB-1"], { env: { ...process.env, TOKNILA_HOME: home } }).toString("utf8");
    out.push(layer("(b) scrollback route", served));
    out.push(layer("(c) page terminal (Copy all)", copied));
    out.push(`grids the page sent: ${pageGrids.map((g) => `${g.cols}x${g.rows}`).join(" ")}`);
    const end = served.indexOf("marker omega");
    if (bytes && end >= 0) out.push(`after the output: ${JSON.stringify(served.slice(end + "marker omega".length)).slice(0, 4000)}`);
    return out;
  };
  const clip = (page) => page.evaluate(() => navigator.clipboard.readText());
  const expectClip = async (page, want, what) => {
    let got;
    for (const until = Date.now() + 8000; Date.now() < until; await page.waitForTimeout(100)) {
      got = await clip(page);
      if (got === want || (want instanceof RegExp && want.test(got))) break;
    }
    const ok = want instanceof RegExp ? want.test(got) : got === want;
    if (!ok) throw new Error(`${what}: expected the clipboard to hold ${want}, got ${JSON.stringify(got).slice(0, 200)} ... ${JSON.stringify(got.slice(-200))}`);
    console.log(`PASS ${what}`);
  };

  // A sentinel no step expects: a copy that has not happened yet cannot pass on the previous text.
  const STALE = "stale-clipboard-sentinel";
  const clearClip = (page) => page.evaluate((s) => navigator.clipboard.writeText(s), STALE);
  const clipNow = async (page, want) => {
    for (const until = Date.now() + 8000; Date.now() < until; await page.waitForTimeout(100)) {
      const got = await clip(page);
      if (got === want || (want instanceof RegExp && want.test(got))) return true;
    }
    return false;
  };
  // Waits for the row holding `marker` to be drawn, then returns its box (null never escapes).
  const rowBox = async (term, marker) => {
    const row = term.getByText(marker).first();
    await row.waitFor({ state: "visible", timeout: 10_000 });
    for (const until = Date.now() + 10_000; Date.now() < until; await term.page().waitForTimeout(100)) {
      const box = await row.boundingBox();
      if (box) return box;
    }
    throw new Error(`the row "${marker}" has no box`);
  };
  // The terminal's selection is not in the DOM (the page has no handle on xterm), so what is
  // asserted is its effect: select the marker's row, clear the clipboard, fire `copy`, and wait for
  // the clipboard to hold exactly the marker. A selection or a copy that lagged is made again.
  const selectAndCopy = async (page, term, marker, copy, what) => {
    let got;
    for (const until = Date.now() + 20_000; Date.now() < until; ) {
      const box = await rowBox(term, marker);
      await page.mouse.click(box.x + 5, box.y + box.height / 2, { clickCount: 3 });
      await clearClip(page);
      await copy();
      if (await clipNow(page, marker)) {
        console.log(`PASS ${what}`);
        return;
      }
      got = await clip(page);
    }
    throw new Error(`${what}: expected the clipboard to hold ${marker}, got ${JSON.stringify(got).slice(0, 200)}`);
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
  const copyButton = () => page.locator("[data-testid=term-copy]").click();
  await selectAndCopy(page, term, "marker alpha", copyButton, "Copy copies the selection");

  // It survives the window getting shorter and taller again (no new selection is made).
  await page.setViewportSize({ width: 1280, height: 600 });
  await page.waitForFunction(() => window.innerHeight === 600);
  const hostHeight = await term.evaluate((el) => el.getBoundingClientRect().height);
  await page.waitForFunction(
    ([sel, h]) => document.querySelector(sel).getBoundingClientRect().height < h,
    ["[data-testid=term-host]", hostHeight + 1],
  );
  let survived = false;
  for (const until = Date.now() + 10_000; !survived && Date.now() < until; ) {
    await clearClip(page);
    await copyButton();
    survived = await clipNow(page, "marker alpha");
  }
  if (!survived) throw new Error(`the selection survives a resize: expected the clipboard to hold marker alpha, got ${JSON.stringify(await clip(page))}`);
  console.log("PASS the selection survives a resize");
  await page.setViewportSize({ width: 1280, height: 720 });
  await page.waitForFunction(() => window.innerHeight === 720);
  await page.waitForTimeout(500); // the terminal refits before its rows are measured again

  // The copy key copies the selection: Cmd+C on a Mac, Ctrl+Shift+C elsewhere.
  const mac = await page.evaluate(() => /mac/i.test(navigator.platform));
  await selectAndCopy(page, term, "marker alpha", () => page.keyboard.press(mac ? "Meta+c" : "Control+Shift+c"), "the copy key copies the selection");

  // Without a selection Copy takes the visible screen: the last lines, not the scrolled-out ones.
  // A click drops the selection; if it lagged, the copy takes the old selection and is made again.
  for (const until = Date.now() + 20_000; Date.now() < until; ) {
    await term.click();
    await clearClip(page);
    await page.locator("[data-testid=term-copy]").click();
    if (await clipNow(page, /marker omega$/)) break;
  }
  await expectClip(page, /marker omega$/, "Copy without a selection copies the visible screen");
  const screen = await clip(page);
  if (screen.includes("scroll line 1\n")) throw new Error("the visible screen holds scrolled-out lines");
  console.log("PASS the visible screen leaves the scrollback out");

  // Copy all takes the scrollback too.
  await clearClip(page);
  await page.locator("[data-testid=term-copy-more]").click();
  await page.locator("[data-testid=term-copy-all]").click();
  // Every line, in order, ending with the markers; once each on macOS and Linux. The pattern this
  // replaced checked only the first and the last lines, and passed while the page lost line 70 in
  // every Windows run (TOKNILA-162, CI run 44308344208). On Windows ConPTY repaints its screen at
  // each resize, and the page's terminal changes size more often than it tells the daemon (it
  // sends the last size of a burst), so a repaint can draw lines the page already has: with the
  // page set for ConPTY (src/terminal/conpty.ts) all 80 lines were there in six of six runs of CI
  // run 44327828545, three of them twice after this spec's resizes. A repeat is allowed there, at
  // most one screen of the page's grid. Windows' clipboard ends lines with CRLF; the blank row
  // between the markers is the line of the agent's copy request.
  const want = [...Array.from({ length: LINES }, (_, i) => `scroll line ${i + 1}`), "marker alpha", "marker omega"];
  let copied = STALE;
  for (const until = Date.now() + 8000; copied === STALE && Date.now() < until; await page.waitForTimeout(100)) copied = await clip(page);
  const got = copied.split(/\r?\n/).filter((l) => l !== "");
  const firstSeen = got.filter((l, i) => got.indexOf(l) === i);
  const repeats = got.length - firstSeen.length;
  const mayRepeat = process.platform === "win32" ? (pageGrids.at(-1)?.rows ?? 0) : 0;
  const inOrder = firstSeen.length === want.length && firstSeen.every((l, i) => l === want[i]);
  const lost = !inOrder || repeats > mayRepeat || got.at(-1) !== "marker omega";
  console.log(`INFO Copy all: ${got.length} lines, ${repeats} repeated (allowed ${mayRepeat})`);
  // Every run says what each layer held, so a passing run is a number too.
  const layers = whereTheLinesWent(copied, lost);
  for (const line of layers) console.log(`INFO ${line}`);
  if (lost) {
    const missing = want.filter((l) => !got.includes(l));
    const differs = firstSeen.findIndex((l, i) => l !== want[i]);
    const at = differs < 0 ? firstSeen.length : differs;
    throw new Error(
      `Copy all copies the whole scrollback: ${got.length} lines for ${want.length}, ${repeats} repeated (allowed ${mayRepeat}), missing ${JSON.stringify(missing)}, first new line out of order at ${at + 1}: ${JSON.stringify(firstSeen[at] ?? "(end)")}, last line ${JSON.stringify(got.at(-1))}\n  where the lines went:\n  ${layers.join("\n  ")}`,
    );
  }
  console.log(`PASS Copy all copies the whole scrollback, every line in order${mayRepeat ? `, at most ${mayRepeat} repeated` : ", each once"}`);

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
  console.error(`web terminal copy: FAIL: ${e.message.slice(0, 3000)}`);
  process.exitCode = 1;
} finally {
  await browser?.close();
  await cleanup(tok, home);
}
