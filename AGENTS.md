# Agent instructions for Toknila

These rules are for any coding agent working in this repository (Claude Code reads them through
`CLAUDE.md`, Codex and others read `AGENTS.md` directly). Project facts live here; generic
techniques do not.

## What this is

Toknila is a local orchestrator for CLI coding agents: projects are planned as chains of tickets,
each ticket runs on a ship (git worktree + agent runner), and ticket status moves from agent
events. Phase 0 builds the model without real agents: a built-in `sim` profile accepts the same
events phase 1 will get from MCP and hooks, but a human sends them from the CLI.

Never name the reference product this project is modelled on, in code, docs, commits or PRs.
Call it "the reference" if you must refer to it.

## Layout and boundaries

The target layering, the data flow and the UI pattern (MVI) are in `docs/architecture.md`; read it
before moving code between crates. Summary of what holds today:

- `crates/core` (`toknila-core`): domain types and **pure** rules. No I/O, no `tokio`, no
  `rusqlite`/`sqlx`, no `axum`. Every state transition goes through one function in core
  (`apply(ticket, event) -> (ticket, effects)`); a transition outside the table is a typed error.
  `cargo xtask guards` enforces the dependency ban.
- `crates/app` (`toknila-app`): use cases over ports (`Reader`, `UnitOfWork`): the chain unit of
  work, reference resolution, agent events, recovery, `AppError`. Same dependency ban as core;
  the daemon implements the ports over SQLite.
- `crates/api-types` (`toknila-api-types`): what crosses the wire (read models, `ErrorBody`,
  `EventRecord`); `crates/client` (`toknila-client`): the typed client of the API.
- `crates/daemon` (`toknila-daemon`): owns persistence (SQLite, one writer task, every command is
  one transaction "state + events"), HTTP on `127.0.0.1` with a bearer token, WebSocket stream.
- `crates/cli` (`toknila`, binaries `toknila` and `tok`): a thin client of the daemon API. No
  business rules here; if the CLI needs a rule, the rule belongs in core.
- `apps/web`: UI served by the daemon. `apps/desktop`: Tauri, later.
- A production source file (Rust or TypeScript, inline tests excluded) stays at or under 400
  lines; `cargo xtask guards` enforces it. Files already over it may only shrink, and their entry
  in `xtask/src/guards.rs` is lowered or removed in the same PR that shrinks them.
- A rule that two places need moves one layer down; it is never copied.
- Adding a migration: a new file `crates/daemon/src/store/migrations/<slug>.sql` (the slug starts
  with a letter; the card key is a safe choice) whose first line is `-- name: <unique-slug>`.
  Nothing else is edited: a build step collects the files. Never edit a released migration, and
  never number one; start the file name with a letter so it sorts after the legacy `0001_`
  to `0024_` files (a test fails otherwise); a duplicate name stops the daemon at start, naming both files.
- Agent and hook rules: a rule that must hold in every permission mode (the shell-edit refusal)
  lives in the PreToolUse hook (`POST /tickets/{ticket}/pre-tool`, core's
  `question::automatic_decision`); PermissionRequest fires only when Claude Code would ask, so under
  `auto` and `trust` it never sees the call.
- The database is the source of truth. TOML chain files are import/export only. Chain status,
  progress and "where it stopped" are computed, never stored.

## Commands

```sh
cargo xtask verify   # the one Rust gate: fmt --check, clippy -D warnings, tests, guards
cargo xtask web      # the web gate: lint, unit tests, build (apps/web)
cargo xtask e2e-web  # the browser gate: the daemon with the web build, Playwright scripts
cargo xtask guards   # architecture guards only
cargo fmt --all      # fix formatting
```

CI runs `cargo xtask verify` on Ubuntu, Windows and macOS. Do not copy its steps into CI or
scripts; add new checks to `xtask` so local and CI cannot drift.

Running the gates on a developer Mac:

- `cargo xtask e2e-web` uses the browser that is already installed: set
  `PW_CHROMIUM="/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"`. Never run
  `playwright install`; nothing is downloaded for a gate.
- A new browser spec is a new file in `apps/web/e2e` and nothing else: `e2e-web` runs every
  `*.mjs` there (`smoke` first, the rest alphabetical; `helpers.mjs` is a shared module). Put
  `// needs: fake-agent` on one of its first 15 lines when it drives a fake agent. `e2e-web`
  rebuilds the web bundle itself when `apps/web/dist` is older than the sources.
- When `apps/web/node_modules` is missing, run `npm ci --no-audit --no-fund` in `apps/web` first.
  Do not link or copy `node_modules` from another checkout.
- macOS has no `timeout` command; do not wrap commands in it.
- Run one gate at a time: other agents build and test on the same machine. Stop only processes
  you started, by pid; never `pkill -f` or `killall` (that has killed other agents).
