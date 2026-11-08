// Browser test of the project's setting for commands and "Allow always", against a real daemon
// and the workspace's `fake-agent` (never a real agent), in a real git ship.
// Usage: node e2e/commands.mjs <tok binary> <fake-agent binary>   (cargo xtask e2e-web runs it)
//
// A new project starts on "Let Claude decide". In Settings, Agents, the person chooses "Ask me
// each time" and saves (`tok project show` agrees). Two cards then play an agent's permission hook
// for `cargo test -p core` and `cargo test --lib`: both questions reach the inbox with an Allow
// always button. One click on it answers both (the rule is `cargo test *`), the inbox empties, both
// hooks got "allow", and `tok project show` lists the rule. A third card asks for the same words
// in a list of commands (`cargo test; rm -rf x`): it is still asked. Last, Remove on the
// remembered rule and Save forget it.

import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { checks, cleanup, launch } from "./helpers.mjs";

const [tokArg, agentArg] = process.argv.slice(2);
if (!tokArg || !agentArg) {
  console.error("usage: node e2e/commands.mjs <tok binary> <fake-agent binary>");
  process.exit(2);
}
const tokBin = resolve(tokArg);
const fakeAgent = resolve(agentArg);
const home = mkdtempSync(join(tmpdir(), "toknila-cmd-"));
const run = (cmd, args) => execFileSync(cmd, args, { encoding: "utf8" });
const tok = (...args) => execFileSync(tokBin, args, { env: { ...process.env, TOKNILA_HOME: home }, encoding: "utf8" });
const git = (dir, ...args) =>
  run("git", ["-C", dir, "-c", "user.name=t", "-c", "user.email=t@example.com", "-c", "commit.gpgsign=false", ...args]);
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const logs = (key) => tok("logs", key);

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
  const ask = (tool) =>
    steps(
      "post", "/tickets/{ticket}/activity", JSON.stringify({ signal: "hook:pre_tool_use", tool }),
      "post", "/tickets/{ticket}/permission", "{}",
      "hold",
    );
  for (const [name, tool] of [
    ["first", "Bash: cargo test -p core"],
    ["second", "Bash: cd \"/w/ship\"; cargo test --lib"],
    ["third", "Bash: cargo test; rm -rf x"],
  ]) {
    writeFileSync(join(home, "profiles", `${name}.toml`), `command = ['${fakeAgent}', ${ask(tool)}]\n`);
  }
  tok("project", "new", "QA", "--repo", repo, "--base", "develop", "--profile", "first");
  tok("project", "set", "QA", "--parallel", "6");
  tok("chain", "new", "work", "--project", "QA");
  const { url } = JSON.parse(tok("open", "--json"));
  const at = (path) => url.replace("/?", `${path}?`);

  browser = await launch();
  const page = await browser.newPage();
  const errors = [];
  page.on("pageerror", (e) => errors.push(e.message));
  const { expectText } = checks(page);
  const t = (id) => page.locator(`[data-testid=${id}]`);

  // A new project starts on auto; choosing "Ask me each time" shows the Save bar and saves.
  await page.goto(at("/projects/QA/settings/agents"));
  await t("command-policy-auto").waitFor({ timeout: 10_000 });
  if (!(await t("command-policy-auto").isChecked())) throw new Error("a new project is not on auto");
  await expectText(t("command-policy"), "Claude Code decides and asks you only about risky actions", "each choice says what it means");
  await expectText(t("allowed-commands"), "None yet", "no command is remembered yet");
  await t("command-policy-ask").check();
  await t("save-settings").click();
  await t("saved").waitFor({ timeout: 10_000 });
  await until("the daemon to say ask", () => tok("project", "show", "QA").includes("commands: ask"));
  console.log("PASS the setting is chosen in Settings and saved");

  // Two cards ask for a test run; one click on Allow always answers both.
  tok("ticket", "new", "work", "One");
  tok("ticket", "new", "work", "Two", "--profile", "second");
  await page.goto(at("/inbox"));
  const inbox = t("inbox");
  const row = (key) => inbox.locator(`li[data-kind=question][data-key="${key}"]`);
  await expectText(row("QA-1"), "Bash: cargo test -p core", "the first card asks");
  await expectText(row("QA-2"), "cargo test --lib", "the second card asks");
  await row("QA-1").locator("[data-testid=answer-allow-always]").click();
  await row("QA-1").waitFor({ state: "detached", timeout: 10_000 });
  await row("QA-2").waitFor({ state: "detached", timeout: 10_000 });
  await until("both hooks got allow", () =>
    ["QA-1", "QA-2"].every((k) => logs(k).includes('"decision":"allow"')),
  );
  console.log("PASS Allow always removed both questions from the inbox and the agents were allowed");
  const shown = tok("project", "show", "QA");
  if (!shown.includes("allowed always: Bash(cargo test *)")) throw new Error(`tok project show: ${shown}`);
  console.log("PASS tok project show lists the remembered rule");

  // The same words inside a list of commands are not covered by the rule: still asked.
  tok("ticket", "new", "work", "Three", "--profile", "third");
  await expectText(row("QA-3"), "cargo test; rm -rf x", "a list of commands is still asked");
  if ((await row("QA-3").locator("[data-testid=answer-allow-always]").count()) !== 1) {
    throw new Error("Allow always is offered for a command question");
  }

  // The rule is listed in Settings; Remove and Save forget it.
  await page.goto(at("/projects/QA/settings/agents"));
  await expectText(t("allowed-commands"), "cargo test *", "the rule is listed in Settings");
  await t("forget-rule-0").click();
  await t("save-settings").click();
  await t("saved").waitFor({ timeout: 10_000 });
  await until("the rule to be gone", () => !tok("project", "show", "QA").includes("allowed always"));
  await expectText(t("allowed-commands"), "None yet", "the list is empty again");
  console.log("PASS Remove and Save forget the rule");
  if (errors.length) throw new Error(`the page threw: ${errors.join("; ")}`);
  console.log("web commands: PASS");
} catch (e) {
  console.error(`web commands: FAIL: ${e.message.slice(0, 600)}`);
  process.exitCode = 1;
} finally {
  await browser?.close();
  await cleanup(tok, home);
}
