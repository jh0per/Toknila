# Claude Code end to end, seventh run

The phase-5 chain (`plans/phase-5.toml`, the backlog), run by real Claude Code agents (Sonnet)
inside the owner's installed Toknila.app 0.2.11, on the owner's own TOKNILA project in **local**
merge mode, 11 and 12 November 2026. The chain was cut from the orchestrator's backlog after three
read-only agents checked every open item against `main` at v0.2.11: most items were already done
by phases 2 to 4, seventeen gaps remained and became the cards. Review mode was `both` throughout:
the daemon ran `cargo xtask verify` and then its reviewer agent on every hand-in, and the person
accepted. The person ran `cargo xtask web`, `e2e-web` and `verify` once more in each ship after
merging `main` into it, before Accept.

Result: every card of the chain reached **Merged** into the local `main`.

| Card | Title | Hand-ins | Note |
|---|---|---|---|
| TOKNILA-50 | Migrations by name, never by number | 2 | returned once for the legacy-order guard, see finding 5 |
| TOKNILA-51 | A hook of work never answers a question | 1 | |
| TOKNILA-52 | Nothing lands that was not handed in | 3 | base-sync conflict with 53; then a hand-edited generated file, see finding 3 |
| TOKNILA-53 | The review turn says what it is doing | 1 | |
| TOKNILA-54 | An empty thread never blocks a hand-in | 1 | |
| TOKNILA-55 | The inbox row carries the hand-in | 1 | |
| TOKNILA-56 | A move on the diagram is one request | 2 | a file over the 400-line guard after merging 54, see finding 2 |
| TOKNILA-57 | Allow always says what it allows | 1 | |
| TOKNILA-58 | The design library stays consistent | 2 | asked which pdf renderer to use (answer: `sips`, `pdftoppm`, no new dependency); one compile fix after the merge with 64 |
| TOKNILA-59 | A release survives a restart | 2 | returned once: the e2e wrote its screenshot into the repository, see finding 6 |
| TOKNILA-60 | Cards keep their size honest | 2 | base-sync conflict with 56 |
| TOKNILA-61 | The assistant reads the repository and its designs, nothing else | 1 | |
| TOKNILA-62 | A merged card leaves nothing behind | 1 | |
| TOKNILA-63 | A test daemon dies with its test | 2 | returned once: the leak check was too wide for parallel runs, see finding 5 |
| TOKNILA-64 | An unknown field is refused | 2 | returned once: strictness had reached the stored types, see finding 4 |
| TOKNILA-65 | Talking to the assistant never hangs | 1 | |
| TOKNILA-66 | The instructions carry the lessons of runs 5 and 6 | 2 | its own new rule failed a test of 55; fixed in one round |

No daemon bug needed a fix outside the cards this time: the four daemon fixes of run 6 (PATH,
dying runner, reviewer, thread approval) held, and the dying-runner race did not show once.

## Setup

- Toknila.app 0.2.11; the owner's real data home (store at schema version 24, 249 comments);
  `claude` 2.1.322; worker and reviewer model `sonnet`; parallel limit 2; command policy `auto`;
  review mode `both`; check command `cargo xtask verify`.
- No card was design-first; one card asked a question (58, which pdf renderer) and got its answer
  through a comment within minutes.
- The orchestrator queued the cards two at a time in priority order, kept `after` dependencies
  for the cards that share files, and ran the three gates by hand in each ship before Accept.

## Paths seen for the first time

- **A reviewer that ran the gate and could not read its result.** Claude Code spills a tool output
  over about 100 KB to a file under its own config folder; the reviewer's Read is scoped to the
  ship, so the file was denied and the verify result line was `NOT_RUN` in six verdicts, with the
  named tests run and quoted instead. Once (52) the reviewer gave no verdict and the daemon sent
  the card to the person with the reviewer's reading as a note, as U14 says.
- **Merge artefacts from two cards on one file.** Twice a merge of `main` pushed a file over the
  400-line guard (`crates/core/src/events.rs` with 54 and 56; `crates/cli/src/output.rs` with 52
  and 53), and twice the base sync hit a real conflict (`crates/app/src/external.rs`, 52 with 53;
  `crates/api-types/src/lib.rs` and the tickets route, 60 with 56). Every one was resolved by the
  agent in one round.
- **A generated file edited by hand.** Two agents (52, 62) edited `apps/web/src/gen/api.d.ts` by
  hand because `apps/web/node_modules` was missing in the fresh ship; 52's agent said it had asked
  for permission to run `npm ci` and got no answer, yet no question ever reached the inbox.
- **Strictness that reached the stored data.** 64's first hand-in put `deny_unknown_fields` on
  every type, including the ones the store reads back (comment bodies, verdict items, sizes,
  regions, agent events); an old or newer row with one extra field would not have loaded.
