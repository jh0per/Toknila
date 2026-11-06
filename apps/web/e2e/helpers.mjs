// What the browser e2e scripts share: the checks on a page, and launching the browser.

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
  return { expectText, expectPath };
}
