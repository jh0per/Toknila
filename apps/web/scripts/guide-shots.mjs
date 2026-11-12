// Takes the pictures of the Guide page from the real window, in light and dark, and writes them to
// apps/web/src/assets/guide/<section>-<scheme>.png.
// Usage: node scripts/guide-shots.mjs <path to tok binary>   (cargo xtask guide-shots runs it)
//
// A throwaway daemon (a temporary TOKNILA_HOME, the workspace's fake agent behind a real-agent
// profile so no simulated-agent banner shows, a repository under a short path) holds
// example data only: one project with a card waiting for review, a question, a blocked card, a
// proposal and a finished chain that can be released. Nothing of the person's is read, and the
// page loads nothing from the network. Commit the changed pictures with the change that moved a screen.

import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { checks, cleanup, launch, shipOf } from "../e2e/helpers.mjs";

const [tokBin, fakeAgent] = process.argv.slice(2).map((p) => (p ? resolve(p) : p));
if (!tokBin || !fakeAgent) {
  console.error("usage: node scripts/guide-shots.mjs <tok binary> <fake-agent binary>");
  process.exit(2);
}
// A short folder the field of the Add-a-project picture reads like a person's own.
const shotsRoot = process.platform === "win32" ? join(tmpdir(), "toknila-guide") : "/tmp/toknila-guide";
rmSync(shotsRoot, { recursive: true, force: true });
mkdirSync(shotsRoot, { recursive: true });
const out = resolve(import.meta.dirname, "../src/assets/guide");
const home = mkdtempSync(join(tmpdir(), "toknila-guide-"));
const tok = (...args) => execFileSync(tokBin, args, { env: { ...process.env, TOKNILA_HOME: home }, encoding: "utf8" });
const git = (cwd, ...args) =>
  execFileSync("git", ["-c", "user.name=t", "-c", "user.email=t@example.com", "-c", "commit.gpgsign=false", ...args], { cwd, encoding: "utf8" }).trim();
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const until = async (what, ok) => {
  for (const end = Date.now() + 30_000; !ok(); await sleep(150)) if (Date.now() > end) throw new Error(`timed out: ${what}`);
};
const chainFile = (name, version, cards) =>
  `[chain]\nname = "${name}"\nautostart = true\nversion = "${version}"\n` +
  cards.map((c) => `\n[[ticket]]\nid = "${c.id}"\ntitle = "${c.title}"\nprofile = "claude-code"\n${c.after ? `after = ["${c.after}"]\n` : ""}`).join("");

