# Claude Code end to end, sixth run

The phase-4 chain (`plans/phase-4.toml`, visual planning), run by real Claude Code agents (Sonnet)
inside the owner's installed Toknila.app 0.2.9, on the owner's own TOKNILA project in **local**
merge mode, 11 November 2026. The orchestrating session played the person through Toknila's
surfaces only: the thread, inbox and answer routes of the HTTP API and `tok` (`start`, `comment`,
`accept`, `pause`, `resume`). Two cards were added to the chain before the run from the owner's
words of the morning (TOKNILA-48, a phase shows whether it is active or finished; TOKNILA-49, Start
is the epic and Finish is the version). Review mode was `human` throughout, with the daemon's
reviewer agent on for the first two hand-ins (see finding 3).

Result: every card of the chain reached **Merged** into the local `main`.

| Card | Title | Hand-ins | Note |
|---|---|---|---|
| TOKNILA-44 | The assistant opens a card for you | 1 | ran alone on 0.2.8 the evening before |
| TOKNILA-40 | Designs attach to a project | 3 | design in two versions; rounds 1 and 3 were Toknila's, see findings 1 and 3 |
| TOKNILA-48 | A phase shows whether it is active or finished | 3 | same two rounds; opened a second design thread, see finding 6 |
| TOKNILA-41 | The assistant reads the designs first | 1 | |
| TOKNILA-42 | A card shows its mockups | 1 | |
| TOKNILA-46 | The assistant says the next step | 2 | base-sync conflict with 41 at the hand-in, see finding 5 |
| TOKNILA-49 | Start is the epic, Finish is the version | 1 | |
| TOKNILA-43 | The assistant pins what is unclear on the design | 1 | |
| TOKNILA-45 | A card is discussed with the assistant | 2 | base-sync conflict in the e2e list, then no agent for 75 minutes, see findings 5 and 2 |
| TOKNILA-47 | The plan as a board of designs | 1 | designed, built and handed in within fifteen minutes |

Six changes landed beside the cards, done by the orchestrator's own agents and merged by hand,
all of them daemon bugs the run exposed: the daemon's PATH (U12), the dying runner (U13), the
reviewer that could run nothing (U14), the thread approval that never reached a waiting agent
(U15), the stopping run that asked a question (U16) and a Windows path spelling in a test (U17).
Release 0.2.10 (tag on `b6b9d53`) carries them with cards 40 to 44, 46, 48 and 49; 45 and 47 come
with the next release.

## Setup

- Toknila.app 0.2.9 (updated from 0.2.8 at the start; the update restarted the daemon and lost the
  two agents that were running, which came back with `pause`, `resume`, `start`); the owner's real
  data home; `claude` 2.1.319; worker and reviewer model `sonnet`; parallel limit 2; command policy
  `auto`; review mode `human` (the reviewer agent of 0.2.9 is the subject of finding 3).
- `check_command` was `cargo xtask verify` and became `PATH="$HOME/.cargo/bin:$PATH" cargo xtask
  verify` after finding 1; the person ran `cargo xtask web` and `e2e-web` in each ship by hand
  before Accept, after merging `main` into the ship.
- Every card but 41, 44 and 46 was design first: the agent opened one design thread, the person
  approved it (with answers to the open points) and the agent built. On 0.2.9 the approval had to be
  followed by a `tok comment` or the agent never woke (finding 4).

## Paths seen for the first time

- **A check that could not find cargo.** The daemon started from Finder had no `~/.cargo/bin` on
  its PATH; `sh -c "cargo xtask verify"` exited 127 and the first two hand-ins each lost a round.
- **A card running with nobody in it.** Twice after a fast check failure the card read `running`
  for 2.5 hours with no agent process; a third and a fourth time after a base-sync conflict (the
  last one, TOKNILA-45, for 75 minutes while the fix was already in the release the owner had not
  installed yet).
- **A reviewer that failed what it could not run.** The reviewer agent could run no command on
  0.2.9 and returned both first cards with FAIL on every item, each marked NOT_RUN.
- **An approval nobody heard.** Approving a design thread while the agent waited did nothing; the
  agent sat on "Still waiting for approval of design v1" until a comment arrived.
- **Two design threads on one card.** TOKNILA-48 opened a second design thread with the mockups
  while its first was approved; the hand-in was refused until the person approved the second one.
