// What the browser e2e scripts share: the checks on a page, and launching the browser.

import { execFileSync, spawn } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";

/// Launches the browser. With `E2E_ARTIFACTS` set (CI), every page is traced, and when the script
/// has failed by the time it closes the browser (`process.exitCode` is set), a screenshot of each
/// page and its trace are written to `$E2E_ARTIFACTS/<script>/` for the CI artifact.
export async function launch() {
  const browser = await chromium.launch(process.env.PW_CHROMIUM ? { executablePath: process.env.PW_CHROMIUM } : {});
  const dir = process.env.E2E_ARTIFACTS;
  if (!dir) return browser;
  const out = join(dir, basename(process.argv[1] ?? "e2e", ".mjs"));
  const pages = [];
  const newPage = browser.newPage.bind(browser);
  browser.newPage = async (...args) => {
    const page = await newPage(...args);
    await page.context().tracing.start({ screenshots: true, snapshots: true });
    pages.push(page);
    return page;
  };
  const close = browser.close.bind(browser);
  browser.close = async (...args) => {
    const failed = Boolean(process.exitCode);
    if (failed) mkdirSync(out, { recursive: true });
    for (const [i, page] of pages.entries()) {
      if (failed) await page.screenshot({ path: join(out, `failed-${i}.png`) }).catch(() => {});
      await page.context().tracing.stop(failed ? { path: join(out, `trace-${i}.zip`) } : {}).catch(() => {});
    }
    return close(...args);
  };
  return browser;
}

/// Checks bound to a page; each prints `PASS ...` or throws with what it saw instead.
export function checks(page) {
  const expectText = async (loc, text, what) => {
    await loc.filter({ hasText: text }).first().waitFor({ timeout: 10_000 }).catch(async () => {
      throw new Error(`${what}: expected "${text}", got "${await loc.first().textContent().catch(() => "?")}"`);
    });
    console.log(`PASS ${what}: ${text}`);
  };
  const expectPath = async (path, what) => {
    await page.waitForFunction((p) => window.location.pathname === p, path, { timeout: 10_000 }).catch(() => {
      throw new Error(`${what}: expected path ${path}, at ${new URL(page.url()).pathname}`);
    });
    console.log(`PASS ${what}: ${path}`);
  };
  /// The window's connection to the daemon: the sidebar says "Offline" while it is lost and says
  /// nothing while it works (`data-link` carries the state).
  const expectLink = async (state, what) => {
    await page.locator(`nav[aria-label=Sidebar][data-link=${state}]`).waitFor({ timeout: 10_000 }).catch(() => {
      throw new Error(`${what}: expected the link to be ${state}`);
    });
    const badge = await page.locator("[data-testid=link]").count();
    if (state === "offline" ? badge !== 1 : badge !== 0) throw new Error(`${what}: the Offline badge count is ${badge} while ${state}`);
    console.log(`PASS ${what}: ${state}`);
  };
  return { expectText, expectPath, expectLink };
}

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

const alive = (pid) => {
  try {
    process.kill(pid, 0);
    return true;
  } catch (e) {
    return e.code === "EPERM";
  }
};

/// The ids of the processes whose command line carries `marker`, this process left out. On
/// Windows there is no `ps`, so the list comes from PowerShell's Win32_Process (a second per
/// call); elsewhere from `ps`.
export function processesWith(marker) {
  const out =
    process.platform === "win32"
      ? execFileSync(
          "powershell",
          ["-NoProfile", "-NonInteractive", "-Command",
            `Get-CimInstance Win32_Process | Where-Object { $_.CommandLine -like '*${marker}*' } | ForEach-Object { "$($_.ProcessId) $($_.CommandLine)" }`],
          { encoding: "utf8" },
        )
      : execFileSync("ps", ["-axo", "pid=,command="], { encoding: "utf8" });
  return out
    .split("\n")
    .filter((l) => l.includes(marker) && !l.includes("Win32_Process"))
    .map((l) => Number(l.trim().split(/\s+/)[0]))
    .filter((p) => p && p !== process.pid);
}

/// A blocking pause: cleanup also runs from `process.on("exit")`, where nothing async can finish.
const pause = (ms) => Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, ms);

/// Waits up to `ms` for the process `pid` to be gone; returns whether it is.
function waitExit(pid, ms) {
  for (const until = Date.now() + ms; alive(pid); pause(50)) {
    if (Date.now() > until) return false;
  }
  return true;
}