let browser;
try {
  const repo = join(shotsRoot, "example-app");
  mkdirSync(repo);
  // A user profile named like a real agent's, running the fake agent: it holds, as a working agent does.
  mkdirSync(join(home, "profiles"), { recursive: true });
  writeFileSync(join(home, "profiles", "claude-code.toml"), `command = ['${fakeAgent}', 'print', 'working', 'hold']\n`);
  git(repo, "init", "--quiet");
  git(repo, "symbolic-ref", "HEAD", "refs/heads/main");
  writeFileSync(join(repo, "README.md"), "Example app\n");
  git(repo, "add", "README.md");
  git(repo, "commit", "--quiet", "-m", "init");

  const { url } = JSON.parse(tok("open", "--json"));
  const at = (path) => url.replace("/?", `${path}?`);
  browser = await launch();
  const page = await browser.newPage({ viewport: { width: 1180, height: 820 } });
  page.on("pageerror", (e) => { throw e; });
  const { expectText } = checks(page);
  const t = (id) => page.locator(`[data-testid=${id}]`);
  mkdirSync(out, { recursive: true });
  /// A picture in both schemes; `target` is the page or one element.
  const shoot = async (name, target = page) => {
    for (const scheme of ["light", "dark"]) {
      await page.emulateMedia({ colorScheme: scheme });
      await target.screenshot({ path: join(out, `${name}-${scheme}.png`) });
    }
    await page.emulateMedia({ colorScheme: null });
    console.log(`PASS ${name}`);
  };

  // 1. The form, filled in by the daemon from a real folder (no project exists yet).
  await page.goto(at("/"));
  await t("repo-path").fill(repo);
  await t("work-branch").waitFor({ timeout: 10_000 });
  await t("add-details").locator("summary").click();
  await page.waitForFunction(() => document.querySelector("[data-testid=new-check-command]")?.offsetParent !== null);
  await shoot("add-project", t("add-project"));

  // The project and its example cards, after the picture: the form is not submitted by the script.
  tok("project", "new", "APP", "--repo", repo, "--base", "main", "--check-command", "echo checked", "--merge", "local", "--profile", "claude-code");
  tok("review-mode", "human", "--project", "APP");
  const chains = {
    shipped: ["0.1.0", [{ id: "ship", title: "Sign-in page" }]],
    review: ["0.2.0", [{ id: "rev", title: "Password reset" }]],
    auth: ["0.3.0", [{ id: "asks", title: "Choose the database" }, { id: "blocks", title: "Send the welcome mail" }, { id: "later", title: "Session timeout", after: "asks" }]],
  };
  for (const [name, [version, cards]] of Object.entries(chains)) {
    writeFileSync(join(home, `${name}.toml`), chainFile(name, version, cards));
    tok("chain", "import", join(home, `${name}.toml`), "--project", "APP");
  }
  const status = (key) => JSON.parse(tok("show", key, "--json")).ticket?.status ?? JSON.parse(tok("show", key, "--json")).status;
  const finish = async (key, file, message) => {
    const ship = shipOf(tok, key);
    writeFileSync(join(ship, file), `${message}\n`);
    git(ship, "add", file);
    git(ship, "commit", "--quiet", "-m", message);
    tok("sim", key, "finish", message);
    await until(`${key} waits for a person`, () => JSON.parse(tok("show", key, "--json")).review?.awaiting?.by === "person");
  };
  await finish("APP-1", "signin.txt", "Sign-in page with email and password");
  tok("accept", "APP-1");
  await until("APP-1 merged", () => tok("show", "APP-1").includes("merged"));
  await finish("APP-2", "reset.txt", "Password reset by email");
  await until("APP-3 runs", () => ["running", "waiting"].includes(status("APP-3")));
  tok("sim", "APP-3", "ask", "Which database should sign-in use?");
  await until("APP-4 runs", () => ["running", "waiting"].includes(status("APP-4")));
  tok("block", "APP-4", "waiting for the mail server key");
  tok("propose", "auth", "Add a logout button", "--prompt", "Nobody planned this.");

  // 2. The designs of the project, with one design in the library.
  await page.goto(at("/projects/APP/settings/designs"));
  await t("designs-drop").waitFor({ timeout: 10_000 });
  await t("designs-drop").evaluate(async (zone) => {
    const c = document.createElement("canvas");
    c.width = 320; c.height = 200;
    const g = c.getContext("2d");
    g.fillStyle = "#f3f5f7"; g.fillRect(0, 0, 320, 200);
    g.fillStyle = "#0b7a8a"; g.fillRect(100, 60, 120, 28); g.fillRect(100, 100, 120, 28);
    g.fillStyle = "#1d242c"; g.fillRect(120, 20, 80, 14);
    const blob = await new Promise((r) => c.toBlob(r, "image/png"));
    const dt = new DataTransfer();
    dt.items.add(new File([blob], "sign-in.png", { type: "image/png" }));
    zone.dispatchEvent(new DragEvent("drop", { dataTransfer: dt, bubbles: true, cancelable: true }));
  });
  await page.locator("[data-testid=design-card][data-name='sign-in.png']").waitFor({ timeout: 10_000 });
  await page.waitForFunction(() => [...document.querySelectorAll("[data-testid=design-thumb]")].every((i) => i.complete && i.naturalWidth > 0));
  await shoot("designs");

  // 3. The assistant's screen: the plan so far and the conversation.
  await page.goto(at("/projects/APP/planner"));
  await t("planner-title").waitFor({ timeout: 10_000 });
  await sleep(500);
  await shoot("plan");

  // 4. A chain with a running card, the agents count and the autostart switch.
  await page.goto(at("/projects/APP/chains/auth"));
  await t("chain-autostart-toggle").waitFor({ timeout: 10_000 });
  await page.getByRole("button", { name: "Fit", exact: true }).click();
  await sleep(500);
  await shoot("start");

  // 5. The inbox.
  await page.goto(at("/projects/APP/inbox"));
  await expectText(page.locator("main"), "Agent questions", "the inbox shows the question");
  await shoot("inbox");

  // 6. A card in review: the hand-in, the rounds, Accept and Return.
  await page.goto(at("/projects/APP/chains/review/cards/APP-2"));
  await t("actions").waitFor({ timeout: 10_000 });
  await t("actions").scrollIntoViewIfNeeded();
  await sleep(500);
  await shoot("review");

  // 7. A finished chain: its version and the Release button at the end of the diagram.
  await page.goto(at("/projects/APP/chains/shipped"));
  await t("release-button").first().waitFor({ timeout: 10_000 });
  await page.getByRole("button", { name: "Fit", exact: true }).click();
  await sleep(500);
  await shoot("release");

  // 8. The sidebar: the project's tree and the foot.
  await page.goto(at("/projects/APP"));
  await t("settings-link").waitFor({ timeout: 10_000 });
  await sleep(300);
  await shoot("where", page.locator("nav.sidebar"));
} catch (e) {
  console.error(`FAIL ${e.message}`);
  process.exitCode = 1;
} finally {
  await browser?.close();
  await cleanup(tok, home);
  rmSync(shotsRoot, { recursive: true, force: true });
}
