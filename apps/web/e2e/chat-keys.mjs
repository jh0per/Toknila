// Browser test of a card's Chat and Events tabs, and of the keyboard, against a real daemon (D3).
// Usage: node e2e/chat-keys.mjs <path to tok binary>   (cargo xtask e2e-web runs it)
//
// Chat: one bubble per comment (the agent's report, the person's instruction), the agent's open
// question with its answer box right there (the answer comes back as the person's bubble and the
// card runs again), then a box that sends the agent an instruction. Events: one line in words per
// event, the raw event behind a disclosure. Keyboard: the arrow keys move between the card's tabs
// (and Home and End), "?" opens the list of shortcuts outside text fields only, a dialog keeps the
// focus inside it and gives it back on close. The Settings entry wears a dot while an update waits,
// and keeps it after "Later". SCREENSHOT_DIR, when set, receives pictures.

import { execFileSync } from "node:child_process";
import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { checks, cleanup, launch } from "./helpers.mjs";

const tokBin = process.argv[2];
if (!tokBin) {
  console.error("usage: node e2e/chat-keys.mjs <tok binary>");
  process.exit(2);
}
const home = mkdtempSync(join(tmpdir(), "toknila-chat-keys-"));
const tok = (...args) =>
  execFileSync(tokBin, args, { env: { ...process.env, TOKNILA_HOME: home }, encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] });
const statusOf = (key) => JSON.parse(tok("show", key, "--json")).ticket.status;
const card = (id, title, extra = "") => `\n[[ticket]]\nid = "${id}"\ntitle = "${title}"\nprofile = "sim"\n${extra}`;