/// The ids of every descendant of `pids` (children, grandchildren...), from `ps`. Empty on Windows,
/// where `taskkill /T` ends the tree itself.
function descendants(pids) {
  if (process.platform === "win32" || pids.length === 0) return [];
  let rows;
  try {
    rows = execFileSync("ps", ["-axo", "pid=,ppid="], { encoding: "utf8" })
      .split("\n")
      .map((l) => l.trim().split(/\s+/).map(Number));
  } catch {
    return [];
  }
  const found = new Set();
  for (let frontier = pids; frontier.length; ) {
    frontier = rows.filter(([pid, ppid]) => !found.has(pid) && frontier.includes(ppid)).map(([pid]) => pid);
    for (const p of frontier) found.add(p);
  }
  return [...found];
}

/// Ends the process `pid` and everything it started: `taskkill /T /F` on Windows (a plain kill
/// leaves the children), the children first and then the process itself elsewhere.
function killTree(pid) {
  if (process.platform === "win32") {
    try {
      execFileSync("taskkill", ["/T", "/F", "/PID", String(pid)], { stdio: "ignore" });
    } catch {
      /* gone meanwhile */
    }
    return;
  }
  for (const p of [...descendants([pid]), pid]) {
    try {
      process.kill(p, "SIGKILL");
    } catch {
      /* gone meanwhile */
    }
  }
}

/// Stops the daemon of `home` and waits for its process to exit, not just for it to stop answering:
/// after `daemon stop` it may still be closing files (the database, run logs, ships), and removing
/// the folder under it fails with ENOTEMPTY on macOS. A daemon that is still there after 15 s is killed.
function stopDaemonSync(tokBin, home) {
  let pid;
  try {
    pid = JSON.parse(readFileSync(join(home, "daemon.json"), "utf8")).pid;
  } catch {
    /* no daemon was started */
  }
  try {
    execFileSync(tokBin, ["daemon", "stop"], {
      env: { ...process.env, TOKNILA_HOME: home },
      stdio: "ignore",
      timeout: 20_000,
    });
  } catch {
    /* already stopped */
  }
  if (pid && !waitExit(pid, 15_000)) {
    console.warn(`warning: daemon ${pid} did not exit after stop; killing it`);
    killTree(pid);
    waitExit(pid, 5_000);
  }
}

const sessions = new Set();
let hooked = false;

/// Ends every session still open. Safe to call twice: a session ends once.
function endAll() {
  for (const s of [...sessions]) s.endSync();
}

function hookProcess() {
  if (hooked) return;
  hooked = true;
  process.on("exit", endAll);
  for (const [signal, code] of [["SIGINT", 130], ["SIGTERM", 143]]) {
    process.on(signal, () => {
      endAll();
      process.exit(code);
    });
  }
  process.on("uncaughtException", (e) => {
    console.error(`FAIL uncaught: ${e?.stack ?? e}`);
    endAll();
    process.exit(1);
  });
  process.on("unhandledRejection", (e) => {
    console.error(`FAIL unhandled rejection: ${e?.stack ?? e}`);
    endAll();
    process.exit(1);
  });
}

/// The one owner of what a spec starts. Makes the temp home (`<tmp>/toknila-e2e-<run id>-<spec>-<random>`),
/// knows the marker of its fake agents (`toknila-fa-<run id>-<spec>-<random>`, the run id from
/// `TOKNILA_E2E_RUN`; a profile puts it in the agent's arguments as `'sleep', '<marker>'`, a step
/// that waits for nothing) and registers one cleanup that runs on `cleanup()` in `finally`, on
/// process exit, on SIGINT and SIGTERM and on an uncaught error: stops the daemon (killed after a
/// wait), kills every process whose command line carries the marker with its children, removes the
/// home. A spec starts no process outside it; a second daemon is registered with `session.track(home)`.
/// `opts.tok` is the `tok` binary (default: the spec's first argument), `opts.name` the spec's name.
export function session(opts = {}) {
  const name = opts.name ?? basename(process.argv[1] ?? "e2e", ".mjs");
  const tokBin = resolve(opts.tok ?? process.argv[2] ?? "tok");
  const runId = process.env.TOKNILA_E2E_RUN ?? "manual";
  const prefix = `toknila-e2e-${runId}-${name}-`;
  const home = mkdtempSync(join(tmpdir(), prefix));
  const suffix = basename(home).slice(prefix.length);
  const marker = `toknila-fa-${runId}-${name}-${suffix}`;
  const homes = [home];
  let ended = false;
  const s = {
    home,
    marker,
    /// Registers the home of another daemon this spec starts; it is stopped and removed with the rest.
    track(other) {
      homes.push(other);
      writeWatchList(home, homes);
      return other;
    },
    endSync() {
      if (ended) return;
      ended = true;
      sessions.delete(s);
      if (watcher) {
        try {
          process.kill(watcher, "SIGKILL");
        } catch {
          /* gone meanwhile */
        }
      }
      cleanupHomes(tokBin, homes, marker);
    },
  };
  writeWatchList(home, homes);
  const watcher = startWatcher(tokBin, home, marker);
  sessions.add(s);
  hookProcess();
  return s;
}

