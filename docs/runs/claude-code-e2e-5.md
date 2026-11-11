# Claude Code end to end, fifth run

The phase-3 chain (`plans/phase-3.toml`), run by real Claude Code agents (Sonnet) inside the
owner's installed Toknila.app (0.2.7, then 0.2.8), on the owner's own TOKNILA project in **local**
merge mode, 9 and 11 November 2026. The orchestrating session played the person through Toknila's
surfaces only: the inbox and answer routes of the HTTP API and `tok` (`start`, `comment`,
`return`, `accept`, `cancel`, the reject of a proposal). Three cards were added during the run
from what it showed (TOKNILA-36, 37, 38); two were cancelled (35 covered by a fix made outside the
chain, 39 a proposal for a test that fails only under load).

Result: every card of the chain reached **Merged** into the local `main`.

| Card | Title | Hand-ins | Note |
|---|---|---|---|
| TOKNILA-26 | Shell edits are refused, not asked about | 1 | hung in the Check turn, see finding 1 |
| TOKNILA-27 | An agent cannot read the person's private files | 1 | same; its deny rules probed live by hand |
| TOKNILA-29 | A ship that fell behind main is brought up before Accept | 4 | three hand-ins at one commit, see finding 2 |
| TOKNILA-30 | A question is never lost behind the silent state | 1 | found the missing transition |
| TOKNILA-28 | One check command at a time on this machine | 1 | its restart test raced, see finding 6 |
| TOKNILA-31 | A hand-in says what it did | 1 | |
| TOKNILA-38 | A hook of work never clears an open question | 1 | proposed a card for a load flake |
| TOKNILA-36 | The reviewer runs what the hand-in names | 1 | |
| TOKNILA-33 | Settings: the no-checks timeout and the warning mark | 1 | |
| TOKNILA-34 | The macOS release build does not fail on a busy disk image | 1 | |
| TOKNILA-32 | A finished chain becomes a release from the window | 1 | |
| TOKNILA-37 | Tests pass on a loaded machine | 3 | returned twice by a reviewer that could run nothing |

Six changes landed beside the cards, done by the orchestrator's own agents and merged by hand:
cards that fit their text and can be resized (U6), after-commit effects that outlive the request
(U7), the Return text in the restarted run's kickoff and the refusal of a same-commit hand-in
(U8), tests that wait for the person's turn (U9), the check line joined at commit (U10), and the
effect task spawned from the writer thread (U11).

## Setup

- Toknila.app 0.2.7 at the start, 0.2.8 from 11 November; the owner's real data home; `claude`
  2.1.319; worker and reviewer model `sonnet`; parallel limit 2; command policy `auto`; review
  mode `both`, switched to `human` on 11 November (see finding 4).
- `check_command` was `cargo xtask verify` until the first evening, then cleared as a workaround
  for finding 1 (the daemon of 0.2.7 and 0.2.8 lost the check); the person ran the check command
  in each ship by hand before Accept, after merging `main` into the ship.
- The owner's working copy `~/Work/Toknila` was the merge target; the orchestrator merged its own
  branches into it between the cards' merges. Other agents built and tested in worktrees of the
  same repository on the same machine.

## Paths seen for the first time

- **A hand-in that never left the Check turn.** Both first cards sat `being checked` for twenty
  minutes with no check process alive; a daemon restart handed them to a person.
- **A Return that nobody read.** TOKNILA-29 was returned with a code change to make and handed in
  the same commit three times, each within two minutes of the restart; the reviewer approved it
  every time by reading.
- **A reviewer that cannot run.** Every verdict of the run says "by reading only"; on the second
  day the reviewer returned TOKNILA-37 twice with FAIL for items it could not run.
- **A proposal for a flake.** The agent of TOKNILA-38 proposed a card to fix a test that fails
  only while another build runs.
- **Two branches off one `main`.** TOKNILA-29 and U8 each passed their gates and broke each
  other's tests when merged (a check round now precedes a Return, so the Return is round 2).
