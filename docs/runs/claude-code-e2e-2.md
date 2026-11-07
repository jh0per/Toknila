# Claude Code end to end, second run

Ticket P1-27. One card was run by a real Claude Code agent (Sonnet) on a
ship with the **built-in** `claude-code` and `claude-code-reviewer` profiles, unchanged. The aim
was to confirm the fixes of #36 (first prompt, uncommitted-ship guard, per-role run files,
`check_command`, reviewer stopped after its verdict), #37 (role-aware inbox questions, Allow/Deny
through the `PermissionRequest` hook, the "agent waits on its screen" item, Waiting -> Running on
an answer) and #38 (`--permission-mode acceptEdits`). A person (the owner's role, played by the
test driver) did every human action, only through Toknila's own surfaces: the HTTP API (inbox,
answer route, and the input route once, see finding 2) and `tok`.

Result: the card reached **Merged** through a real pull request.

| Card | Title | PR | CI | Merged (UTC) |
|---|---|---|---|---|
| TOK-1 | Usage: Haiku 5.5 long-prompt price tier | [#40](https://github.com/jh0per/Toknila/pull/40) | verify on ubuntu, macos, windows, desktop and the 4 build jobs: pass | 00:30:17 |

## Setup

- Toknila built from `main` at `210623b` (debug build), daemon started with a data home outside
  any git repository (run 1's extra "external CLAUDE.md imports" dialog did not come back).
  `claude` 2.1.319, model `sonnet` through `ANTHROPIC_MODEL` in the daemon's environment (the
  built-in profile has no model flag and was not touched).
- Project repository: a fresh clone of `jh0per/Toknila`. Project `TOK`: merge mode `github`, base
  `main`, parallel limit 1, review mode `both`, `check_command` = `cargo xtask verify`, default
  profile `claude-code` (reviewer `claude-code-reviewer`). One chain, autostart off.
- One shared `CARGO_TARGET_DIR` for the ship builds, outside the repository.
- The daemon was started from a Claude Code session; the runner scrubbed the `CLAUDECODE` and
  `CLAUDE_CODE_*` variables (the sessions showed no "child session" notice).
- The card: add an optional long-prompt price tier to `prices.toml` and fill it for Haiku 5.5 from
  the pricing page; price each call by its own prompt size; core tests on both sides of the
  threshold; update the Usage section of `docs/architecture.md`.
- Permission requests were answered as the person through `POST /v1/tickets/{t}/runner/answer`
  (`allow` / `deny`) by a small script that logged every inbox item before answering and denied
  anything that looked risky (push, installs, network, deleting outside the ship). Nothing was
  denied; nothing risky came.

## Timeline

Local times (UTC+2), 7 November 2026.

| Time | What | Who |
|---|---|---|
| 01:55:58 | project, chain and card created, Ready | person |
| 01:56:15 | Ready -> Running, ship created, runner started (first prompt on the command line) | person (Start), system |
| 01:56:38 | Inbox item "The agent waits on its screen": the folder trust dialog, with its lines | system |
| 01:56:55 to 01:58:56 | tried quick keys `2`, then `1` (nothing happened), then a text answer carrying a down-arrow sequence plus Enter: "Yes, I trust this folder" chosen (finding 2) | person |
| 01:59:46 | agent started by itself and worked; first permission request (Bash, a script editing `price.rs`) | agent |
| 01:59:47 | Allow; card Waiting -> Running at once | person, system |
| 02:01:16 | second request (Bash: `sed` mutation check plus `cargo test`, restoring with `git checkout -p`) | agent |
| 02:01:18 | Allow | person |
| 02:11:27 | third request (Bash: backup copy of the file to `/tmp` and the scratchpad, `rm -f` of the backup, `perl -pi`) | agent |
| 02:11:28 | Allow (a copy of its own file; judged harmless) | person |
| 02:12:33 | agent committed `2a56715` itself, then `finished`: Running -> Review; reviewer runner started | agent, system |
| 02:13 | reviewer ran `cargo xtask verify` without asking (2 tests failed, see finding 3), then stopped at its prompt without a verdict, naming what it lacked (finding 1) | reviewer |
| 02:16:55 | after a note typed into the reviewer's terminal, reviewer recorded its verdict: **approve**; runner exited one second later | person, reviewer, system |
| 02:17:17 | **Accept** (person); push, PR #40 opened 02:17:21 | person, system |
| 02:30:17 | CI green (the Windows `verify` job decided the time); PR merged by `gh`, card Merged | system |

The person checked the card's output before accepting: the Haiku 5.5 rows (0.50 / 0.625 / 0.05 /
2.50 USD per million tokens for input / cache write / cache read / output over 100,000) and the
rule that the prompt counts input, cache writes and cache reads (output not counted) match the
pricing page; costing runs per message, so each call is priced by its own prompt. No Return was
needed. About 17 minutes passed from Start to Accept; the agent worked about 16 of them.

