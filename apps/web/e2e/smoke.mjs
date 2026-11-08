// Browser smoke test of the IDE-layout page against a real daemon.
// Usage: node e2e/smoke.mjs <path to tok binary>   (cargo xtask e2e-web runs it)
//
// It starts a daemon in a private TOKNILA_HOME, imports examples/demo.toml, opens the page and
// checks: the sidebar leads to a chain, the canvas draws its cards and milestone, a card opens
// its Details and Events tabs, a change made from the CLI appears live, the card and inbox deep
// links and the title link navigate, and the page reconnects by itself after the daemon restarts.

import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { checks, cleanup, launch } from "./helpers.mjs";

const tokBin = process.argv[2];
if (!tokBin) {
  console.error("usage: node e2e/smoke.mjs <tok binary>");
  process.exit(2);
}
const home = mkdtempSync(join(tmpdir(), "toknila-web-"));
const demo = resolve(import.meta.dirname, "../../../examples/demo.toml");
// Screenshots of the main screens, kept as run artifacts (never committed): SHOTS_DIR, default
// apps/web/e2e-artifacts/<SHOTS_LABEL>; light and dark, for the chain, the card and the threads.
const shotsDir = process.env.SHOTS_DIR ?? resolve(import.meta.dirname, "../e2e-artifacts", process.env.SHOTS_LABEL ?? "run");
mkdirSync(shotsDir, { recursive: true });
const tok = (...args) =>
  execFileSync(tokBin, args, { env: { ...process.env, TOKNILA_HOME: home }, encoding: "utf8" });