- **Tests under load.** Four tests failed only while other builds ran, on macOS and on Windows
  CI; one more (the check line's restart test) failed two runs in three on a quiet machine.
- **A Windows spelling.** TOKNILA-27's test compared raw paths (`C:\Users\x/.claude`) with the
  rooted rule form (`//c/Users/x/.claude`).
- **The person's own instruction overloaded the machine.** Asked for "two or three" build loops,
  the agent of TOKNILA-37 drove the load average to 118.
- **Leaked test daemons again.** Four `tok daemon run` processes from earlier test runs were
  found and stopped by pid.

## Findings, ranked by how much they hurt daily use, and what became of them

1. **The Check turn ran inside the agent's hand-in request.** `Store::write` awaited the
   after-commit effects, so the handler sat on `cargo xtask verify` for minutes; the agent's client
   gave up (or the `StopWorker` effect ended the agent), the connection dropped, axum dropped the
   future, and the check's result was never recorded. Fixed in U7: the effects run in a task the
   request does not own, `Store::settled()` for tests, a bounded shutdown that kills a running
   check, log lines for the check and the review turn. U11 closes the last gap (the task is
   spawned from the writer thread, in the same step as the commit).
2. **A Return never reached the restarted run.** Pending instruction comments came at the next
   Stop hook, after the first turn's `finished`. Fixed in U8: pending instructions and the Return
   text are in the kickoff and the instructions file, marked delivered at start; a hand-in after a
   Return at the same commit is refused with the Return quoted; the protocol has "After a Return".
3. **A person had to run the check and merge `main` by hand** while the daemon of the installed
   app lost the check. TOKNILA-28 queues the checks of a machine; TOKNILA-29 brings a ship up to
   its base before the check and before Accept, check command or not.
4. **The reviewer judged by reading.** Its profile could run git reads, `grep`, `head`, `tail`,
   `wc` and the check command only, nothing when the project had no check command, and never one
   named test. TOKNILA-36 derives read-only allow rules from the toolchain (`cargo test *`,
   `cargo xtask *`, `cargo clippy *`, `npm test *`...), tells the reviewer to run what the hand-in
   names and quote the lines, and marks each item `ran`, `read` or `not_run`. Still open: an item
   the reviewer could not run must be `not_run`, never `fail`, and a verdict with every item
   `not_run` goes to the person instead of returning the card.
5. **Hand-ins looked like one line.** The summary body was structured all along; `tok show`
   printed only its gist. TOKNILA-31 makes the template a rule (what changed, each check as PASS,
   FAIL, NOT_RUN or BLOCKED with evidence, what is left), refuses a hand-in without the checks
   section, and shows the blocks on the Review card.
6. **Tests that pass alone and fail together.** U10 fixed the check line (joined at commit, in
   hand-in order, instead of inside racing detached tasks); U9 made the CLI tests wait for the
   person's turn instead of racing the new Check turn; TOKNILA-37 made the remaining three wait on
   events with generous ceilings and the CLI survive a busy daemon. The Windows path test was
   fixed by comparing rules, not paths.
7. **Two branches off one `main`.** Each branch's gate was green and the merge was red twice
   (round numbering). TOKNILA-29's own feature now reruns the check after the base is merged in,
   on the new daemon; the orchestrator runs the full gate on `main` after every merge.
8. **Agents proposed and asked less, but one proposed a flake.** Instruction candidate: before
   proposing a fix for a failing test, run it alone once; a test that passes alone is a load
   problem to report, not a card.
9. **Load discipline, again.** One build loop under `nice`, never more, with a trap that stops it;
   the person's instructions must say so too.
10. **A question or permission is never cleared by a hook of work.** TOKNILA-30 and TOKNILA-38;
    an open permission prompt still resumes on a hook (kept on purpose: a prompt answered at the
    terminal yields only `post_tool_use`).

Findings 1 to 7 and 10 are fixed on `main` after this run; 4 (the `not_run` rule), 8 and 9 are
open as instruction text.
