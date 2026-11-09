# Claude Code end to end, fourth run

The rest of the phase-2 chain, run by real Claude Code agents (Sonnet) inside the owner's installed
Toknila.app 0.2.6, on the owner's own TOKNILA project in **local** merge mode, 8 and 9 November
2026. The orchestrating session played the person through Toknila's surfaces only: the inbox and
answer routes of the HTTP API, and `tok` (`start`, `comment`, `return`, `accept`). Two cards were
added during the run from what it showed (TOKNILA-24, TOKNILA-25).

Result: eleven cards reached **Merged** into the local `main`; two are still running as this is
written (TOKNILA-11, TOKNILA-16).

| Card | Title | Hand-ins | Note |
|---|---|---|---|
| TOKNILA-12 | Cancelled cards leave nothing behind | 2 | one Return round (wording, actor) |
| TOKNILA-19 | No flaky tests | 1 | found the leak and the spin, see finding 1 |
| TOKNILA-13 | Replacing the daemon asks while agents run | 1 | asked its design question through `note` |
| TOKNILA-21 | Warn when the data home sits inside a repository | 3 | kept editing after `finished` |
| TOKNILA-24 | The daemon holds agents to the protocol | 2 | added during the run |
| TOKNILA-15 | Usage: live page, status line, agent chips | 1 | asked to copy the owner's transcripts |
| TOKNILA-14 | The reviewer can read the sources a card names | 1 | |
| TOKNILA-25 | The daemon closes the loop | 4 | merged `main` twice mid-card |
| TOKNILA-22 | CI keeps the e2e screenshots | 2 | |

Four more changes landed beside the cards, done by the orchestrator's own agents and merged by
hand: the command policy with "Allow always", the WebGL terminal, the node-graph canvas with
draggable cards, and design-first cards.

## Setup

- Toknila.app 0.2.6, the owner's real data home; `claude` 2.1.319; worker and reviewer model
  `sonnet`; parallel limit 2; `check_command` = `cargo xtask verify`; review mode `human`, switched
  to `both` during the run.
- The owner's working copy `~/Work/Toknila` was the merge target; the orchestrator merged its own
  branches into it between the cards' merges.
- Other agents built and tested in worktrees of the same repository on the same machine.

## Paths seen for the first time

- **A question in a `note`.** Two agents (TOKNILA-13, TOKNILA-19) asked a decision through `note`
  and sat `Waiting: silent` until the person read the card. Nothing reached the inbox.
- **Edits after `finished`.** TOKNILA-21 handed in, then kept editing: Review showed a workspace
  that differed from the hand-in commit; Accept would have merged the commit and lost the rest.
- **A lost `ask_human`.** TOKNILA-15 asked properly; the tool answered "waiting", but no question
  appeared in the inbox and the card read `Waiting: silent`. Answered through an instruction.
- **Copying the person's private files.** TOKNILA-15 asked to copy `~/.claude/projects` (every
  conversation of the owner) into its scratchpad for a measurement. Denied; it built synthetic data.
- **Pattern kills and heavy loops.** TOKNILA-13 asked for `pkill -f`; TOKNILA-19 wrote a stress
  script with twelve busy loops; agents started `cargo xtask verify` in parallel on the same machine.
- **Rewriting every snapshot.** TOKNILA-25 asked to run the whole workspace with
  `TOKNILA_UPDATE_SNAPSHOTS=1`, which would have hidden regressions.
- **Two cards, one migration number.** TOKNILA-14 and TOKNILA-25 both added migration 16; the
  second had to merge `main` and renumber.
- **Merging `main` into a running card.** Done twice for TOKNILA-25; the card then handed in before
  reading the instruction and had to be returned to finish the merge properly.
- **The orchestrator's own gate leaked.** A `cargo xtask verify` on the main checkout left 31
  `tok daemon` processes behind, each at about 36 % CPU, most ignoring SIGTERM.

## Findings, ranked by how much they hurt daily use, and what became of them

1. **Test daemons leaked and spun.** The daemon's usage ticker parsed every transcript under
   `~/.claude/projects` from the top on a fresh data home, so each test daemon stayed busy for its
   whole life, and the daemon waited for that work on SIGTERM. Fixed in TOKNILA-19 (tests isolate
   `CLAUDE_CONFIG_DIR`, a drop-and-signal guard stops the daemons a test started, the runtime is
   shut down with a timeout) and TOKNILA-15 (the collector reads in bounded passes, low priority).
2. **Agents slip past the protocol.** Questions through `note`, edits after `finished`,
   re-verifying after the hand-in. The instructions every agent gets now say so, and TOKNILA-24
   makes the daemon hold the line: the worker's run ends at the hand-in, Accept is refused while
   the workspace differs from the hand-in commit, a note that asks a question becomes a question.
3. **A person had to close every loop.** Three Return rounds in one evening for things a check
   or a reviewer could have caught. TOKNILA-25: the daemon runs the check command itself on
   `finished`, the reviewer judges each acceptance item, a card that does not pass goes back to its
   worker up to the project's round limit, and new projects default to review mode `both`.
4. **Twenty permission prompts for a small card.** The command policy (ask, auto, trust) and
   "Allow always" landed in the same batch; the owner's project stays on `ask` until switched.
5. **Agents reach for shell edits and interpreter probes** despite the instruction (every card
   started with a `python3` heredoc). The instructions now name it explicitly; the daemon could
   refuse a heredoc that writes a file outright.
6. **Private files are reachable.** An agent may read anything the person can. The instructions
   now forbid it; a sandbox for the agent's reads is a later card.
7. **Load discipline is the person's job.** Nothing stops two cards and the orchestrator from
   running three full test suites at once. The instructions ask for one gate at a time; a
   machine-wide check queue is a later card.
8. **Migration numbers collide** when two cards add one. A later card could allocate them by
   name or detect the collision at merge.
9. **An `ask_human` can vanish** behind the silent-agent state (finding of TOKNILA-15). To
   reproduce with the fake agent and fix: a question always wins over silence.
10. **The hand-in summary is often empty or one line**, so the person reads the diff instead.
    The instructions ask for what changed, each check as PASS/FAIL with evidence, and what is left.

Findings 1 to 4 are fixed on `main` after this run; 5 to 10 are open.