- **A leak check too wide for this machine.** 63's first check failed any `tok daemon run` under
  the temp folder younger than the run, while two card agents and the orchestrator run `verify`
  at the same time.
- **A browser spec that dirtied the repository.** 59's `release.mjs` wrote its screenshot into
  `docs/runs/` on every run, so Accept after the person's own e2e run was refused with
  "changes beyond the hand-in commit".
- **The hand-in guard met the orchestrator's habit.** 52 makes Accept refuse commits beyond the
  hand-in except the daemon's base-sync merges; the person's practice of merging `main` into the
  ship by hand before the gates becomes such a commit from 0.2.12 on.
- **Every e2e run ends with "daemon did not exit after stop; killing it".** The e2e helper's stop
  of its test daemon needed the kill fallback on every run of this phase.
- **A shell edit that slipped through.** One `sed -i` edited a web file in 58; the shell-edit
  refusal catches python, perl and heredocs, not `sed -i`.

## Findings, ranked by how much they hurt daily use, and what became of them

1. **The reviewer cannot read Claude Code's spill file.** Six verdicts without the gate's result
   line; one card went to the person without a verdict. Instruction fix in 66: run a long gate as
   `cargo xtask verify 2>&1 | tail -n 30`, never redirect output to a file (reviewer and worker).
   Daemon candidate: the run's allow rules add Read on the run's own output spill folder.
2. **Shared files that two cards touch.** Named migrations (50) removed the migration array as a
   conflict point; the e2e list was removed by U18 before; the remaining hot files are the event
   enum and the CLI printer, both now split by responsibility. Rule kept in the instructions (66):
   merge the work branch into the ship before the hand-in, keep every entry of a shared list.
3. **Generated files edited by hand because `npm ci` did not happen.** Instruction fix in 66:
   never edit a generated file by hand, run the generator; when `node_modules` is missing run
   `npm ci` without asking. Candidate: a project `setup_command` the shipyard runs once after it
   creates a ship (for example `npm ci --prefix apps/web`). The vanished `npm ci` question is not
   explained: no inbox item, no daemon log line.
4. **`deny_unknown_fields` on stored types.** Returned once; 64 now refuses unknown fields only
   at the request boundary with `serde_ignored` and keeps every stored type tolerant, with a test
   that a stored comment with an extra field still loads. Rule for AGENTS.md: a type the store
   reads back never carries `deny_unknown_fields`.
5. **Guards that must know their scope.** 50's upgrade seed relied on new migration files sorting
   after the legacy ones, with no test; 63's leak check counted every test daemon on the machine.
   Both returned once and fixed: a test pins the legacy order; the leak check lists only daemons
   under `TOKNILA_TEST_ROOT`, which `cargo xtask verify` creates per run, with a five-second grace
   for daemons that are shutting down.
6. **A spec that writes into the repository.** 59's screenshot now goes to the e2e artefacts
   folder; `docs/runs/release.png` is refreshed only on request.
7. **The orchestrator's pre-Accept merge.** From 0.2.12 the gates run in a throwaway worktree of
   `main` merged with the ship branch, never as a commit in the ship. Product candidate: Accept's
   own sync reruns the check (and the web gate when `apps/web` changed) when `main` moved.
8. **The e2e daemon never exits on stop.** Candidate: find why the daemon's graceful stop does not
   end within the helper's wait during e2e, or give the helper the lifeline of 63.
9. **Planner tests under load.** The cli planner tests failed twice in one ship with a network
   error to the test daemon and passed alone; the third sighting of this family (once in CI for
   0.2.11). Card candidate, not a retry.
10. **`sed -i` is not a refused shell edit.** Candidate: add `sed -i`, `perl -pi` and redirections
    that write into the tree to the shell-edit deny patterns, with a test.

## What the owner can do now that was not possible before

A migration is a new file and never a merge conflict; a card that waits on a question or a
permission is never woken by a hook of work; Accept takes only what was handed in and shows what
changed since a Return; the review turn says whether it is queued, syncing or running the check,
and the daemon's own comments read as Toknila; an empty thread never blocks a hand-in and a second
thread of the same kind gets a hint; the inbox row carries the hand-in; a move on the diagram is one
request; Allow always says which rule it adds and trust allows every tool the deny list does not
name; the assistant and the agents may read the repository and the designs only; cards keep their
size honest; a merged card leaves no branch; a test daemon dies with its test and `verify` names a
leak; an unknown field in a request is refused with the accepted ones named; the discussion box
frees itself after ninety seconds; the instructions carry the lessons of runs 5 and 6; a deleted
design takes its references and pins with it after one question, a pdf shows its first page, a pin
without a card is in the inbox; a release survives a daemon restart and a released chain offers the
next release when a new card merges.
