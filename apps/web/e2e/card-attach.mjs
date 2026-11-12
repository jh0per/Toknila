// Browser test of attaching files to a card, against a real daemon.
// Usage: node e2e/card-attach.mjs <path to tok binary>   (cargo xtask e2e-web runs it)
//
// A png dropped on a card's Designs section is a mockup there and a design of the project's
// library; an image pasted on the page is named from the card's key; a text file dropped with a png
// is refused with the library's sentence while the png is attached; a started card shows no drop
// target and no picker.

import { execFileSync } from "node:child_process";
import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { checks, cleanup, launch } from "./helpers.mjs";

const tokBin = process.argv[2];
if (!tokBin) {
  console.error("usage: node e2e/card-attach.mjs <tok binary>");
  process.exit(2);
}
const home = mkdtempSync(join(tmpdir(), "toknila-cattach-"));
const tok = (...args) =>
  execFileSync(tokBin, args, { env: { ...process.env, TOKNILA_HOME: home }, encoding: "utf8" });

// A 1x1 png.
const PNG = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/q842iQAAAABJRU5ErkJggg==";

let browser;
try {
  tok("project", "new", "ATT", "--sim");
  writeFileSync(
    join(home, "c.toml"),
    `[chain]\nname = "work"\nautostart = false\n\n[[ticket]]\nid = "A"\ntitle = "Login screen"\nprofile = "sim"\n\n[[ticket]]\nid = "B"\ntitle = "Started one"\nprofile = "sim"\n`,
  );
  tok("chain", "import", join(home, "c.toml"), "--project", "ATT");
  const { url } = JSON.parse(tok("open", "--json"));
  const at = (path) => url.replace("/?", `${path}?`);

  browser = await launch();
  const page = await browser.newPage({ viewport: { width: 1280, height: 900 } });
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
        for (const f of files) {
          dt.items.add(f.text ? new File([f.text], f.name, { type: f.type }) : new File([bytes], f.name, { type: f.type }));
        }
        return dt;
      },
      { files, b64: PNG },
    );
  const drop = async (files) => {
    const dt = await dataTransfer(files);
    await t("card-designs").evaluate((el, dt) => {
      el.dispatchEvent(new DragEvent("dragover", { dataTransfer: dt, bubbles: true, cancelable: true }));
      el.dispatchEvent(new DragEvent("drop", { dataTransfer: dt, bubbles: true, cancelable: true }));
    }, dt);
  };
  const paste = async (files) => {
    const dt = await dataTransfer(files);
    await page.evaluate((dt) => {
      document.body.dispatchEvent(new ClipboardEvent("paste", { clipboardData: dt, bubbles: true, cancelable: true }));
    }, dt);
  };
  const library = async () => {
    await page.goto(at("/projects/ATT/settings/designs"));
    await t("designs-drop").waitFor({ timeout: 10_000 });
    await page.waitForSelector("[data-testid=design-card]", { timeout: 10_000 });
    return page.locator("[data-testid=design-card]").evaluateAll((els) => els.map((e) => e.getAttribute("data-name")));
  };

  await page.goto(at("/projects/ATT/chains/work/cards/ATT-1"));
  await t("card-designs-drop").waitFor({ timeout: 10_000 });
  console.log("PASS an editable card without designs shows the drop hint and the picker");

  await drop([{ name: "home.png", type: "image/png" }]);
  await t("mockup-image").waitFor({ timeout: 10_000 });
  await t("card-designs-drop").waitFor({ state: "detached" });
  if ((await t("card-design").count()) !== 1) throw new Error("the dropped png is not one mockup");
  const names = await library();
  if (!names.includes("home.png")) throw new Error(`the library lists ${names.join(", ")}`);
  console.log("PASS a dropped png is a mockup on the card and a design of the library");

  await page.goto(at("/projects/ATT/chains/work/cards/ATT-1"));
  await t("card-designs-upload").waitFor({ timeout: 10_000 });
  await paste([{ name: "image.png", type: "image/png" }]);
  await page.waitForFunction(() => document.querySelectorAll("[data-testid=card-design]").length === 2, undefined, { timeout: 10_000 });
  const afterPaste = await library();
  if (!afterPaste.includes("ATT-1-1.png")) throw new Error(`the pasted image is named ${afterPaste.join(", ")}`);
  console.log("PASS a pasted image is attached under a name made from the card's key");

  await page.goto(at("/projects/ATT/chains/work/cards/ATT-1"));
  await t("card-designs-upload").waitFor({ timeout: 10_000 });
  await drop([
    { name: "notes.txt", type: "text/plain", text: "hello" },
    { name: "second.png", type: "image/png" },
  ]);
  await expectText(t("card-designs-refused"), "notes.txt: only png, jpg, webp and pdf files can be designs", "the refusal is shown");
  await page.waitForFunction(() => document.querySelectorAll("[data-testid=card-design]").length === 3, undefined, { timeout: 10_000 });
  console.log("PASS a text file is refused with the library's sentence and the png beside it is attached");
  if (process.env.SCREENSHOT_DIR) await page.screenshot({ path: join(process.env.SCREENSHOT_DIR, "card-attach.png") });

  // A card the daemon no longer lets be edited (can_edit false) keeps its designs but offers no
  // drop target, no picker and no paste.
  await page.goto(at("/projects/ATT/chains/work/cards/ATT-2"));
  await t("card-designs-upload").waitFor({ timeout: 10_000 });
  await drop([{ name: "own.png", type: "image/png" }]);
  await t("card-design").waitFor({ timeout: 10_000 });
  tok("cancel", "ATT-2", "not needed");
  await page.goto(at("/projects/ATT/chains/work/cards/ATT-2"));
  await t("card-design").waitFor({ timeout: 10_000 });
  await page.waitForTimeout(1500);
  await paste([{ name: "image.png", type: "image/png" }]);
  await page.waitForTimeout(1000);
  for (const id of ["card-designs-drop", "card-designs-upload", "card-designs-file-input", "add-design"]) {
    if ((await t(id).count()) !== 0) throw new Error(`a card that cannot be edited shows ${id}`);
  }
  if ((await t("card-design").count()) !== 1) throw new Error("a paste changed the designs of a card that cannot be edited");
  console.log("PASS a card the daemon does not let be edited shows no drop target and no picker");

  if (errors.length) throw new Error(`the page threw: ${errors.join("; ")}`);
  console.log("web card attach: PASS");
} catch (e) {
  console.error(`web card attach: FAIL: ${e.message.slice(0, 800)}`);
  process.exitCode = 1;
} finally {
  await browser?.close();
  await cleanup(tok, home);
}
