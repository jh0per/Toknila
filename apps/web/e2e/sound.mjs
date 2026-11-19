// Browser test of the sounds against a real daemon.
// Usage: node e2e/sound.mjs <path to tok binary>   (cargo xtask e2e-web runs it)
//
// The audio is a stub put in before the page loads: it counts every oscillator started and the
// frequency it was given, so "a sound played" is a number. With "A story is done" on, a story that
// merges (while another of its epic is still open) makes the window play the story sound exactly
// once (two notes, 660 and 880 Hz); with it off a story that merges makes none; and the Play button
// of Settings plays whatever the switch says.

import { execFileSync } from "node:child_process";
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { cleanup, launch, shipOf, session } from "./helpers.mjs";

const tokBin = process.argv[2];
if (!tokBin) {
  console.error("usage: node e2e/sound.mjs <tok binary>");
  process.exit(2);
}
const { home } = session();
const tok = (...args) =>
  execFileSync(tokBin, args, { env: { ...process.env, TOKNILA_HOME: home }, encoding: "utf8" });
const git = (cwd, ...args) =>
  execFileSync("git", ["-c", "user.name=t", "-c", "user.email=t@example.com", "-c", "commit.gpgsign=false", ...args], {
    cwd,
    encoding: "utf8",
  }).trim();
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

/// The stub of Web Audio: counts what is started, by frequency.
const STUB = () => {
  const log = { contexts: 0, starts: [] };
  window.__sounds = log;
  class Ctx {
    constructor() {
      log.contexts += 1;
      this.state = "suspended";
      this.currentTime = 0;
      this.destination = {};
    }
    resume() {
      this.state = "running";
      return Promise.resolve();
    }
    createGain() {
      return { gain: { setValueAtTime() {}, linearRampToValueAtTime() {} }, connect() {} };
    }
    createOscillator() {
      return {
        frequency: { value: 0 },
        type: "sine",
        connect() {},
        start() {
          log.starts.push(this.frequency.value);
        },
        stop() {},
      };
    }
  }
  window.AudioContext = Ctx;
  window.webkitAudioContext = Ctx;
};

let browser;
try {
  const repo = join(home, "repo");
  mkdirSync(repo);
  git(repo, "init", "--quiet");
  git(repo, "symbolic-ref", "HEAD", "refs/heads/develop");
  writeFileSync(join(repo, "README.md"), "hi\n");
  git(repo, "add", "README.md");
  git(repo, "commit", "--quiet", "-m", "init");
  tok("project", "new", "SND", "--repo", repo, "--base", "develop", "--check-command", "echo checked", "--merge", "local", "--sim");
  tok("review-mode", "human", "--project", "SND");
  const chain = join(home, "chain.toml");
  const one = (id) => `\n[[ticket]]\nid = "${id}"\ntitle = "${id}"\nprofile = "sim"\n`;
  writeFileSync(chain, `[chain]\nname = "noisy"\nautostart = true\n${one("a")}${one("b")}${one("c")}`);
  tok("chain", "import", chain, "--project", "SND");

  const merge = async (key) => {
    const ship = shipOf(tok, key);
    writeFileSync(join(ship, `${key}.txt`), "done\n");
    git(ship, "add", `${key}.txt`);
    git(ship, "commit", "--quiet", "-m", key);
    tok("sim", key, "finish", "all done");
    for (const until = Date.now() + 30_000; JSON.parse(tok("show", key, "--json")).review.awaiting?.by === "check"; ) {
      if (Date.now() > until) throw new Error(`${key} stayed in its check turn`);
      await sleep(100);
    }
    tok("accept", key);
    for (const until = Date.now() + 20_000; !tok("show", key).includes("merged"); ) {
      if (Date.now() > until) throw new Error(`${key} did not merge`);
      await sleep(200);
    }
  };

  const { url } = JSON.parse(tok("open", "--json"));
  browser = await launch();
  const context = await browser.newContext({ viewport: { width: 1280, height: 800 } });
  await context.addInitScript(STUB);
  const errors = [];
  const open = async () => {
    const page = await context.newPage();
    page.on("pageerror", (e) => errors.push(e.message));
    await page.goto(url.replace("/?", "/settings/machine?"));
    await page.locator("[data-testid=sounds]").waitFor({ timeout: 10_000 });
    return page;
  };
  const starts = (page) => page.evaluate(() => window.__sounds.starts);
  const check = (ok, what, detail = "") => {
    if (!ok) throw new Error(`${what} ${detail}`);
    console.log(`PASS ${what}`);
  };

  // All three are off at first, in the window and in the daemon.
  let page = await open();
  for (const k of ["story_done", "epic_done", "problem"]) {
    check(!(await page.locator(`[data-testid=sound-${k}]`).isChecked()), `the ${k} switch is off by default`);
  }

  // The switch in the window is saved by the daemon.
  await page.locator("[data-testid=sound-story_done]").check();
  for (const until = Date.now() + 10_000; !JSON.parse(tok("daemon", "set", "--json")).sounds.story_done; ) {
    if (Date.now() > until) throw new Error("the daemon did not take the switch");
    await sleep(100);
  }
  check(JSON.parse(tok("daemon", "set", "--json")).sounds.epic_done === false, "turning one switch on leaves the others off");

  // Switch on: a story that merges (b and c are still open) plays the story sound exactly once.
  check((await starts(page)).length === 0, "control: nothing has played yet");
  await page.locator("[data-testid=sounds]").click(); // the click that lets the page make sound
  await merge("SND-1");
  for (const until = Date.now() + 10_000; (await starts(page)).length < 2; ) {
    if (Date.now() > until) throw new Error(`no sound for the merged story: ${JSON.stringify(await starts(page))}`);
    await sleep(100);
  }
  await sleep(3_000); // a second sound, if the rule played twice, would be here by now
  const played = await starts(page);
  check(JSON.stringify(played) === JSON.stringify([660, 880]), "a merged story plays the story sound once (660 Hz, 880 Hz)", JSON.stringify(played));

  // Switch off: the next story merges and the window stays quiet.
  await page.locator("[data-testid=sound-story_done]").uncheck();
  for (const until = Date.now() + 10_000; JSON.parse(tok("daemon", "set", "--json")).sounds.story_done; ) {
    if (Date.now() > until) throw new Error("the daemon did not take the switch off");
    await sleep(100);
  }
  await merge("SND-2");
  await sleep(4_000); // "nothing happens": a window of the length the sound would have needed
  const quiet = await starts(page);
  check(quiet.length === 2, "with the switch off a merged story plays nothing", JSON.stringify(quiet));

  // Play plays whether or not the switch is on.
  await page.locator("[data-testid=play-epic_done]").click();
  for (const until = Date.now() + 5_000; (await starts(page)).length < 5; ) {
    if (Date.now() > until) throw new Error(`Play did not play: ${JSON.stringify(await starts(page))}`);
    await sleep(50);
  }
  const preview = (await starts(page)).slice(2);
  check(JSON.stringify(preview) === JSON.stringify([523.25, 659.25, 783.99]), "Play plays the epic sound with its switch off", JSON.stringify(preview));

  if (errors.length) throw new Error(`page errors: ${errors.join("; ")}`);
  console.log("PASS no page errors");
} catch (e) {
  console.error(`FAIL ${e.message}`);
  process.exitCode = 1;
} finally {
  await browser?.close();
  await cleanup(tokBin, home);
}
