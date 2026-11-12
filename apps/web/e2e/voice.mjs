// Browser test of speaking to the assistant, with a FAKE recognizer injected into the page (the
// real one is the system's and is tried by a person in the Tauri window), against a real daemon and
// the workspace's `fake-agent`.
// Usage: node e2e/voice.mjs <tok binary> <fake-agent binary>   (cargo xtask e2e-web runs it)
// needs: fake-agent
//
// The microphone button listens and stops; interim words show muted outside the field and final
// words fill it; typing stops listening; a refusal says what to allow and where; with no recognizer
// the hint names the system's dictation; "send when I stop speaking" sends after two seconds; the
// language is the window's (Ukrainian works); light and dark both show the button.

import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { cleanup, launch, shoot } from "./helpers.mjs";

const [tokArg, agentArg] = process.argv.slice(2);
if (!tokArg || !agentArg) {
  console.error("usage: node e2e/voice.mjs <tok binary> <fake-agent binary>");
  process.exit(2);
}
const tokBin = resolve(tokArg);
const fakeAgent = resolve(agentArg);
const home = mkdtempSync(join(tmpdir(), "toknila-voice-"));
const run = (cmd, args) => execFileSync(cmd, args, { encoding: "utf8" });
const tok = (...args) => execFileSync(tokBin, args, { env: { ...process.env, TOKNILA_HOME: home }, encoding: "utf8" });
const git = (dir, ...args) =>
  run("git", ["-C", dir, "-c", "user.name=t", "-c", "user.email=t@example.com", "-c", "commit.gpgsign=false", ...args]);