The card's Usage block showed numbers while it ran and after: 18 messages, 1.02 M tokens
processed, 90.7 % cached, an estimated 0.37 USD.

## Permission log

Three requests, all from the worker, all kind Bash; every file edit went through without a
prompt, and the reviewer asked for nothing. Each was logged from the Inbox item before it was
answered.

| # | Time | Tool | Summary the Inbox showed | Answer |
|---|---|---|---|---|
| 1 | 01:59:46 | Bash | `python3 - <<'EOF'` script that rewrites `crates/core/src/usage/price.rs` (add the tier and the rates) | Allow |
| 2 | 02:01:16 | Bash | `sed -i` flips a comparison in `price.rs`, runs `cargo test -p toknila-core long_prompt`, restores with `git checkout -p --` (a mutation check) | Allow (inside the ship; run 1 held the same shape for a person) |
| 3 | 02:11:27 | Bash | `cp price.rs /tmp/price.bak`, copy to the session scratchpad, `rm -f /tmp/price.bak`, `perl -pi` | Allow (its own backup; a stricter script now denies copies to `/tmp`) |

By kind: Bash 3, file edit 0, other tools 0 (reviewer 0). Run 1 had 19 for two cards in 10 minutes.

## Run 1 findings, one by one

| # | Run 1 finding | Run 2 |
|---|---|---|
| 1 | The agent is never told to begin (worker and reviewer) | **Fixed.** Worker and reviewer started by themselves with the first prompt. The reviewer, however, ended its turn without a verdict and nothing woke it (new finding 1). |
| 2 | A reviewer's permission prompt has no answer path | **Partly.** The route and the inbox take `role=reviewer` now, but the built-in reviewer runs `dontAsk`, so it never asked: it was denied and wrote that to the terminal, where nothing showed it. The need moved from "answer a prompt" to "tell the person the reviewer is stuck" (new finding 1). |
| 3 | Nothing tells the worker to commit, Accept does not notice | **Fixed in part, guard not exercised.** The agent committed by itself before `finished`; the refusal of `finished` without a commit did not trigger, so it is not confirmed with a real session. |
| 4 | New ships start with dialogs Toknila cannot see | **Half fixed.** The trust dialog now raises an Inbox item with its lines, but the quick keys cannot choose its "Yes" option (new finding 2). The external-imports dialog did not appear (data home outside a repository). |
| 5 | The permission item carries no content | **Fixed.** The item names the tool and the command ("Claude Code asks for permission: Bash: ..."), cut to a cap. |
| 6 | A card stays Waiting after the person said yes | **Fixed.** Waiting -> Running at the moment of the answer (01:59:47, 02:01:18, 02:11:28). |
| 7 | A reviewer cannot run the checks | **Fixed for the check command.** `cargo xtask verify` ran without a prompt. It then failed for another reason (new finding 3); pipes, `curl` and web tools are still denied. |
| 8 | The reviewer's process lives until the merge | **Fixed.** The reviewer exited one second after its verdict (journal `runner_exited` 02:16:56). |
| 9 | The agent inherits the caller's environment | **Fixed.** The `CLAUDE_CODE_*` markers were scrubbed. The Toknila role variables are a different case (new finding 3). |
| 10 | Small things: `tok watch --json`, no timestamps in watch, shared `instructions.md` per ticket | **Per-role run files fixed** (`runs/<ticket>/worker` and `reviewer`). `tok watch` still prints no timestamps (the timeline above comes from `/v1/events`); `tok watch --json` was not retried. |

