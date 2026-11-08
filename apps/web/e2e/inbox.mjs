// Browser test of the inbox against a real daemon.
// Usage: node e2e/inbox.mjs <path to tok binary>   (cargo xtask e2e-web runs it)
//
// A wildcard proposal is approved, a blocked card is unblocked and an agent question is answered,
// all from the inbox; then the three leftover actions (move into a new card, close, leave) are
// taken on three cards that a later card was imported done behind.

import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { checks, cleanup, launch } from "./helpers.mjs";

const tokBin = process.argv[2];
if (!tokBin) {
  console.error("usage: node e2e/inbox.mjs <tok binary>");
  process.exit(2);
}
const home = mkdtempSync(join(tmpdir(), "toknila-inbox-"));
const tok = (...args) =>
  execFileSync(tokBin, args, { env: { ...process.env, TOKNILA_HOME: home }, encoding: "utf8" });

const chainFile = (name, autostart, cards) =>
  `[chain]\nname = "${name}"\nautostart = ${autostart}\n` +
  cards
    .map((c) =>
      `\n[[ticket]]\nid = "${c.id}"\ntitle = "${c.id}"\nprofile = "sim"\n` +
      (c.subtasks ? `subtasks = ["T1 one", "T2 two"]\ndone_subtasks = [1]\n` : "") +
      (c.after ? `after = [${c.after.map((a) => `"${a}"`).join(", ")}]\n` : "") +
      (c.done ? "done = true\n" : ""),
    )
    .join("");

