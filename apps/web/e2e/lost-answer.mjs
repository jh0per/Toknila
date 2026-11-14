// Browser test of a person's answer reaching an agent that the app lost, against a real daemon and
// the workspace's `fake-agent` (never a real agent), in a real git ship.
// Usage: node e2e/lost-answer.mjs <tok binary> <fake-agent binary>   (cargo xtask e2e-web runs it)
// needs: fake-agent
//
// A card runs; the daemon is killed twice in a row (the fake agent's process is left behind, as
// after an update), so the card is Blocked with a lost runner. Its status block and its Inbox row
// say what to press. The composer says an answer starts the agent; the person answers in the chat
// ("Recommended"), the bubble reads that the agent is starting, then "delivered", and the card runs
// again with the answer in its new run.

import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, join, resolve } from "node:path";
import { checks, cleanup, launch, processesWith } from "./helpers.mjs";

const [tokArg, agentArg] = process.argv.slice(2);
if (!tokArg || !agentArg) {
  console.error("usage: node e2e/lost-answer.mjs <tok binary> <fake-agent binary>");
  process.exit(2);
}
const tokBin = resolve(tokArg);
const fakeAgent = resolve(agentArg);
const home = mkdtempSync(join(tmpdir(), "toknila-lost-"));
const marker = `lost-${basename(home)}`;
const run = (cmd, args, opts = {}) => execFileSync(cmd, args, { encoding: "utf8", ...opts });
const tok = (...args) => run(tokBin, args, { env: { ...process.env, TOKNILA_HOME: home } });
const git = (dir, ...args) =>
  run("git", ["-C", dir, "-c", "user.name=t", "-c", "user.email=t@example.com", "-c", "commit.gpgsign=false", ...args]);
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const status = (key) => JSON.parse(tok("show", key, "--json")).ticket.status;

/// The fake agents this script left running: their command line carries the marker (found the
/// same way on every OS by the helper; `ps` does not exist on Windows).
const leftovers = () => processesWith(marker);

async function until(what, f) {
  for (const stop = Date.now() + 30_000; Date.now() < stop; await sleep(100)) {
    if (f()) return;
  }
  throw new Error(`timed out waiting for ${what}`);
}

/// The update: the daemon dies at once and so does nothing it would have recorded; its agent's
/// process is gone with it (killed here by its pid). The next command starts the daemon again.
function lose() {
  const pid = JSON.parse(readFileSync(join(home, "daemon.json"), "utf8")).pid;
  process.kill(pid, "SIGKILL");
  for (const p of leftovers()) {
    try {
      process.kill(p, "SIGKILL");
    } catch {
      /* gone meanwhile */
    }
  }
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
  writeFileSync(join(home, "profiles", "holder.toml"), `command = ['${fakeAgent}', 'print', '${marker}', 'echo', 'hold']\n`);
  tok("project", "new", "LST", "--repo", repo, "--base", "develop", "--profile", "holder");
  tok("chain", "new", "work", "--project", "LST");
  tok("ticket", "new", "work", "Lost");
  await until("the card to run", () => status("LST-1") === "running" && leftovers().length > 0);

  lose();
  tok("daemon", "status"); // starts the daemon again: the card is paused, then comes back
  await until("the agent to come back after the first loss", () => status("LST-1") === "running" && leftovers().length > 0);
  lose();
  tok("daemon", "status");
  await until("the second loss to block the card", () => status("LST-1") === "blocked");
  console.log("PASS a second loss within ten minutes blocks the card");

  const { url } = JSON.parse(tok("open", "--json"));
  browser = await launch();
  const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
  const errors = [];
  page.on("pageerror", (e) => errors.push(e.message));
  const { expectText } = checks(page);
  const t = (id) => page.locator(`[data-testid=${id}]`);
  const sentence = "The app restarted twice while the agent worked. Unblock starts it once more; it gets your answer.";

  // The Inbox row says what to press.
  await page.goto(url.replace("/?", "/inbox?"));
  await expectText(t("inbox-kind"), sentence, "the Inbox row says the app restarted and Unblock starts it");

  // The card: the status block says it, and the composer says an answer starts the agent.
  await page.goto(url.replace("/?", "/projects/LST/chains/work/cards/LST-1?"));
  await expectText(t("status-sentence"), sentence, "the status block says what to press");
  await page.getByRole("tab", { name: "Chat" }).click();
  const box = t("chat-input");
  await box.waitFor();
  const placeholder = await box.getAttribute("placeholder");
  // No agent is live, so Ask is the default (TOKNILA-120); Tell the agent is the button that answers.
  if (placeholder !== "Ask what to do, or tell the agent for its next run") throw new Error(`placeholder: ${placeholder}`);
  console.log(`PASS the composer says: ${placeholder}`);

  // Every sentence the pending line shows, from the moment the answer is sent.
  await page.evaluate(() => {
    window.__pending = [];
    new MutationObserver(() => {
      for (const el of document.querySelectorAll("[data-testid=bubble-pending]")) {
        const text = el.textContent ?? "";
        if (text && !window.__pending.includes(text)) window.__pending.push(text);
      }
    }).observe(document.body, { subtree: true, childList: true, characterData: true });
  });
  await box.fill("Recommended");
  await t("chat-send").click();
  await expectText(t("chat-items"), "Recommended", "the answer is a bubble of the person's");
  await t("bubble-delivered").waitFor({ timeout: 20_000 });
  const seen = await page.evaluate(() => window.__pending);
  if (!seen.includes("Starting the agent; it gets this when it is up.")) {
    throw new Error(`the bubble never said the agent is starting; it said: ${JSON.stringify(seen)}`);
  }
  console.log("PASS the bubble said the agent is starting, then delivered");
  await until("the card to run again", () => status("LST-1") === "running" && leftovers().length > 0);
  console.log("PASS one Send answered and restarted the agent");
  if (errors.length) throw new Error(`page errors: ${errors.join("; ")}`);
} catch (e) {
  console.error(`FAIL ${e.message}`);
  process.exitCode = 1;
} finally {
  await browser?.close();
  for (const p of leftovers()) {
    try {
      process.kill(p, "SIGKILL");
    } catch {
      /* gone meanwhile */
    }
  }
  await cleanup(tok, home);
}
