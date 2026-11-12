// Browser test of pictures attached to a message for the assistant, against a real daemon and the
// workspace's `fake-agent` as the planner (never a real agent): the fake planner reads a line from
// its terminal and prints it back as `got:<line>`, then answers.
// Usage: node e2e/ask-attach.mjs <tok binary> <fake-agent binary>   (cargo xtask e2e-web runs it)
// needs: fake-agent
//
// A png dropped on the assistant's box is a chip; after Send the line the planner received ends with
// `Attached designs: <name>` and the project's library lists the file; a pasted picture is named from
// the project's key and a message with nothing but an attachment can be sent; a text file is refused
// with the library's sentence and the typed text stays; in a card's Talk to the assistant box a
// pasted picture is named from the card's key and the thread shows its name with the message.

import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { checks, cleanup, launch } from "./helpers.mjs";

const [tokArg, agentArg] = process.argv.slice(2);
if (!tokArg || !agentArg) {
  console.error("usage: node e2e/ask-attach.mjs <tok binary> <fake-agent binary>");
  process.exit(2);
}
const tokBin = resolve(tokArg);
const fakeAgent = resolve(agentArg);
const home = mkdtempSync(join(tmpdir(), "toknila-askattach-"));
const run = (cmd, args) => execFileSync(cmd, args, { encoding: "utf8" });
const tok = (...args) => execFileSync(tokBin, args, { env: { ...process.env, TOKNILA_HOME: home }, encoding: "utf8" });
const git = (dir, ...args) =>
  run("git", ["-C", dir, "-c", "user.name=t", "-c", "user.email=t@example.com", "-c", "commit.gpgsign=false", ...args]);
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

