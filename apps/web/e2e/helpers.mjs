// What the browser e2e scripts share: the checks on a page, and launching the browser.

import { readFileSync, rmSync } from "node:fs";
import { join } from "node:path";
import { chromium } from "playwright";

export const launch = () =>
  chromium.launch(process.env.PW_CHROMIUM ? { executablePath: process.env.PW_CHROMIUM } : {});

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

/// Waits up to `ms` for the process `pid` to be gone; returns whether it is.
async function waitExit(pid, ms) {
  for (const until = Date.now() + ms; alive(pid); await sleep(50)) {
    if (Date.now() > until) return false;
  }
  return true;
}

/// Stops the daemon of `home` and waits for its process to exit, not just for it to stop answering:
/// after `daemon stop` it may still be closing files (the database, run logs, ships), and removing
/// the folder under it fails with ENOTEMPTY on macOS.
export async function stopDaemon(tok, home) {
  let pid;
  try {
    pid = JSON.parse(readFileSync(join(home, "daemon.json"), "utf8")).pid;
  } catch {
    /* no daemon was started */
  }
  try {
    tok("daemon", "stop");
  } catch {
    /* already stopped */
  }
  if (pid && !(await waitExit(pid, 15_000))) {
    console.warn(`warning: daemon ${pid} did not exit after stop; killing it`);
    try {
      process.kill(pid, "SIGKILL");
    } catch {
      /* gone meanwhile */
    }
    await waitExit(pid, 5_000);
  }
}

/// Final cleanup of a script: stops the daemon, waits for it, removes the temp home. A folder that
/// cannot be removed is a warning only: the checks have already decided the exit code.
export async function cleanup(tok, home) {
  await stopDaemon(tok, home);
  try {
    rmSync(home, { recursive: true, force: true, maxRetries: 5, retryDelay: 200 });
  } catch (e) {
    console.warn(`warning: cannot remove ${home}: ${e.message}`);
  }
}
