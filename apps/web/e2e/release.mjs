// Browser test of the Release dialog against a real daemon.
// Usage: node e2e/release.mjs <path to tok binary>   (cargo xtask e2e-web runs it)
//
// A chain with one card is finished and merged in a temporary git repository whose project has a
// check command. The Finish node offers Release; its dialog suggests 0.1.0 and lists the steps;
// Release runs the check and the tag, the dialog reads released, and the Finish node reads
// released with the tag v0.1.0, which is a real tag of the repository. The picture of the
// finished dialog goes to docs/runs/release.png.

import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { checks, cleanup, launch, shipOf } from "./helpers.mjs";

const tokBin = process.argv[2];
if (!tokBin) {
  console.error("usage: node e2e/release.mjs <tok binary>");
  process.exit(2);
}
const home = mkdtempSync(join(tmpdir(), "toknila-release-"));
const tok = (...args) =>
  execFileSync(tokBin, args, { env: { ...process.env, TOKNILA_HOME: home }, encoding: "utf8" });
const git = (cwd, ...args) =>
  execFileSync("git", ["-c", "user.name=t", "-c", "user.email=t@example.com", "-c", "commit.gpgsign=false", ...args], {
    cwd,
    encoding: "utf8",
  }).trim();
const shot = join(dirname(fileURLToPath(import.meta.url)), "../../../docs/runs/release.png");

let browser;
try {
  const repo = join(home, "repo");
  mkdirSync(repo);
  git(repo, "init", "--quiet");
  git(repo, "symbolic-ref", "HEAD", "refs/heads/develop");
  writeFileSync(join(repo, "README.md"), "hi\n");
  git(repo, "add", "README.md");
  git(repo, "commit", "--quiet", "-m", "init");
  tok("project", "new", "REL", "--repo", repo, "--base", "develop", "--check-command", "echo release-checked", "--merge", "local");
  tok("review-mode", "human", "--project", "REL");
  const chain = join(home, "chain.toml");
  writeFileSync(chain, `[chain]\nname = "shipped"\nautostart = true\n\n[[ticket]]\nid = "work"\ntitle = "work"\nprofile = "sim"\n`);
  tok("chain", "import", chain, "--project", "REL");
  const ship = shipOf(tok, "REL-1");
  writeFileSync(join(ship, "done.txt"), "done\n");
  git(ship, "add", "done.txt");
  git(ship, "commit", "--quiet", "-m", "done");
  tok("sim", "REL-1", "finish", "all done");
  for (const until = Date.now() + 30_000; JSON.parse(tok("show", "REL-1", "--json")).review.awaiting?.by === "check"; ) {
    if (Date.now() > until) throw new Error("REL-1 stayed in its check turn");
    await new Promise((r) => setTimeout(r, 100));
  }
  tok("accept", "REL-1");
  for (const until = Date.now() + 20_000; !tok("show", "REL-1").includes("merged"); ) {
    if (Date.now() > until) throw new Error("REL-1 did not merge");
    await new Promise((r) => setTimeout(r, 200));
  }
  console.log("PASS the chain is finished: its card is merged");

  const { url } = JSON.parse(tok("open", "--json"));
  browser = await launch();
  const page = await browser.newPage({ viewport: { width: 1280, height: 800 } });
  const errors = [];
  page.on("pageerror", (e) => errors.push(e.message));
  const { expectText } = checks(page);
  const t = (id) => page.locator(`[data-testid=${id}]`);

  await page.goto(url.replace("/?", "/projects/REL/chains/shipped?"));
  await t("release-button").first().click();
  await t("release-dialog").waitFor({ timeout: 10_000 });
  await page.waitForFunction(() => document.querySelector("[data-testid=release-version]")?.value === "0.1.0", null, { timeout: 10_000 }).catch(() => {
    throw new Error("the Release dialog did not suggest 0.1.0");
  });
  console.log("PASS the Release dialog suggests 0.1.0");
  await expectText(t("release-steps"), "Check", "the dialog lists the check step");
  await expectText(t("release-steps"), "Tag", "the dialog lists the tag step");
  if (git(repo, "tag", "--list") !== "") throw new Error("a tag was made before the click");
  console.log("PASS nothing is tagged before the click");

  await t("release-confirm").click();
  await t("release-done").waitFor({ timeout: 60_000 });
  await expectText(t("release-done"), "v0.1.0", "the dialog reads released with the tag");
  await expectText(t("release-steps").locator("[data-step=check]"), "release-checked", "the check's output is shown");
  if (git(repo, "tag", "--list") !== "v0.1.0") throw new Error(`the repository's tags are: ${git(repo, "tag", "--list")}`);
  console.log("PASS the repository has the tag v0.1.0");
  mkdirSync(dirname(shot), { recursive: true });
  await page.screenshot({ path: shot });

  await t("release-close").click();
  await expectText(t("finish-state"), "released", "the Finish node reads released");
  await expectText(t("end-node-text"), "v0.1.0", "the Finish node shows the tag");
  if (errors.length) throw new Error(`page errors: ${errors.join("; ")}`);
  console.log("PASS no page errors");
} catch (e) {
  console.error(`FAIL ${e.message}`);
  process.exitCode = 1;
} finally {
  await browser?.close();
  await cleanup(tok, home);
}