- Run a long gate as `cargo xtask verify 2>&1 | tail -n 30`; never redirect its output to a file.
- A failing test is run alone once before any fix: if it passes alone it is a load problem, report
  it. A check failing on the environment (command not found, exit 127, a tool refused) is
  reported in the hand-in and you stop; no environment fix, no empty commit.
- Before the hand-in merge the work branch into the ship, keeping every entry of a shared list.
  A PASS line quotes the gate's result line; the daemon refuses evidence under 20 characters.
- A test that starts a `tok daemon` stops it before it ends, and leaves no process behind.

## Working on a ticket

1. Read the ticket: description, acceptance criteria, dependencies. The ticket is the only source
   of the task; do not invent scope.
2. Write the failing test first where the criterion is testable, then the code.
3. Keep the change to the ticket. Note follow-ups instead of doing them.
4. Cross-platform from the first line: paths via `std::path`/`dirs`, no shell-only assumptions,
   no hard-coded `/tmp`, line endings normalised by `.gitattributes`.
5. One ticket, one branch. Commit on it; a pull request only when the project merges by pull
   request. The hand-in (the `finished` summary, or the PR body) follows the comment template
   below. Never hand in with uncommitted changes, and do not keep editing after handing in.
6. Change files with the Edit and Write tools, not shell edits (python, sed, perl, heredocs):
   tool edits are approved without asking, shell edits are refused.
   Never edit a generated file by hand (`apps/web/src/gen/api.d.ts`, `openapi.json`): run
   `npm run gen` and let `gen:check` prove it.
7. A new dependency is declared once in the workspace `Cargo.toml` under
   `[workspace.dependencies]` and used with `workspace = true`; `cargo add` into a crate is not
   enough. Ask before adding one.
8. When you need a decision (which option, may I install something), ask the person through the
   question tool you were given (`ask_human` under Toknila); a note or a report is never answered.

## Rust conventions

- Edition 2024, stable toolchain. `unsafe` is denied workspace-wide; the only exception is the
  audited Windows FFI in `crates/cli/src/win.rs`. Do not add another without a `SAFETY:` comment.
- Errors: typed (`thiserror`) in libraries, `anyhow` only at binary edges. No `unwrap()` outside
  tests; `expect("why this cannot fail")` when an invariant guarantees it.
- IDs are ULIDs; humans address tickets by short keys like `DEMO-3`.
- Serialisation with `serde`; any JSON a user or another process sees is pinned by a snapshot test.
- A type the store reads back never carries `#[serde(deny_unknown_fields)]`; strictness against
  unknown fields lives in the HTTP body extractor (`serde_ignored`), so an old or newer row with
  an extra field still loads.
- Public items get a one-line doc comment saying what, not how.

## Verification discipline

A check you have never seen fail is a claim, not a check.

- Make every new gate fail once (break what it guards, watch it go red, restore). Mention it in
  the PR under "Gate that was made to fail first".
- Assert the value you expect, not the absence of one you do not.
- Probes need controls: a search that finds nothing proves something only if the same probe finds
  a thing you know is there.
- Print the bytes that cross a boundary (HTTP bodies, event JSON), not the struct you meant to send.
- Run the gate on exactly what you commit; stage the paths you changed, read `git diff --cached`.
- A snapshot moves only when you meant it to: update one test at a time
  (`TOKNILA_UPDATE_SNAPSHOTS=1 cargo test -p <crate> <test>`), read the snapshot diff and keep it
  only if it is the change you intended. Never run the whole workspace with the variable set:
  that rewrites every pinned output and hides regressions.

## Reporting

Keep output short: do not narrate what you are about to do, do not paste code back. Report each
check as **PASS / FAIL / NOT_RUN / BLOCKED**, say what CI does not cover, and mark local-only
evidence as such. Decisions and reports use the ticket comment template:

```markdown
# [instruction | report | decision] <one-line gist>

**What:** what to do or what was done
**Why:** context and reason, one or two sentences
**How to verify:** criterion or command
**Alternatives:** what was rejected and why (decisions)
**Consequences / risks:** what changes next (decisions)
```

Omit empty fields.

## Checklist before saying done

- [ ] `cargo xtask verify` passes locally on the committed tree; `cargo xtask web` and
      `cargo xtask e2e-web` too when `apps/web` changed.
- [ ] Every acceptance criterion of the ticket has a test or a stated manual check.
- [ ] New gates were seen failing once.
- [ ] No business rule outside `crates/core`; core has no I/O dependency.
- [ ] Nothing platform-specific without a Windows path.
- [ ] The hand-in lists checks as PASS / FAIL / NOT_RUN / BLOCKED with one line of evidence each,
      and names what is not covered.
- [ ] Nothing of yours is still running (test daemons, servers), nothing is left uncommitted.