let browser;
try {
  tok("project", "new", "CHT");
  const toml = join(home, "work.toml");
  writeFileSync(toml, `[chain]\nname = "work"\nautostart = false\n${card("a", "Login")}${card("b", "Profile", 'after = ["a"]\n')}`);
  tok("chain", "import", toml, "--project", "CHT");
  tok("start", "CHT-1");
  tok("sim", "CHT-1", "start");
  tok("sim", "CHT-1", "note", "Login form drafted");
  tok("comment", "CHT-1", "Use the blue button");
  tok("sim", "CHT-1", "ask", "Archive or delete?");
  const { url } = JSON.parse(tok("open", "--json"));
  const open = (path) => page.goto(url.replace("/?", `${path}?`));

  browser = await launch();
  const page = await browser.newPage({ viewport: { width: 1440, height: 900 }, colorScheme: "light" });
  const errors = [];
  page.on("pageerror", (e) => errors.push(e.message));
  const { expectText } = checks(page);
  const t = (id) => page.locator(`[data-testid=${id}]`);
  const tab = (name) => page.getByRole("tab", { name });
  const must = (ok, what) => {
    if (!ok) throw new Error(what);
    console.log(`PASS ${what}`);
  };
  const shot = async (name) => {
    if (process.env.SCREENSHOT_DIR) await page.screenshot({ path: join(process.env.SCREENSHOT_DIR, `${name}.png`) });
  };
  const focusedInside = (sel) => page.evaluate((s) => Boolean(document.activeElement?.closest(s)), sel);

  // 1. Chat: the bubbles, then the open question with its answer box.
  await open("/projects/CHT/chains/work/cards/CHT-1");
  await t("details").waitFor();
  await tab("Chat").click();
  await t("chat").waitFor();
  await expectText(t("chat-items"), "Login form drafted", "the agent's report is a bubble");
  await expectText(t("chat-items"), "Use the blue button", "the person's instruction is a bubble");
  const sides = await page.locator("[data-testid=bubble]").evaluateAll((els) => els.map((e) => `${e.dataset.side}:${e.dataset.kind}:${e.querySelector(".gist").textContent}`));  must(
    sides.join(",") === "agent:report:Login form drafted,you:instruction:Use the blue button",
    `one bubble per comment, with who said it, and the open question is not repeated as one (${sides})`,
  );
  must((await page.locator("[data-testid=bubble] time").count()) === 2, "each bubble has its time");
  await expectText(t("chat-answer"), "Archive or delete?", "the open question is asked in Chat");
  must((await t("chat-box").count()) === 0, "while a question is open the answer box replaces the message box");
  await shot("chat-question");

  // The answer box is the Inbox's: answering comes back as the person's bubble and the card runs again.
  await page.getByLabel("Reply to CHT-1").fill("Archive them");
  await page.getByRole("button", { name: "Reply" }).click();
  await expectText(t("chat-items"), "Archive them", "the answer is a bubble of the person's");
  for (const until = Date.now() + 10_000; statusOf("CHT-1") !== "running"; await page.waitForTimeout(100)) {
    if (Date.now() > until) throw new Error(`the answer did not send the card on (${statusOf("CHT-1")})`);
  }
  console.log("PASS the answer reached the daemon and the card runs again");
  await t("chat-box").waitFor();
  must((await t("chat-answer").count()) === 0, "the answer box is gone once the question is answered");
  await expectText(t("chat-items"), "Archive or delete?", "the answered question stays in the history");

  // Otherwise a text box sends the agent an instruction.
  await t("chat-input").fill("Thanks, also add a logout button");
  await page.keyboard.press("Enter");
  await expectText(t("chat-items"), "Thanks, also add a logout button", "Enter sends an instruction");
  const comments = JSON.parse(tok("show", "CHT-1", "--json")).comments ?? [];
  must(comments.some((c) => c.kind === "instruction" && c.body.gist.includes("logout")), "the instruction is a comment of the card in the daemon");
  await shot("chat-conversation");
  // "?" typed into the box is a character, not a request.
  await t("chat-input").fill("");
  await t("chat-input").focus();
  await page.keyboard.type("why?");
  must((await t("shortcuts").count()) === 0 && (await t("chat-input").inputValue()) === "why?", "a ? typed in a text field opens nothing");
  await t("chat-input").fill("");

  // 2. Events: words, with the raw event behind a disclosure.
  await tab("Events").click();
  await t("events").waitFor();
  await expectText(t("events"), "Moved from ready to running", "an event reads as a sentence");
  await expectText(t("events"), "Wrote a report: Login form drafted", "so does a report");
  await expectText(t("events"), "Wrote an instruction: Archive them", "so does the answer to the question");
  const text = await t("events").innerText();
  must(!/ticket\.status_changed|comment\.added|\{"/.test(text), "no raw journal name or JSON shows by default");
  const raw = t("event-raw").first();
  must(!(await raw.evaluate((d) => d.open)), "the raw event starts closed");
  await raw.locator("summary").click();
  await expectText(raw, "ticket.", "the disclosure shows the raw event");
  await shot("events");

  // 3. Arrow keys move between the card's tabs.
  await tab("Details").click();
  await tab("Details").focus();
  const selected = async () => page.locator("[role=tab][aria-selected=true]").innerText();
  await page.keyboard.press("ArrowRight");
  must((await selected()) === "Events" && (await focusedInside("[role=tab][aria-selected=true]")), "ArrowRight selects the next tab and moves the focus");
  await expectText(t("events"), "Moved from", "the panel shows the tab");
  await page.keyboard.press("ArrowRight");
  must((await selected()) === "Chat", "ArrowRight again");
  await page.keyboard.press("ArrowLeft");
  must((await selected()) === "Events", "ArrowLeft goes back");
  await page.keyboard.press("End");
  const tabs = await page.locator("[role=tab]").allInnerTexts();
  must((await selected()) === tabs[tabs.length - 1], `End goes to the last tab (${tabs.join(", ")})`);
  await page.keyboard.press("ArrowRight");
  must((await selected()) === "Details", "the arrows wrap round");
  await page.keyboard.press("ArrowLeft");
  must((await selected()) === tabs[tabs.length - 1], "and wrap back");
  await page.keyboard.press("Home");
  must((await selected()) === "Details", "Home goes to the first tab");
  must((await page.locator("[role=tab][tabindex='0']").count()) === 1, "only the selected tab is in the Tab order");

  // 4. "?" opens the shortcuts, outside text fields only.
  await page.locator("body").click({ position: { x: 700, y: 30 } });
  await page.keyboard.press("?");
  await t("shortcuts").waitFor();
  const rows = await page.locator("[data-testid=shortcut]").evaluateAll((els) => els.map((e) => e.querySelector("dt").innerText.replace(/\s+/g, " ").trim()));
  must(rows.join("|") === "N|S|Enter|Delete|Esc|Cmd/Ctrl + ,|?", `the popover lists the keys (${rows.join("|")})`);
  await shot("shortcuts");
  must(await focusedInside("[data-testid=shortcuts]"), "the focus moved into the popover");
  for (let i = 0; i < 4; i++) {
    await page.keyboard.press("Tab");
    if (!(await focusedInside("[data-testid=shortcuts]"))) throw new Error("Tab left the popover");
  }
  for (let i = 0; i < 4; i++) {
    await page.keyboard.press("Shift+Tab");
    if (!(await focusedInside("[data-testid=shortcuts]"))) throw new Error("Shift+Tab left the popover");
  }
  console.log("PASS Tab and Shift+Tab stay inside the popover");
  await page.keyboard.press("?");
  await t("shortcuts").waitFor({ state: "detached" });
  console.log("PASS ? closes it again");
  await t("shortcuts-link").click();
  await t("shortcuts").waitFor();
  await page.keyboard.press("Escape");
  await t("shortcuts").waitFor({ state: "detached" });
  must(await page.evaluate(() => document.activeElement?.getAttribute("data-testid") === "shortcuts-link"), "Escape closes it and the focus returns to what opened it");

  // 5. A dialog keeps the focus inside it and gives it back.
  await t("new-card").focus();
  await t("new-card").press("Enter");
  await t("new-card-dialog").waitFor();
  must(await focusedInside("[data-testid=new-card-dialog]"), "a dialog takes the focus when it opens");
  for (let i = 0; i < 14; i++) {
    await page.keyboard.press("Tab");
    if (!(await focusedInside("[data-testid=new-card-dialog]"))) throw new Error(`Tab left the dialog on press ${i + 1}`);
  }
  console.log("PASS Tab cycles inside the dialog");
  await page.keyboard.press("Shift+Tab");
  must(await focusedInside("[data-testid=new-card-dialog]"), "so does Shift+Tab");
  await page.keyboard.press("Escape");
  await t("new-card-dialog").waitFor({ state: "detached" });
  must(await page.evaluate(() => document.activeElement?.getAttribute("data-testid") === "new-card"), "closing the dialog returns the focus to the button that opened it");

  // 6. The Settings dot while an update waits.
  must((await t("settings-dot").count()) === 0 && (await t("update-banner").count()) === 0, "no update, no dot");
  await page.addInitScript(() => {
    window.__TAURI__ = {
      core: {
        invoke: async (c) =>
          c === "update_status"
            ? { current_version: "0.2.4", last_check: null, busy: false, banner: { title: "Toknila 0.2.5 is ready", notes: "A better diagram." }, error: null }
            : {},
      },
    };
  });
  await open("/projects/CHT/chains/work/cards/CHT-1");
  await t("update-banner").waitFor();
  await t("settings-dot").waitFor();
  must((await t("settings-dot").getAttribute("aria-label")) === "An update is ready", "the Settings entry wears a dot while an update waits");
  await shot("update-dot");
  await t("update-later").click();
  await t("update-banner").waitFor({ state: "detached" });
  must((await t("settings-dot").count()) === 1, "Later hides the banner and keeps the dot");

  if (errors.length) throw new Error(`the page threw: ${errors.join("; ")}`);
  console.log("web chat and keyboard: PASS");
} catch (e) {
  console.error(`web chat and keyboard: FAIL: ${e.message.slice(0, 600)}`);
  process.exitCode = 1;
} finally {
  await browser?.close();
  await cleanup(tok, home);
}