/// A recognizer like the system's: the test says what it hears. The last one made is `window.__rec`.
const FAKE = `
  class FakeRecognizer {
    constructor() { window.__rec = this; this.started = false; }
    start() { if (window.__recFail) { setTimeout(() => this.onerror && this.onerror({ error: window.__recFail }), 0); return; } this.started = true; }
    stop() { this.started = false; setTimeout(() => this.onend && this.onend(), 0); }
    say(parts) {
      this.onresult({ results: parts.map(([t, f]) => Object.assign([{ transcript: t }], { isFinal: f })) });
    }
  }
  window.SpeechRecognition = FakeRecognizer;
  window.webkitSpeechRecognition = FakeRecognizer;
`;
const NONE = "window.SpeechRecognition = undefined; window.webkitSpeechRecognition = undefined;";

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
  const answer = ["echo", "post", "/planner/say", JSON.stringify({ text: "Done." })];
  const steps = ["print", "planner ready", ...answer, ...answer, ...answer, "hold"].map((x) => `'${x}'`).join(", ");
  writeFileSync(join(home, "profiles", "fake-planner.toml"), `role = 'planner'\ncommand = ['${fakeAgent}', ${steps}]\nstartup_secs = 0\n`);
  tok("project", "new", "PV", "--repo", repo, "--base", "develop");
  tok("chain", "new", "work", "--project", "PV", "--no-autostart");
  tok("ticket", "new", "work", "Login screen");
  tok("planner", "start", "PV", "--profile", "fake-planner");
  const { url } = JSON.parse(tok("open", "--json"));
  const origin = new URL(url).origin;
  const bearer = { authorization: `Bearer ${new URL(url).searchParams.get("token")}` };
  const scrollback = async () => await (await fetch(`${origin}/v1/projects/PV/planner/scrollback`, { headers: bearer })).text();
  const toldAbout = async (line) => {
    for (const stop = Date.now() + 15_000; Date.now() < stop; await new Promise((r) => setTimeout(r, 150))) {
      if ((await scrollback()).includes(`got:${line}`)) return console.log(`PASS the assistant was told: ${line}`);
    }
    throw new Error(`the assistant was not told "${line}": ${(await scrollback()).slice(-400)}`);
  };
  const at = (path) => {
    const u = new URL(url);
    u.pathname = path;
    return u.href;
  };

  browser = await launch();
  const errors = [];
  const open = async (script, locale) => {
    const ctx = await browser.newContext({ viewport: { width: 1400, height: 900 }, locale });
    await ctx.addInitScript(script);
    const page = await ctx.newPage();
    page.on("pageerror", (e) => errors.push(e.message));
    await page.goto(at("/projects/PV/chains/work"));
    await page.locator("[data-testid=assistant-toggle]").click();
    await page.locator("[data-testid=planner-ask-input]").waitFor();
    return page;
  };
  const check = (cond, what) => {
    if (!cond) throw new Error(what);
    console.log(`PASS ${what}`);
  };

  /// The store applies an edit a moment after the event, so the field is waited for, not read at once.
  const fieldIs = async (p, want, what) => {
    await p.waitForFunction((w) => document.querySelector("[data-testid=planner-ask-input]")?.value === w, want, { timeout: 5000 }).catch(async () => {
      throw new Error(`${what}: expected ${JSON.stringify(want)}, got ${JSON.stringify(await p.locator("[data-testid=planner-ask-input]").inputValue())}`);
    });
    console.log(`PASS ${what}`);
  };

  // 1. No recognizer: no button, the hint names the system's dictation.
  const bare = await open(NONE);
  check((await bare.locator("[data-testid=planner-ask-mic]").count()) === 0, "with no recognizer there is no microphone button");
  check((await bare.locator("[data-testid=planner-ask-mic-note]").innerText()).includes("Fn twice"), "with no recognizer the hint says Fn twice");
  await bare.context().close();

  // 2. Listening: press, interim words show muted, final words fill the field, press again to stop.
  const page = await open(FAKE, "en-US");
  const t = (id) => page.locator(`[data-testid=${id}]`);
  const mic = t("planner-ask-mic");
  check((await mic.getAttribute("data-mic")) === "idle", "the button is idle at first");
  await mic.click();
  check((await mic.getAttribute("data-mic")) === "listening", "pressing the button listens");
  check((await page.evaluate(() => window.__rec.started)) === true, "the recognizer was started");
  check((await page.evaluate(() => window.__rec.lang)) === "en-US", "the language is the window's (en-US)");
  await page.evaluate(() => window.__rec.say([["build an", false]]));
  check((await t("planner-ask-mic-interim").innerText()) === "build an", "interim words show in the muted line");
  check((await t("planner-ask-input").inputValue()) === "", "interim words are not in the field yet");
  await page.evaluate(() => window.__rec.say([["build an export", true], ["to csv", false]]));
  await fieldIs(page, "build an export", "final words fill the field");
  await shoot(page, "voice-listening", t("planner-ask"));
  await mic.click();
  check((await mic.getAttribute("data-mic")) === "idle", "pressing again stops");
  await fieldIs(page, "build an export to csv", "stopping keeps the interim words in the field");

  // 3. The person edits and sends.
  await t("planner-ask-input").fill("build an export to csv please");
  await t("planner-ask-send").click();
  await toldAbout("About chain work: build an export to csv please");

  // 4. Typing while listening stops listening and keeps the typed words (after the box is free again).
  await page.waitForFunction(() => document.querySelector("[data-testid=planner-ask-input]")?.disabled === false);
  await mic.click();
  await page.evaluate(() => window.__rec.say([["hello", true]]));
  await fieldIs(page, "hello", "the heard words reach the field");
  await t("planner-ask-input").press("End");
  await t("planner-ask-input").pressSequentially("!");
  check((await mic.getAttribute("data-mic")) === "idle", "typing stops listening");
  check((await t("planner-ask-input").inputValue()).endsWith("hello!"), `typing (${JSON.stringify(await t("planner-ask-input").inputValue())}) keeps the heard words and adds the typed ones`);
  await t("planner-ask-input").fill("");

  // 5. A refusal says what to allow and where; the button is pressable again.
  await page.evaluate(() => { window.__recFail = "not-allowed"; });
  await mic.click();
  await t("planner-ask-mic-note").waitFor();
  check((await t("planner-ask-mic-note").innerText()).includes("System Settings"), "a refusal says what to allow and where");
  check((await mic.getAttribute("data-mic")) === "denied", "the button shows the refusal");

  // 6. Another error shows its sentence.
  await page.evaluate(() => { window.__recFail = "network"; });
  await mic.click();
  await page.waitForFunction(() => document.querySelector("[data-testid=planner-ask-mic]")?.getAttribute("data-mic") === "error");
  check((await t("planner-ask-mic-note").innerText()).includes("network"), "an error shows its sentence");
  await page.evaluate(() => { window.__recFail = ""; });

  // 7. "Send when I stop speaking": two seconds of silence send.
  await t("planner-ask-mic-auto").check();
  await mic.click();
  await page.evaluate(() => window.__rec.say([["go go", true]]));
  await page.waitForTimeout(1000);
  check((await t("planner-ask-input").inputValue()) === "go go", "before the pause the words wait in the field");
  await toldAbout("About chain work: go go");
  check((await mic.getAttribute("data-mic")) === "idle", "after an automatic send the microphone is off");
  await t("planner-ask-mic-auto").uncheck();
  await page.context().close();

  // 8. Ukrainian: the window's language goes to the recognizer and the words arrive as spoken; dark works too.
  const uk = await open(FAKE, "uk-UA");
  await uk.emulateMedia({ colorScheme: "dark" });
  await uk.locator("[data-testid=planner-ask-mic]").click();
  check((await uk.evaluate(() => window.__rec.lang)) === "uk-UA", "the language is the window's (uk-UA)");
  await uk.evaluate(() => window.__rec.say([["створи два релізи", true]]));
  await fieldIs(uk, "створи два релізи", "Ukrainian words fill the field");
  await shoot(uk, "voice-listening-ukrainian-dark", uk.locator("[data-testid=planner-ask]"));
  await uk.context().close();

  if (errors.length) throw new Error(`the page threw: ${errors.join("; ")}`);
  console.log("web voice: PASS");
} catch (e) {
  console.error(`web voice: FAIL: ${e.message.slice(0, 800)}`);
  process.exitCode = 1;
} finally {
  await browser?.close();
  try {
    tok("planner", "stop", "PV");
  } catch {
    /* not running */
  }
  await cleanup(tok, home);
}
