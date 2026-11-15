// Test of the e2e helper's `session()`: a spec that dies without reaching its `finally` leaves no
// daemon, no fake agent and no home behind. Real daemon and the workspace's `fake-agent`, no browser.
// Usage: node e2e/session-exit.mjs <tok binary> <fake-agent binary>   (cargo xtask e2e-web runs it)
// needs: fake-agent
//
// The parent starts this same file as a victim: it makes a session, starts a daemon and a fake agent
// through it, prints their marker, and then dies three ways before any `finally` (an uncaught error,
// `process.exit`, SIGTERM). The parent then checks, from outside, that nothing of that marker is alive.

import { execFileSync, spawn } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { cleanup, processesWith, session } from "./helpers.mjs";

const [tokArg, agentArg, role, mode] = process.argv.slice(2);
if (!tokArg || !agentArg) {
  console.error("usage: node e2e/session-exit.mjs <tok binary> <fake-agent binary>");
  process.exit(2);
}
const tokBin = resolve(tokArg);
const fakeAgent = resolve(agentArg);
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const alive = (pid) => {
  try {
    process.kill(pid, 0);
    return true;
  } catch (e) {
    return e.code === "EPERM";
  }
};

/// The direct children of `pids` (the `child` step of the fake agent starts one). On Windows there
/// is no `ps`; the check of the children is left to `taskkill /T` and the CI step that lists leftovers.
function kidsOf(pids) {
  if (process.platform === "win32") return [];
  return execFileSync("ps", ["-axo", "pid=,ppid="], { encoding: "utf8" })
    .split("\n")
    .map((l) => l.trim().split(/\s+/).map(Number))
    .filter(([pid, ppid]) => pid && pids.includes(ppid))
    .map(([pid]) => pid);
}

async function until(what, f) {
  for (const stop = Date.now() + 30_000; Date.now() < stop; await sleep(100)) {
    if (f()) return;
  }
  throw new Error(`timed out waiting for ${what}`);
}

if (role === "victim") {
  const { home, marker } = session();
  const tok = (...args) => execFileSync(tokBin, args, { env: { ...process.env, TOKNILA_HOME: home }, encoding: "utf8" });
  const git = (dir, ...args) =>
    execFileSync("git", ["-C", dir, "-c", "user.name=t", "-c", "user.email=t@example.com", "-c", "commit.gpgsign=false", ...args]);
  const repo = join(home, "repo");
  mkdirSync(repo, { recursive: true });
  git(repo, "init", "--quiet");
  git(repo, "symbolic-ref", "HEAD", "refs/heads/develop");
  writeFileSync(join(repo, "README.md"), "hi\n");
  git(repo, "add", "README.md");
  git(repo, "commit", "--quiet", "-m", "init");
  mkdirSync(join(home, "profiles"), { recursive: true });
  writeFileSync(join(home, "profiles", "holder.toml"), `command = ['${fakeAgent}', 'sleep', '${marker}', 'child', 'hold']\n`);
  tok("project", "new", "SEX", "--repo", repo, "--base", "develop", "--profile", "holder");
  tok("chain", "new", "work", "--project", "SEX");
  tok("ticket", "new", "work", "Exit");
  await until("the fake agent to run", () => processesWith(marker).length > 0);
  const daemon = JSON.parse(readFileSync(join(home, "daemon.json"), "utf8")).pid;
  const agents = processesWith(marker);
  // `kidsOf` is empty on Windows by design (no `ps`): waiting for a child there never ends.
  if (process.platform !== "win32") await until("the fake agent's child to run", () => kidsOf(agents).length > 0);
  console.log(`READY ${JSON.stringify({ marker, home, daemon, agents, kids: kidsOf(agents) })}`);
  if (mode === "throw") setTimeout(() => { throw new Error("boom before finally"); }, 100);
  else if (mode === "exit") setTimeout(() => process.exit(3), 100);
  else setInterval(() => {}, 1000); // sigterm: the parent ends it
} else {
  const { home } = session();
  let failed = false;
  try {
    for (const mode of ["throw", "exit", "sigterm"]) {
      const child = spawn(process.execPath, [process.argv[1], tokBin, fakeAgent, "victim", mode], {
        env: { ...process.env, TOKNILA_E2E_RUN: process.env.TOKNILA_E2E_RUN ?? "manual" },
        stdio: ["ignore", "pipe", "ignore"], // the victim's own "uncaught" report is expected
      });
      let out = "";
      child.stdout.on("data", (d) => (out += d));
      const exited = new Promise((r) => child.on("exit", (code, signal) => r({ code, signal })));
      await Promise.race([until("the victim to be ready", () => out.includes("READY ")), exited.then(() => { throw new Error(`the ${mode} victim ended before it was ready: ${out}`); })]);
      const seen = JSON.parse(out.split("READY ")[1].split("\n")[0]);
      // The controls: the probe finds what is there before the victim dies.
      if (seen.agents.length === 0) throw new Error("control: no fake agent carries the marker while the victim lives");
      if (!alive(seen.daemon)) throw new Error("control: the victim's daemon is not alive");
      if (mode === "sigterm") child.kill("SIGTERM");
      const { code, signal } = await exited;
      const left = processesWith(seen.marker);
      const children = [...seen.agents, ...seen.kids].filter(alive);
      if (left.length || alive(seen.daemon) || children.length || existsSync(seen.home)) {
        throw new Error(
          `${mode}: after the victim ended (code ${code}, signal ${signal}) alive: marker ${JSON.stringify(left)}, daemon ${alive(seen.daemon)}, agents ${JSON.stringify(children)}, home ${existsSync(seen.home)}`,
        );
      }
      console.log(`PASS ${mode}: no daemon, no fake agent and no home left (victim code ${code}, signal ${signal})`);
    }
  } catch (e) {
    console.error(`FAIL ${e.message}`);
    failed = true;
    process.exitCode = 1;
  } finally {
    await cleanup(null, home);
    if (failed) process.exitCode = 1;
  }
}