/// Stops the daemon of each home, kills every process carrying `marker` with its children and
/// removes the homes: the one cleanup of a session, run by the spec or by its watcher.
function cleanupHomes(tokBin, homes, marker) {
  for (const h of homes) stopDaemonSync(tokBin, h);
  for (const pid of processesWith(marker)) killTree(pid);
  for (const h of homes) {
    try {
      rmSync(h, { recursive: true, force: true, maxRetries: 5, retryDelay: 200 });
    } catch (e) {
      console.warn(`warning: cannot remove ${h}: ${e.message}`);
    }
  }
}

const WATCH_FILE = ".session-homes.json";

/// Tells the watcher which homes the session owns (the home itself and every `track`ed one).
function writeWatchList(home, homes) {
  try {
    writeFileSync(join(home, WATCH_FILE), JSON.stringify(homes));
  } catch {
    /* the home is gone: the session has ended */
  }
}

/// A SIGKILL (a runner's timeout, `kill -9`) runs no handler, so nothing in the spec's own process
/// can end its daemon and fake agents. This starts a detached watcher, a second `node` running this
/// file, that waits until the spec's process is gone and then does the cleanup. `endSync` kills it.
/// Returns its pid, or undefined when it cannot start.
function startWatcher(tokBin, home, marker) {
  try {
    const child = spawn(process.execPath, [fileURLToPath(import.meta.url), WATCH_ARG, String(process.pid), tokBin, home], {
      detached: true,
      stdio: "ignore",
      windowsHide: true,
      env: { ...process.env, TOKNILA_WATCH_MARKER: marker },
    });
    child.unref();
    return child.pid;
  } catch {
    return undefined;
  }
}

const WATCH_ARG = "--watch-session";

/// The watcher's body: wait for the spec's process `parent` to end, then clean up what its session
/// owned (the homes named in the home's watch list; the marker comes in the environment, not the
/// command line, so the leak checks never take the watcher for a fake agent).
function watch(parent, tokBin, home) {
  while (alive(parent)) pause(500);
  let homes = [home];
  try {
    homes = JSON.parse(readFileSync(join(home, WATCH_FILE), "utf8"));
  } catch {
    /* no list: the home alone */
  }
  cleanupHomes(tokBin, homes, process.env.TOKNILA_WATCH_MARKER);
}

if (process.argv[2] === WATCH_ARG && process.argv[1] && fileURLToPath(import.meta.url) === resolve(process.argv[1])) {
  const [, , , parent, tokBin, home] = process.argv;
  watch(Number(parent), tokBin, home);
  process.exit(0);
}

/// The workspace folder of a card. The daemon makes it after the command that started the card
/// has answered, so this waits (up to 20 s) until the card has one.
export function shipOf(tok, key) {
  const until = Date.now() + 20_000;
  for (;;) {
    const path = JSON.parse(tok("show", key, "--json")).ship?.path;
    if (path) return path;
    if (Date.now() > until) throw new Error(`${key} has no workspace folder`);
    Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, 100);
  }
}

/// Final cleanup of a script (its `finally`): ends the session that made `home` (see `session`).
/// A folder that cannot be removed is a warning only: the checks have already decided the exit code.
export async function cleanup(_tok, home) {
  const s = [...sessions].find((x) => x.home === home);
  if (s) s.endSync();
  else if (existsSync(home)) throw new Error(`cleanup: ${home} is not the home of a session`);
}

/// With `SCREENSHOT_DIR` set: a screenshot of the page (or of `target`, a locator) in light and
/// dark, as `<name>-light.png` and `<name>-dark.png`. Without it nothing happens.
export async function shoot(page, name, target = page) {
  const dir = process.env.SCREENSHOT_DIR;
  if (!dir) return;
  for (const scheme of ["light", "dark"]) {
    await page.emulateMedia({ colorScheme: scheme });
    await target.screenshot({ path: join(dir, `${name}-${scheme}.png`) });
  }
  await page.emulateMedia({ colorScheme: null });
}