- **A ship behind `main` at the hand-in.** TOKNILA-46 (planner files, with 41) and TOKNILA-45
  (the e2e list in `xtask/src/main.rs`, with 43 and 49) hit the base-sync conflict rule of
  TOKNILA-29; both agents merged `main`, resolved and handed in again.
- **Migration numbers chosen in flight.** Five cards needed a migration; the orchestrator assigned
  the numbers in the kickoff context and the agents renumbered at the merge (46 took 22 after 42's
  21 landed first).
- **A release that went red on Windows only, three times.** A test that saw a question from a run
  that was still ending (U16) and a test comparing raw paths with rooted rules (U17).
- **The e2e tested the old bundle.** `cargo xtask e2e-web` serves the dist of the last `web`
  build; after a merge the old UI was tested until `web` ran again.

## Findings, ranked by how much they hurt daily use, and what became of them

1. **The daemon's PATH.** A daemon started from Finder inherits the launchd PATH, so the check
   command cannot find cargo while the agents it starts can (the runner adopts the shell's PATH).
   Fixed in U12: the daemon adopts the agents' PATH at start, and an exit 127 or a "command not
   found" on the last lines of the check is a machine problem, not the agent's: no round, the card
   goes to the person with the toolpath hint.
2. **The dying runner.** `finished` pushes StopWorker; when the check failed within milliseconds
   the restart found the old run not yet over and wrote into a process that was ending. Four cards
   hit it (40, 48, 46, 45). Fixed in U13: a run has a stopping state, a start never reuses one,
   and a run that ends with its card still `running` is replaced or the card is marked crashed.
   The person's way to see it on an old daemon: a `running` card whose terminal ends with "Resume
   this session with" and a daemon with no child process; the way out is pause, resume, start.
3. **The reviewer failed what it could not run.** Its command shapes (`cd`, `&&`, `VAR=`, `sh -c`)
   were refused by the allow rules, so every item was NOT_RUN and the verdict said FAIL. Fixed in
   U14: a verdict with every item unrun is no round and goes to the person; the reviewer
   instructions ask for plain commands and forbid FAIL on an item it did not run; the daemon logs
   its own refusals. The rules were right all along, which a wiring test now shows.
4. **A thread approval did not wake the agent.** The approval changed the thread and nothing else;
   the agent's `ask_human` on the design stayed open. Fixed in U15: approve and request-changes on a
   Waiting or Running card record an instruction the agent receives, and the design-first text says
   to wait, not to ask.
5. **The hand-in met a `main` that moved.** The TOKNILA-29 rule caught it every time, and the
   agent resolved it in one round each; the cost was a round and the conflicts were always the same
   two files: the e2e list in `xtask/src/main.rs` and the migration array. Candidates: the e2e
   runner discovers its specs from the folder; migrations by name instead of position.
6. **A second thread when the first was approved.** TOKNILA-48 posted its mockups as a new thread
   instead of a new version of the approved one, then could not hand in. The design-first text of
   U15 now says: never a second thread for the same design, answer with a new version in the same
   thread, and wait for the approval instead of asking. Candidate: the daemon answers a second
   `open_thread` of the same kind with a hint to post a version on the first.
7. **A Return read as a Return.** After U12 and U14 the agents of 40 and 48 handed in an empty
   commit with the quoted gate lines, as told, which is the right move for an environment failure.
   Instruction candidate kept: when a check failure is about the environment, say so and stop.
8. **The e2e bundle.** `e2e-web` should build the web bundle when the dist is older than the
   sources, or say which bundle it tests.

Findings 1 to 4 are fixed on `main` after this run; 5, 6 and 8 are open as cards or instruction
text; 7 is an instruction candidate.

## What the owner can do now that was not possible before

Drop designs on a project and paste Figma frames; the assistant reads them first and names them on
every card it proposes; a card shows its mockups with the region outlined and a thumbnail on its
node; the assistant pins what is unclear on the mockup and the person answers in place; the
assistant opens a card in the window, keeps one next step on its screen and first in the inbox,
and is talked to from the card page, with its edits shown as a diff to accept or reject; a chain
reads finished or active at a glance, finished chains fold away, Start is the epic and Finish is
"Release vX.Y.Z"; a chain's designs lay out as a board with every card's region.
