# Claude Code end to end, first real run

Plan ticket `claude-code` (P1-23). Two cards were run by real Claude Code agents (Sonnet) in
parallel on ships, reported through MCP and hooks, went through review with one Return round, and
were merged by Toknila through GitHub pull requests with green CI. A person (the owner's role,
played by the test driver) did every human action.

Result: both cards reached **Merged** through real pull requests.

| Card | Title | PR | CI | Merged (UTC) |
|---|---|---|---|---|
| TOK-1 | Threads: changes-requested as an API state | [#32](https://github.com/jh0per/Toknila/pull/32) | verify on ubuntu, macos, windows, desktop and the 4 build jobs: pass | 22:21:04 |
| TOK-2 | Typed blocked reason when a card has no working dir | [#33](https://github.com/jh0per/Toknila/pull/33) | the same jobs: pass | 22:24:04 |

Wall clock from Start to the last merge: 39 minutes (23:45 to 00:24 local, UTC+2). Of that,
about 10 minutes were CI (the Windows `verify` job takes 10m45s and decided when each card could
merge).

## Setup

- Toknila built from `main` at `74caf13` (debug build of `toknila`/`tok`), daemon started with an
  isolated data home (`TOKNILA_HOME=<worktree>/.e2e-home`). `claude` 2.1.319, model `sonnet`.
- Project repository: a fresh clone of `jh0per/Toknila` next to the worktree, base branch `main`.
- Project `TOK`: merge mode `github`, PR title `{key}: {title}`, merge method `merge`, parallel
  limit 2, review mode `both`, default profile `claude-sonnet`, reviewer `claude-sonnet-reviewer`.
- Chain `e2e` with two independent cards (prompt and acceptance written like `plans/phase-1.toml`,
  telling the agent to follow `AGENTS.md`: tests first, gates green). Autostart off; the person
  started both.
- Profiles `claude-sonnet.toml` and `claude-sonnet-reviewer.toml` (a built-in name cannot be
  overridden) copy the built-in `claude-code` wiring (MCP, hooks, `--append-system-prompt-file`)
  and add `--model sonnet`. No `--dangerously-skip-permissions`:
  - worker: `--permission-mode acceptEdits` and `--allowedTools` for `cargo`, `npm`,
    `git status|diff|log|show|add|commit` and the `toknila` MCP server. No push, no network
    tools, no `rm`.
  - reviewer: first the same, then `--permission-mode dontAsk` (see findings 2 and 7).
  - `CARGO_TARGET_DIR` points into the data home so the two ships share one build directory
    (5.3 GB; the disk had about 30 GB free).
  - Both profiles got a first prompt as the first positional argument of the command (see
    finding 1). It is a workaround, not product behaviour.
- Permission prompts and the trust dialogs were answered as the person through the web/HTTP
  terminal input route (`POST /v1/tickets/{t}/runner/input`), because the CLI has no command for
  it. A small script approved prompts for option 1 ("Yes") unless the text looked risky (push,
  network, `rm -rf`, `checkout --`, ...). It held one: a mutation check in TOK-1 that edits a
  source file and restores it with `git checkout -p`; the person approved it by hand.

## Timeline

Times are local (UTC+2), 6 and 7 November 2026. "person" is the driver acting as the owner,
"agent" is the card's Claude Code session (events from MCP and hooks), "system" is the daemon.

| Time | Card | What | Who |
|---|---|---|---|
| 23:45:24 | both | project, chain and cards created, both Ready | person |
| 23:45:33 | TOK-1 | Ready -> Running, ship created, runner started | person (Start), system |
| 23:45:34 | TOK-2 | Ready -> Running, ship created, runner started | person (Start), system |
| 23:46 | both | terminal stuck on the "trust this folder" dialog, then "allow external CLAUDE.md imports"; person answered both in each terminal | person |
| 23:47:37 | both | sessions idle at an empty prompt; person left an instruction comment "begin now"; delivered to the live terminal | person, system |
| 23:47 to 23:54 | both | agents work; TOK-1 asked permission 12 times and TOK-2 7 times over the run (card Waiting, then Running again) | agent, person |
| 23:54:49 | TOK-2 | Running -> Review (`finished`); reviewer runner started | agent, system |
| 23:55:46 | TOK-1 | Running -> Review (`finished`); reviewer runner started | agent, system |
| 23:56 | both | both reviewers idle at an empty prompt (no first prompt); TOK-2's reviewer later stuck on a permission prompt nobody could answer | - |
| 00:00:53 | TOK-1 | **Return** (round 1, person): commit your work; add the missing API-level assertion. Reviewer stopped, comment typed into the worker's live session | person, system |
| 00:00:54 | TOK-2 | **Return** (round 1, person): commit your work (it was in the working tree only) | person, system |
| 00:03:07 | TOK-2 | Running -> Review, committed as `982c8c0`; reviewer started | agent, system |
| 00:05:28 | TOK-2 | **Return** (round 2, person): hand in again to get a fresh review, because the first reviewer was stuck | person |
| 00:05:31 | TOK-2 | Running -> Review again, no change; reviewer (now `dontAsk`, with a first prompt) started | agent, system |
| 00:06:36 | TOK-1 | Running -> Review, committed as `1dce6e4`, assertion added; reviewer started | agent, system |
| ~00:08 | TOK-1, TOK-2 | reviewers recorded verdicts: **approve** (round 2 for TOK-1, round 3 for TOK-2) | reviewer agent |
| 00:09:52 | TOK-1, TOK-2 | **Accept** (person), merge starts: push, PR opened, checks watched | person, system |
| 00:21:04 | TOK-1 | PR #32 merged by `gh`, card Merged, ship removed, runners stopped | system |
| 00:24:04 | TOK-2 | PR #33 merged, card Merged | system |

No card was blocked, crashed or nudged. No runner was stopped for budget: neither card looped or
stalled. Both cards were in Running for about 10 minutes before their first hand-in.

## Review rounds

Rounds as recorded in `review.rounds` (also in the PR bodies):

TOK-1
1. person, request changes: (a) commit the work, (b) assert at the API level that after the agent
   posts a new version the still-open thread has `changes_requested = false` again. The comment
   reached the same live session (journal `comment.delivered` at 00:00:53, no new run) and the
   agent addressed both points in the same session: commit `1dce6e4`, one added assertion.
2. reviewer agent, approve: unit cases covered, API field filled from core, web reads the field;
   it could not run `cargo xtask verify` or the web tests (finding 7), so it said BLOCKED for those
   and asked for CI. It noted a 5 ms sleep in the daemon test.
3. person, approve (Accept).

TOK-2
1. person, request changes: commit the work.
2. person, request changes: hand in again (workaround for the stuck reviewer).
3. reviewer agent, approve (static read only, same limit); noted missing end-to-end coverage and
   an overlong docs line.
4. person, approve (Accept).

The return round the plan asks for is TOK-1 round 1. It happened before any reviewer verdict
because the first reviewers never ran (findings 1 and 2); a second round with the reviewer first
was not repeated for TOK-1.

## Permission prompts

19 prompts in total (TOK-1: 12, TOK-2: 7), all answered "Yes" (option 1); none denied. Toknila
shows them as `Waiting` with the text "Claude needs your permission" and no command. Kinds, from
the terminal:

- compound shell commands with a Python or `sed` here-document used to edit files (most of them);
- several `cd` in one command, and `cd <dir> && git ...` ("changes directory before running a
  version-control command");
- reading files outside the working directory through `../../..`;
- the mutation check described under Setup.

The reviewer of TOK-2 hit the same `cd` prompt and no one could answer it (finding 2).

## Nudges, tokens

No nudge fired (the agents were active; the silence detector never reached its threshold). Token
usage is not visible in Toknila or in the run logs (only the per-turn counter of the terminal
UI), so none is reported.

## Findings, ranked by how much they hurt daily use

1. **The agent is never told to begin (worker and reviewer).** `claude` is launched with a system
   prompt addition but no first message, so every session sits at an empty prompt. The silence
   nudge would wake a worker after 120 s; a reviewer is never nudged (the detector watches the
   worker). Symptom: both workers idle after the trust dialogs; both reviewers idle for minutes
   (card shows "waiting for the reviewer"). Workaround here: a first positional argument in the
   profile command and, the first time, an instruction comment. Suggested fix: a `prompt` field in
   the profile (or Toknila sends the first message) with a default for the built-ins.
2. **A reviewer's permission prompt has no answer path.** The input route writes only to the
   worker's terminal, the reviewer's run cannot report a question (its token may send only
   `review_verdict`), and nothing appeared in the journal or the Inbox. The TOK-2 reviewer waited
   on a `cd ... && git ...` prompt until the person returned the card. Workaround: reviewer profile
   with `--permission-mode dontAsk`. Suggested fix: surface the reviewer's prompt as an Inbox item
   and accept input with `?role=reviewer`, or make the built-in reviewer non-interactive.
3. **Nothing tells the worker to commit, and Accept does not notice.** Both cards reached Review
   with all changes in the working tree. Toknila pushes the branch, so accepting them would have
   opened a PR without the work (inferred from the push step in `docs/architecture.md`; not run).
   Fixed in this PR: the `finished` line of the generated instructions now asks for a commit first
   (snapshot test seen failing before the change). Still open: Accept (or the hand-in) should
   refuse or warn when the ship has uncommitted changes or no commit ahead of the base.
4. **Every new ship starts with dialogs Toknila cannot see.** Claude Code asks "trust this
   folder?" for each new worktree, and "allow external CLAUDE.md imports?" because the data home
   here sat inside another checkout whose `CLAUDE.md` imports `AGENTS.md` (that part comes from
   the test layout, a data home outside any repository avoids it). The card stays Running, the
   Inbox is empty, and `tok` has no command to send keys; the answers went through the HTTP input
   route. Suggested fix: pre-trust the ship folder in the per-run settings, or raise a question
   item when the terminal shows a dialog, and add `tok input`.
5. **The permission item carries no content.** "Claude needs your permission" is all the card and
   the Inbox say; the person has to open the terminal to learn what is asked. With a narrow
   allow-list the agents asked 19 times in 10 minutes of work, mostly for shell shapes (here-document
   edits, `cd &&`), not for dangerous actions. Suggested fix: carry the tool and command in the
   hook payload text, and ship a better default allow-list or a recommended profile.
6. **A card stays Waiting after the person said yes.** `Waiting -> Running` waits for the next
   `post_tool_use`, which comes only when the approved command ends. During a 3 to 5 minute
   `cargo xtask verify` the card showed "waiting 4m" and the Inbox still listed a question.
   Suggested fix: when the person answers a permission item, move the card back to Running.
7. **A reviewer cannot run the checks.** In `dontAsk` mode the reviewer's `cargo xtask verify ...
   | tail` and `git diff | grep` were denied (pipes are not covered by the `cargo:*` or `git
   diff:*` rules), so both verdicts are static reads and say so. Adding `tail`, `head`, `grep`,
   `wc` to the reviewer's allow-list is the profile-level fix; the product-level fix is a reviewer
   profile recommended by Toknila.
8. **The reviewer's process lives until the merge.** The doc says it is one-shot and exits after
   its verdict, but both runners of each card exited together only when the card merged (journal
   `runner_exited` twice at 00:21:10 and 00:24:08). An idle reviewer holds a parallel slot while
   the PR's CI runs.
9. **The agent inherits the caller's environment.** The sessions showed "inherited
   `CLAUDE_CODE_CHILD_SESSION` marker, transcript saving is off" and ran the owner's global
   user-level hooks (a Stop hook of another tool). The daemon was started from an agent shell, so
   part of this is the test setup, but a runner should scrub `CLAUDE_CODE_*` variables from the
   environment it passes on.
10. **Small things.** `tok watch --json` prints the text format; the watch output has no
    timestamps (the timeline above comes from the database); the worker's and the reviewer's
    generated `instructions.md` and `mcp.json` share `runs/<ticket id>/`, so the reviewer
    overwrites the worker's file (harmless today because every start regenerates it).

## Fixes in this PR

- `crates/core/src/instructions/protocol.rs` and `snapshots/run.md`: the worker is told to commit
  before `finished` (finding 3).

Everything else is listed as a finding; the run was finished by working around it as described.

## Cleanup

The daemon and every runner were stopped after the run. The data home (`.e2e-home`, with the two
profiles, the chain file, logs and the shared cargo target directory) and the clone
(`toknila-e2e-repo`) were kept.