Run 1's other note, the worker's mutation check, came again as permission request 2.

## New findings, ranked by how much they hurt daily use

1. **A reviewer that cannot finish its checks ends its turn without a verdict, and nothing tells
   the person.** The reviewer (`dontAsk`) had Bash and web access denied for the pricing page and
   for printing its environment, wrote "what I need from you" to its terminal and stopped. The
   card said "waiting for the reviewer" for four minutes, the Inbox was empty (no screen item for
   the reviewer, no permission item, no nudge) and the card could not be accepted or returned
   usefully. The way out was raw input into the reviewer's terminal
   (`POST .../runner/input?role=reviewer`), which the window does not offer. Suggested fix: raise
   the screen-wait item for a reviewer that is idle at its prompt without a verdict, and nudge it
   like a worker (or end its run and show "reviewer gave no verdict, Return / Accept / run again").
2. **The folder trust dialog cannot be answered with the quick keys.** The Inbox shows the dialog
   (lines included), but its options are not numbered, `1` and `2` do nothing, and Enter would
   take the preselected "No, exit". The only working answer was a text answer containing a
   down-arrow escape sequence. The item also comes back every 30 seconds while the dialog is on
   screen. Suggested fix: pre-trust the ship folder in the per-run settings, or add arrow keys to
   the quick keys. Every new card hits this; with pre-trust, none would.
3. **The role variables of a reviewer leak into the tests it runs.** `cargo xtask verify` in the
   reviewer's shell failed two `crates/cli/tests/mcp_stdio.rs` tests, because `TOKNILA_ROLE`
   (and `TOKNILA_RUN`) of its own session reached the `tok mcp` server under test, which then
   listed only `review_verdict`. The reviewer could not tell this from a real failure and could
   not clear the variable. Fixed in this PR: the test helper removes both variables (seen failing
   with `TOKNILA_ROLE=reviewer`, passing after).
4. **Permission requests become card comments.** Each request left a `[report]` comment with the
   full command on the card (three here, long here-documents). The card's comment list is
   then mostly commands. Suggested fix: keep them in the journal and the Inbox, not as comments.
5. **The reviewer cannot read the page its card is about.** Under `dontAsk` the web tools are
   denied, so a card that says "from the pricing page" gets unverified numbers from the
   reviewer ("NOT_RUN"). Here the person checked them and said so. A recommended reviewer
   profile with a read-only fetch tool, or a rule that the card names its sources in the
   instructions, would help.
6. **The screen item reappears after each answer.** After the person answers keys, the item is
   cleared and raised again after a pause while the screen did not change (trust dialog,
   01:56:38, 01:57:18, 01:57:48). Harmless once known; confusing when it looks like the answer
   did not arrive.

## What was not exercised

- The refusal of `finished` without a commit (the agent committed on its own).
- A Deny (nothing risky was asked) and a Return round (the change was right).
- A reviewer permission prompt (the reviewer cannot raise one in `dontAsk`).
- Windows and Linux terminals.

## Fixes in this PR

- `crates/cli/tests/mcp_stdio.rs`: the MCP session helper removes `TOKNILA_ROLE` and
  `TOKNILA_RUN` from the child's environment (finding 3).

Everything else is listed as a finding.

## Cleanup

The daemon and every runner were stopped after the run, and the temporary data home, the clone of
the repository and the cargo target directories were deleted.
