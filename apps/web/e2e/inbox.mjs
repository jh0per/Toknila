// Browser test of the inbox against a real daemon.
// Usage: node e2e/inbox.mjs <path to tok binary>   (cargo xtask e2e-web runs it)
//
// A wildcard proposal is approved, a blocked card is unblocked and an agent question is answered,
// all from the inbox; then the three leftover actions (move into a new card, close, leave) are
// taken on three cards that a later card was imported done behind.
// D2: a row leads with the card's title and says what is asked on a muted second line; its buttons
// keep their size and every row has Open card; a card in review has Accept and Return in the row
// (Accept merges it for real); the sidebar highlights no project on the inbox; a card's Details
// lead with a status sentence (the answer box inline when it asks) and, in review, the agent's
// report above Accept and Return, with the workspace folded under Technical details.

import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { checks, cleanup, launch, shipOf } from "./helpers.mjs";

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
  // A new project reviews with both (a reviewer agent first); this script plays the person.
  tok("review-mode", "human", "--project", "INB");
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

  // The card's title leads; the kind is a muted second line; every row can open its card.
  await expectText(row("question", "INB-1").locator(".row-head .t"), "asks", "a row leads with the card's title");
  await expectText(row("question", "INB-1").locator("[data-testid=inbox-kind]"), "waiting for your answer", "the kind is the second line");
  await expectText(row("proposal", "INB-3").locator(".row-head .t"), "Found work", "a proposal leads with its title");
  for (const [kind, key] of [["proposal", "INB-3"], ["question", "INB-1"], ["blocked", "INB-2"], ["leftovers", "INB-4"]]) {
    await row(kind, key).locator("[data-testid=inbox-open-card]").waitFor({ timeout: 10_000 });
  }
  console.log("PASS every kind of row has Open card");
  // Buttons are never stretched: Approve is narrower than a third of its row.
  const approve = await row("proposal", "INB-3").getByRole("button", { name: "Approve" }).boundingBox();
  const wide = await row("proposal", "INB-3").boundingBox();
  if (approve.width > wide.width / 3) throw new Error(`Approve is ${approve.width}px in a ${wide.width}px row`);
  console.log(`PASS Approve keeps its size (${Math.round(approve.width)}px of ${Math.round(wide.width)}px)`);
  const lit = await page.locator("[data-testid=projects] .item.sel").count();
  if (lit !== 0) throw new Error("the sidebar highlights a project on the inbox");
  console.log("PASS the sidebar highlights no project on the inbox");

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
  // Details lead with the status sentence; the waiting card's question is answered right there.
  const t = (id) => page.locator(`[data-testid=${id}]`);
  await expectText(t("status-sentence"), "Waiting for your answer", "Details lead with the status sentence");
  await expectText(t("status-block").locator("[data-testid=question]"), "and the port?", "the question is in the status block");
  await t("status-block").getByLabel("Reply to INB-1").fill("port 8080");
  await t("status-block").getByRole("button", { name: "Reply" }).click();
  await expectText(t("status-sentence"), "An agent is working on it", "after the answer the card runs");
  if (!tok("show", "INB-1").includes("port 8080")) throw new Error("the inline answer did not reach the agent");
  console.log("PASS the answer box inline in Details reaches the agent");
  const tabs = await page.locator("[role=tab]").allTextContents();
  if (tabs.some((x) => /pull request|^PR$/i.test(x))) throw new Error(`a locally merged project shows a pull request tab: ${tabs}`);
  console.log("PASS no Pull request tab for a project that merges locally");
  await expectText(page.locator("[data-testid=details] h4").first(), "Description", "the columns are Description, Acceptance criteria and Subtasks");
  const headings = await page.locator("[data-testid=details] .cols h4").allTextContents();
  if (headings.join("|") !== "Description|Acceptance criteria|Subtasks") throw new Error(`columns: ${headings}`);
  console.log("PASS Details have the three columns");
  if ((await t("actions").count()) !== 1) throw new Error("the action bar should be there once");
  console.log("PASS the action bar is there once");
  // Accept while the workspace has changes beyond the hand-in commit: refused at once, naming the
  // file; the card stays in Review, and once the file is committed the accept merges it. The ship
  // is a real worktree of a temporary repository.
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
  tok("review-mode", "human", "--project", "GIT");
  const shipChain = join(home, "ship.toml");
  writeFileSync(shipChain, chainFile("shipped", true, [{ id: "work" }]));
  tok("chain", "import", shipChain, "--project", "GIT");
  const ship = shipOf(tok, "GIT-1");
  writeFileSync(join(ship, "done.txt"), "done\n");
  git(ship, "add", "done.txt");
  git(ship, "commit", "--quiet", "-m", "done");
  tok("sim", "GIT-1", "finish", "all done");
  // The hand-in's check runs after `finished` answers; Accept is refused until it waits for a person.
  for (const until = Date.now() + 30_000; JSON.parse(tok("show", "GIT-1", "--json")).review.awaiting?.by === "check"; ) {
    if (Date.now() > until) throw new Error("GIT-1 stayed in its check turn");
    await new Promise((r) => setTimeout(r, 100));
  }
  writeFileSync(join(ship, "late.txt"), "forgotten\n");
  let refused = "";
  try {
    tok("accept", "GIT-1");
  } catch (e) {
    refused = `${e.stdout ?? ""}${e.stderr ?? ""}${e.message}`;
  }
  if (!refused.includes("late.txt")) throw new Error(`accept was not refused naming late.txt: ${refused}`);
  if (!tok("show", "GIT-1").includes("review")) throw new Error("the card did not stay in review");
  console.log("PASS accept with changes beyond the hand-in is refused and names the file");
  await page.locator("[data-testid=inbox-link]").click();
  await row("review", "GIT-1").waitFor({ timeout: 10_000 });
  git(ship, "add", "late.txt");
  git(ship, "commit", "--quiet", "-m", "late");
  tok("accept", "GIT-1");
  for (const until = Date.now() + 20_000; !tok("show", "GIT-1").includes("merged"); ) {
    if (Date.now() > until) throw new Error("the card did not merge after the file was committed");
    await new Promise((r) => setTimeout(r, 200));
  }
  console.log("PASS once the file is committed, the accept merges the card");
  // A card in review: the row has Accept and Return; Details show the report above them.
  const revChain = join(home, "reviewed.toml");
  writeFileSync(revChain, chainFile("reviewed", true, [{ id: "rev" }, { id: "back" }]));
  tok("chain", "import", revChain, "--project", "GIT");
  const rship = shipOf(tok, "GIT-2");
  writeFileSync(join(rship, "rev.txt"), "rev\n");
  git(rship, "add", "rev.txt");
  git(rship, "commit", "--quiet", "-m", "rev");
  tok("sim", "GIT-2", "finish", "wrote rev.txt as asked");
  await row("review", "GIT-2").waitFor({ timeout: 10_000 });
  await expectText(row("review", "GIT-2").locator("[data-testid=inbox-kind]"), "Ready for your review", "a review row says so");
  for (const id of ["review-accept", "review-return-open", "inbox-open-card"]) {
    await row("review", "GIT-2").locator(`[data-testid=${id}]`).waitFor({ timeout: 10_000 });
  }
  console.log("PASS a review row has Accept, Return and Open card");
  await row("review", "GIT-2").locator("[data-testid=inbox-open-card]").click();
  await page.waitForFunction(() => window.location.pathname.endsWith("/cards/GIT-2"), null, { timeout: 10_000 });
  await expectText(t("status-sentence"), "In review: read the agent's report below, then Accept or Return", "a card in review says what to do");
  await expectText(t("agent-report"), "wrote rev.txt as asked", "the agent's report is shown");
  const order = await page.evaluate(() => {
    const y = (sel) => document.querySelector(sel)?.getBoundingClientRect().y ?? -1;
    return [y("[data-testid=agent-report]"), y("[data-testid=review-block] [data-testid=actions]")];
  });
  if (!(order[0] > 0 && order[0] < order[1])) throw new Error(`the report should be above Accept and Return: ${order}`);
  console.log("PASS the report is above Accept and Return");
  if ((await t("actions").count()) !== 1) throw new Error("the action bar should be there once, under the report");
  console.log("PASS the action bar is there once, under the report");
  if (await t("technical-details").evaluate((d) => d.open)) throw new Error("Technical details should start folded");
  if (await t("ship-path").isVisible()) throw new Error("the workspace path should be folded away");
  await t("technical-details").locator("summary").click();
  await expectText(t("ship-path"), rship, "Technical details hold the workspace folder");
  await expectText(t("ship-branch"), "ship/", "and its branch");
  // Return from the row: it asks what to change, sends the card back to its agent, and the row goes.
  const bship = shipOf(tok, "GIT-3");
  writeFileSync(join(bship, "back.txt"), "back\n");
  git(bship, "add", "back.txt");
  git(bship, "commit", "--quiet", "-m", "back");
  tok("sim", "GIT-3", "finish", "first try");
  await page.locator("[data-testid=inbox-link]").click();
  await row("review", "GIT-3").waitFor({ timeout: 10_000 });
  await row("review", "GIT-3").locator("[data-testid=review-return-open]").click();
  await row("review", "GIT-3").getByLabel("What GIT-3 needs").fill("add a test");
  await row("review", "GIT-3").locator("[data-testid=review-return]").click();
  await row("review", "GIT-3").waitFor({ state: "detached", timeout: 10_000 });
  const returned = tok("show", "GIT-3");
  if (!returned.includes("running") || !returned.includes("add a test")) throw new Error(`Return in the row did not reach the agent:\n${returned}`);
  console.log("PASS Return in the inbox row sends the card back with the comment");
  await row("review", "GIT-2").locator("[data-testid=review-accept]").click();
  await row("review", "GIT-2").waitFor({ state: "detached", timeout: 10_000 });
  for (let i = 0; i < 100 && !tok("show", "GIT-2").includes("merged"); i++) await new Promise((r) => setTimeout(r, 100));
  if (!tok("show", "GIT-2").includes("merged")) throw new Error(`Accept in the row did not merge the card:\n${tok("show", "GIT-2")}`);
  console.log("PASS Accept in the inbox row merges the card");
  if (failed) throw new Error("the page threw errors");
  console.log("inbox: PASS");
} catch (e) {
  console.error(`inbox: FAIL: ${e.message}`);
  process.exitCode = 1;
} finally {
  await browser?.close();
  await cleanup(tok, home);
}