let browser;
let failed = false;
try {
  tok("project", "new", "INB");
  const live = join(home, "live.toml");
  writeFileSync(live, chainFile("live", true, [{ id: "asks" }, { id: "blocks" }]));
  tok("chain", "import", live, "--project", "INB");
  tok("sim", "INB-1", "ask", "which database?");
  tok("block", "INB-2", "waiting for the key");
  tok("propose", "live", "Found work", "--prompt", "Nobody planned this.");
  const left = join(home, "left.toml");
  writeFileSync(
    left,
    chainFile("imported", false, [
      { id: "L1", subtasks: true },
      { id: "L2", subtasks: true },
      { id: "L3", subtasks: true },
      { id: "D", done: true, after: ["L1", "L2", "L3"] },
    ]),
  );
  tok("chain", "import", left, "--project", "INB");

  const { url } = JSON.parse(tok("open", "--json"));
  browser = await launch();
  const page = await browser.newPage();
  page.on("pageerror", (e) => {
    console.error(`page error: ${e.message}`);
    failed = true;
  });
  await page.goto(url);
  const { expectText, expectLink } = checks(page);
  await expectLink("live", "stream connected");
  await expectText(page.locator("[data-testid=inbox-count]"), "6", "badge counts proposal, question, blocked and 3 leftovers");
  await page.locator("[data-testid=inbox-link]").click();
  const inbox = page.locator("[data-testid=inbox]");
  const row = (kind, key) => inbox.locator(`li[data-kind=${kind}][data-key="${key}"]`);
  for (const kind of ["proposal", "question", "blocked", "leftovers"]) {
    await inbox.locator(`[data-testid=inbox-${kind}]`).waitFor({ timeout: 10_000 });
  }
  console.log("PASS inbox is grouped by kind");
  await expectText(row("question", "INB-1"), "which database?", "the question is shown");
  await expectText(row("blocked", "INB-2"), "waiting for the key", "the blocking reason is shown");

  // A wildcard proposal is approved in the inbox.
  await row("proposal", "INB-3").getByRole("button", { name: "Approve" }).click();
  await row("proposal", "INB-3").waitFor({ state: "detached", timeout: 10_000 });
  if (!tok("show", "INB-3").includes(", approved")) throw new Error("the card is not approved");
  console.log("PASS a wildcard proposal is approved from the inbox");

  // A blocked card is unblocked.
  await row("blocked", "INB-2").getByRole("button", { name: "Unblock" }).click();
  await row("blocked", "INB-2").waitFor({ state: "detached", timeout: 10_000 });
  if (!tok("show", "INB-2").includes("running")) throw new Error("the unblocked card is not running");
  console.log("PASS a blocked card is unblocked from the inbox");

  // The question is answered: the reply is an instruction, delivered to the simulated agent.
  await page.getByLabel("Reply to INB-1").fill("use SQLite");
  await row("question", "INB-1").getByRole("button", { name: "Reply" }).click();
  await row("question", "INB-1").waitFor({ state: "detached", timeout: 10_000 });
  const shown = tok("show", "INB-1");
  if (!shown.includes("running") || !shown.includes("use SQLite")) {
    throw new Error(`the answer did not reach the agent:\n${shown}`);
  }
  console.log("PASS an agent question is answered and delivered");

  // The three leftover actions.
  await row("leftovers", "INB-4").getByRole("button", { name: "Move to a new card" }).click();
  await row("leftovers", "INB-4").waitFor({ state: "detached", timeout: 10_000 });
  if (!tok("ls").includes("Leftovers from L1")) throw new Error("no card for the moved leftovers");
  await row("leftovers", "INB-5").getByRole("button", { name: "Close them" }).click();
  await row("leftovers", "INB-5").waitFor({ state: "detached", timeout: 10_000 });
  await row("leftovers", "INB-6").getByRole("button", { name: "Leave as is" }).click();
  await row("leftovers", "INB-6").waitFor({ state: "detached", timeout: 10_000 });
  console.log("PASS leftovers: moved, closed and left");
  await expectText(page.locator("[data-testid=inbox-count]"), "0", "the inbox is empty");
  await expectText(inbox, "Nothing needs a person.", "the empty inbox says so");

  // Clicking an item opens its card.
  tok("sim", "INB-1", "ask", "and the port?");
  await row("question", "INB-1").waitFor({ timeout: 10_000 });
  await row("question", "INB-1").locator("a.key").click();
  await page.waitForFunction(() => window.location.pathname.endsWith("/cards/INB-1"), null, { timeout: 10_000 });
  console.log("PASS an item opens its card");
  // Accept of a ship whose work is not committed: the card is blocked with ship_uncommitted and
  // the inbox says so in words. The ship is a real worktree of a temporary repository.
  const git = (cwd, ...args) =>
    execFileSync("git", ["-c", "user.name=t", "-c", "user.email=t@example.com", "-c", "commit.gpgsign=false", ...args], {
      cwd,
      encoding: "utf8",
    });
  const repo = join(home, "repo");
  mkdirSync(repo);
  git(repo, "init", "--quiet");
  git(repo, "symbolic-ref", "HEAD", "refs/heads/develop");
  writeFileSync(join(repo, "README.md"), "hi\n");
  git(repo, "add", "README.md");
  git(repo, "commit", "--quiet", "-m", "init");
  tok("project", "new", "GIT", "--repo", repo, "--base", "develop");
  const shipChain = join(home, "ship.toml");
  writeFileSync(shipChain, chainFile("shipped", true, [{ id: "work" }]));
  tok("chain", "import", shipChain, "--project", "GIT");
  const ship = JSON.parse(tok("show", "GIT-1", "--json")).ship.path;
  writeFileSync(join(ship, "done.txt"), "done\n");
  git(ship, "add", "done.txt");
  git(ship, "commit", "--quiet", "-m", "done");
  tok("sim", "GIT-1", "finish", "all done");
  writeFileSync(join(ship, "late.txt"), "forgotten\n");
  tok("accept", "GIT-1");
  await page.locator("[data-testid=inbox-link]").click();
  await row("blocked", "GIT-1").waitFor({ timeout: 10_000 });
  await expectText(row("blocked", "GIT-1"), "not committed", "the blocked item says the ship has uncommitted changes");
  await expectText(row("blocked", "GIT-1"), "late.txt", "the blocked item names the file");
  if (!tok("show", "GIT-1").includes("ship_uncommitted")) throw new Error("the card is not blocked with ship_uncommitted");
  console.log("PASS accept of an uncommitted ship is blocked as ship_uncommitted");
  if (failed) throw new Error("the page threw errors");
  console.log("inbox: PASS");
} catch (e) {
  console.error(`inbox: FAIL: ${e.message}`);
  process.exitCode = 1;
} finally {
  await browser?.close();
  await cleanup(tok, home);
}