let browser;
let failed = false;
try {
  tok("project", "new", "DEMO");
  tok("chain", "import", demo, "--project", "DEMO");
  const { url } = JSON.parse(tok("open", "--json"));
  console.log(`page: ${url.replace(/token=.*/, "token=…")}`);
  const at = (path) => url.replace("/?", `${path}?`);

  browser = await launch();
  const page = await browser.newPage();
  page.on("pageerror", (e) => {
    console.error(`page error: ${e.message}`);
    failed = true;
  });
  await page.goto(url);
  const status = (key) => page.locator(`a[data-key="${key}"] [data-testid=status]`);
  const shot = async (name) => {
    for (const scheme of ["light", "dark"]) {
      await page.emulateMedia({ colorScheme: scheme });
      await page.screenshot({ path: join(shotsDir, `${name}-${scheme}.png`) });
    }
    await page.emulateMedia({ colorScheme: "light" });
    console.log(`PASS screenshot ${name}`);
  };
  const { expectText, expectPath, expectLink } = checks(page);
  const openChain = async () => {
    await page.locator("[data-testid=chains]").getByRole("link", { name: "demo" }).click();
    await expectPath("/projects/DEMO/chains/demo", "chain route");
  };

  // Sidebar: the project, then its chain; the canvas draws the chain.
  await expectLink("live", "stream connected");
  await expectText(page.locator("[data-testid=projects]"), "DEMO", "project listed");
  await openChain();
  await expectText(status("DEMO-1"), "running", "DEMO-1 autostarted");
  await expectText(status("DEMO-2"), "todo · backlog", "DEMO-2 waits for DEMO-1");
  const keyWraps = await page.locator(".node .key").evaluateAll((els) => els.filter((e) => e.getClientRects().length > 1 || e.scrollWidth > e.clientWidth).length);
  if (keyWraps) throw new Error("a card key wraps or is clipped");
  console.log("PASS card keys do not wrap");
  await expectText(page.locator('[data-version="R1"]'), "planned", "milestone R1 is a node");
  await expectText(page.locator("[data-testid=running]"), "DEMO-1", "running agents list");
  const nodes = await page.locator(".canvas .node").count();
  if (nodes !== 7) throw new Error(`canvas: expected 7 nodes, got ${nodes}`);
  console.log("PASS canvas: 4 cards, 1 milestone, Start and Finish");
  await shot("chain");

  // Agent slots: one of the two slots is taken; a chain without autostart whose three cards are
  // started takes the second slot, so its second and third cards queue and wear "queued #1" and "#2" chips.
  await expectText(page.locator("[data-testid=slots]"), "1/2 agents", "top bar shows the slots");
  const extra = join(home, "extra.toml");
  writeFileSync(
    extra,
    '[chain]\nname = "extra"\ngoal = "slots"\nautostart = false\n' +
      [1, 2, 3].map((i) => `\n[[ticket]]\nid = "x${i}"\ntitle = "Extra ${i}"\nprofile = "sim"\n`).join(""),
  );
  tok("chain", "import", extra, "--project", "DEMO");
  for (const key of ["DEMO-5", "DEMO-6", "DEMO-7"]) tok("start", key);
  await page.locator("[data-testid=chains]").getByRole("link", { name: "extra" }).click();
  await expectPath("/projects/DEMO/chains/extra", "extra chain route");
  await expectText(page.locator("[data-testid=slots]"), "2/2 agents", "both slots are taken");
  await expectText(page.locator('a[data-key="DEMO-6"] [data-testid=queued]'), "queued #1", "the second card is first in line");
  await expectText(page.locator('a[data-key="DEMO-7"] [data-testid=queued]'), "queued #2", "the third card is next");
  if ((await page.locator('a[data-key="DEMO-5"] [data-testid=queued]').count()) !== 0) {
    throw new Error("a running card must not wear the queued chip");
  }
  await shot("queued");
  await openChain();

  // Open a card: the Details tab shows the ticket.
  await page.locator('a[data-key="DEMO-1"]').click();
  await expectPath("/projects/DEMO/chains/demo/cards/DEMO-1", "card route");
  await expectText(page.locator("[data-testid=details]"), "First step.", "Details tab shows the prompt");
  await expectText(page.locator("[data-testid=details]"), "write the test", "Details tab shows subtasks");
  await shot("card-detail");

  // A change made from the CLI shows live, in the canvas, the inbox and the Details tab.
  tok("sim", "DEMO-1", "ask", "which way?");
  await expectText(status("DEMO-1"), "in progress · waiting", "a waiting card is in progress, not blocked");
  await expectText(page.locator("[data-testid=status-sentence]"), "Waiting for your answer", "Details lead with the status sentence");
  await expectText(page.locator("[data-testid=details]"), "which way?", "the question is shown");
  await expectText(page.locator("[data-testid=chain-status]"), "needs human", "chain needs a human");
  await expectText(page.locator("[data-testid=inbox-count]"), "1", "inbox count");

  // Events tab: the card's journal, then a new line arrives from the stream.
  await page.getByRole("tab", { name: "Events" }).click();
  await expectText(page.locator("[data-testid=events]"), "Moved from", "Events tab lists the journal in words");
  const before = await page.locator("[data-testid=events] li").count();
  tok("comment", "DEMO-1", "live note");
  await page.waitForFunction(
    (n) => document.querySelectorAll("[data-testid=events] li").length > n,
    before,
    { timeout: 10_000 },
  );
  console.log("PASS Events tab: a new event arrived live");
  await page.getByRole("tab", { name: "Terminal" }).click();
  await expectText(page.locator("[data-testid=placeholder]"), "no runner yet", "Terminal tab is a placeholder");

  // The comment answered the question, so the card runs again; ask once more for the inbox.
  await page.getByRole("tab", { name: "Details" }).click();
  await expectText(status("DEMO-1"), "running", "a comment answers the question");
  tok("sim", "DEMO-1", "ask", "and now?");
  await expectText(status("DEMO-1"), "waiting", "asked again");

  // Threads tab: an agent posts a Mermaid and an HTML version in a design thread; the person asks
  // for changes, the agent answers with a new version, the person approves.
  const art = (name, body) => {
    const f = join(home, name);
    writeFileSync(f, body);
    return f;
  };
  const flow = art("flow.mmd", "graph TD; A[List] --> B[Detail]");
  const states = art("states.html", "<h1 id=s>empty / loading / error / content</h1><script>document.title='ran'</script>");
  tok("sim", "DEMO-1", "thread", "open", "design", "Profile list states");
  tok("sim", "DEMO-1", "thread", "post", "1", "--artifact", flow, "--format", "mermaid");
  tok("sim", "DEMO-1", "thread", "post", "1", "--artifact", states, "--format", "html", "--message", "all four states");
  await page.getByRole("tab", { name: "Threads" }).click();
  const threads = page.locator("[data-testid=threads]");
  await expectText(threads.locator("[data-testid=thread-state]"), "open", "design thread is open");
  await expectText(page.locator("[data-testid=phase]"), "design", "phase badge reads design");
  await expectText(page.locator("[data-testid=hand-in-notice]"), "blocks hand-in", "open thread blocks hand-in");
  const mockup = page.locator("[data-testid=mockup]");
  await mockup.waitFor({ timeout: 10_000 });
  if ((await mockup.getAttribute("sandbox")) !== "") throw new Error("mockup iframe must have an empty sandbox");
  await page.frameLocator("[data-testid=mockup]").locator("#s").waitFor({ timeout: 10_000 });
  if ((await page.frameLocator("[data-testid=mockup]").locator("html").evaluate(() => document.title)) === "ran") {
    throw new Error("a script ran inside the sandboxed mockup");
  }
  console.log("PASS HTML version renders in a sandboxed iframe, scripts do not run");
  await page.getByRole("button", { name: "v1" }).click();
  await page.locator("[data-testid=mermaid] svg").waitFor({ timeout: 20_000 });
  console.log("PASS Mermaid version renders as an svg diagram");
  await shot("threads");
  await page.getByLabel("Message in thread 1").fill("show the error state larger");
  await page.getByRole("button", { name: "Request changes" }).click();
  await expectText(threads.locator("[data-testid=thread-state]"), "changes requested", "changes requested");
  await expectText(threads.locator("[data-testid=messages]"), "show the error state larger", "comment is in the thread");
  tok("sim", "DEMO-1", "thread", "post", "1", "--artifact", states, "--format", "html", "--message", "error state is larger");
  await expectText(page.locator("[data-testid=versions]"), "v3", "a new version arrived live");
  await expectText(threads.locator("[data-testid=thread-state]"), "open", "a new version answers the request");
  await page.getByRole("button", { name: "Approve" }).click();
  await expectText(threads.locator("[data-testid=thread-state]"), "approved", "thread approved");
  await expectText(page.locator("[data-testid=phase]"), "implement", "phase badge reads implement");
  if (!(await page.getByRole("button", { name: "Approve" }).isDisabled())) throw new Error("Approve must be disabled after approval");
  if (await page.locator("[data-testid=hand-in-notice]").count()) throw new Error("the hand-in notice must go after approval");
  console.log("PASS actions are disabled after approval, notice is gone");
  await page.getByRole("tab", { name: "Details" }).click();

  // Navigation lives in the URL: deep links open the card and the inbox, the title leads back.
  await page.goto(at("/projects/DEMO/chains/demo/cards/DEMO-2"));
  await expectText(page.locator("[data-testid=details]"), "DEMO-2", "card deep link loads");
  await page.goto(at("/inbox"));
  await expectLink("live", "inbox deep link loads");
  await page.locator('[data-testid=inbox] li[data-key="DEMO-1"]').waitFor({ timeout: 10_000 });
  const inbox = await page.locator("[data-testid=inbox] li").count();
  if (inbox !== 1) throw new Error(`inbox: expected 1 ticket, got ${inbox}`);
  console.log("PASS inbox route: 1 ticket");
  await page.getByRole("link", { name: "Toknila" }).click();
  await expectPath("/", "title link");

  tok("daemon", "stop");
  await expectLink("offline", "daemon stop seen");
  await expectText(page.locator("[data-testid=link]"), "Offline", "the sidebar says Offline");
  tok("daemon", "start");
  tok("comment", "DEMO-2", "left");
  await expectLink("live", "reconnected after restart");
  await openChain();
  await expectText(status("DEMO-1"), "waiting", "state is still there after the restart");
  if (failed) throw new Error("the page threw errors");
  console.log("web smoke: PASS");
} catch (e) {
  console.error(`web smoke: FAIL: ${e.message}`);
  process.exitCode = 1;
} finally {
  await browser?.close();
  await cleanup(tok, home);
}