// A 1x1 png.
const PNG = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/q842iQAAAABJRU5ErkJggg==";

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
  const say = ["post", "/planner/say", JSON.stringify({ text: "Seen." })];
  const reply = ["post", "/planner/cards/AT-1/threads/1/messages", JSON.stringify({ text: "Looked at it" })];
  writeFileSync(
    join(home, "profiles", "fake-planner.toml"),
    `role = 'planner'\ncommand = ['${fakeAgent}', ${steps("print", "planner ready", "echo", ...say, "echo", ...say, "echo", ...say, "echo", ...reply, "hold")}]\nstartup_secs = 0\n`,
  );
  tok("project", "new", "AT", "--repo", repo, "--base", "develop");
  tok("chain", "new", "work", "--project", "AT", "--no-autostart");
  tok("ticket", "new", "work", "Login screen", "--prompt", "p", "--accept", "a");
  tok("planner", "start", "AT", "--profile", "fake-planner");
  const { url } = JSON.parse(tok("open", "--json"));
  const origin = new URL(url).origin;
  const bearer = { authorization: `Bearer ${new URL(url).searchParams.get("token")}` };
  const at = (path) => {
    const u = new URL(url);
    u.pathname = path;
    return u.href;
  };
  const scrollback = async () => (await fetch(`${origin}/v1/projects/AT/planner/scrollback`, { headers: bearer })).text();
  const library = async () => (await (await fetch(`${origin}/v1/projects/AT/designs`, { headers: bearer })).json()).designs.map((d) => d.name);

  browser = await launch();
  const page = await browser.newPage({ viewport: { width: 1280, height: 1000 } });
  const errors = [];
  page.on("pageerror", (e) => errors.push(e.message));
  const { expectText } = checks(page);
  const t = (id) => page.locator(`[data-testid=${id}]`);

  // Files for a drop or a paste, built in the page the way a file manager or a screenshot tool gives them.
  const dataTransfer = (files) =>
    page.evaluateHandle(
      ({ files, b64 }) => {
        const bytes = Uint8Array.from(atob(b64), (c) => c.charCodeAt(0));
        const dt = new DataTransfer();
        for (const f of files) dt.items.add(f.text ? new File([f.text], f.name, { type: f.type }) : new File([bytes], f.name, { type: f.type }));
        return dt;
      },
      { files, b64: PNG },
    );
  const drop = async (box, files) => {
    const dt = await dataTransfer(files);
    await t(box).evaluate((el, dt) => {
      el.dispatchEvent(new DragEvent("dragover", { dataTransfer: dt, bubbles: true, cancelable: true }));
      el.dispatchEvent(new DragEvent("drop", { dataTransfer: dt, bubbles: true, cancelable: true }));
    }, dt);
  };
  const paste = async (field, files) => {
    const dt = await dataTransfer(files);
    await t(field).evaluate((el, dt) => el.dispatchEvent(new ClipboardEvent("paste", { clipboardData: dt, bubbles: true, cancelable: true })), dt);
  };
  const heard = async (text, what) => {
    for (const stop = Date.now() + 15_000; Date.now() < stop; await sleep(150)) {
      if ((await scrollback()).includes(text)) return;
    }
    throw new Error(`${what}: the planner did not receive "${text}": ${(await scrollback()).slice(-300)}`);
  };

  // 1. A png dropped on the assistant's box is a chip; Send puts the line last; the library has it.
  await page.goto(at("/projects/AT/planner"));
  await t("planner-ask-input").waitFor({ timeout: 10_000 });
  await drop("planner-ask", [{ name: "checkout-ref.png", type: "image/png" }]);
  await expectText(t("planner-ask-chip"), "checkout-ref.png", "the dropped png is a chip");
  if ((await t("planner-ask-chip").locator("img").count()) !== 1) throw new Error("the chip has no thumbnail");
  await t("planner-ask-input").fill("make it look like this");
  await t("planner-ask-send").click();
  await heard("got:make it look like this Attached designs: checkout-ref.png", "dropped png");
  if (!(await library()).includes("checkout-ref.png")) throw new Error(`the library lists ${(await library()).join(", ")}`);
  console.log("PASS a png dropped on the assistant's box is a chip; the planner's line ends with the attachment; the library lists it");
  await t("planner-ask-chip").waitFor({ state: "detached" });
  await expectText(t("chat-items"), "Attached designs: checkout-ref.png", "the record names the attachment");

  // 2. A pasted picture is named from the project's key; a message with only an attachment is sent.
  await t("planner-ask-input").waitFor({ state: "visible" });
  await page.waitForFunction(() => !document.querySelector("[data-testid=planner-ask-input]")?.disabled, null, { timeout: 15_000 });
  await paste("planner-ask-input", [{ name: "image.png", type: "image/png" }]);
  await expectText(t("planner-ask-chip"), "AT-1.png", "a pasted picture is named from the project's key");
  await t("planner-ask-send").click();
  await heard("got:Attached designs: AT-1.png", "attachment only");
  console.log("PASS a pasted picture is named from the project's key and a message with only an attachment is sent");

  // 3. A refused file shows the library's sentence; the typed text stays; a chip comes off.
  await page.waitForFunction(() => !document.querySelector("[data-testid=planner-ask-input]")?.disabled, null, { timeout: 15_000 });
  await t("planner-ask-input").fill("keep this text");
  await drop("planner-ask", [{ name: "notes.txt", type: "text/plain", text: "hello" }]);
  await expectText(t("planner-ask-refused"), "notes.txt: only png, jpg, webp and pdf files can be designs", "the refusal");
  if ((await t("planner-ask-input").inputValue()) !== "keep this text") throw new Error("the typed text was lost");
  await drop("planner-ask", [{ name: "other.png", type: "image/png" }]);
  await expectText(t("planner-ask-chip"), "other.png", "a png after the refusal");
  await t("planner-ask-chip-remove").click();
  if ((await t("planner-ask-chip").count()) !== 0) throw new Error("the chip stayed after its remove");
  if (!(await library()).includes("other.png")) throw new Error("taking a chip off removed the design from the library");
  console.log("PASS a refused file shows the library's sentence and keeps the text; taking a chip off keeps the design");
  await t("planner-ask-input").fill("");

  // 4. The panel beside a card (the card's chip on): a pasted picture is a chip of the panel's box
  //    and the card's discussion names it with the message.
  await page.goto(at("/projects/AT/chains/work/cards/AT-1"));
  await t("assistant-toggle").click();
  await expectText(t("context-chip"), "Card AT-1", "the panel's chip is the open card");
  await t("planner-ask-input").waitFor({ timeout: 10_000 });
  await paste("planner-ask-input", [{ name: "image.png", type: "image/png" }]);
  await t("planner-ask-chip").waitFor({ timeout: 10_000 });
  const pasted = (await t("planner-ask-chip").first().textContent()).match(/AT-\d+\.png/)?.[0];
  if (!pasted) throw new Error("the pasted picture is not named from the project's key");
  await t("planner-ask-input").fill("move the button as in the picture");
  await t("planner-ask-send").click();
  await expectText(t("chat-items"), `Attached designs: ${pasted}`, "the record names the attachment with the message");
  await expectText(t("chat-items"), "move the button as in the picture", "and the message");
  await heard(`(thread 1): move the button as in the picture Attached designs: ${pasted}`, "card message");
  console.log("PASS with the card chip the panel's discussion names the attached picture with the message");

  if (errors.length) throw new Error(`the page threw: ${errors.join("; ")}`);
  console.log("web ask attach: PASS");
} catch (e) {
  console.error(`web ask attach: FAIL: ${e.message.slice(0, 800)}`);
  process.exitCode = 1;
} finally {
  await browser?.close();
  await cleanup(tok, home);
}
