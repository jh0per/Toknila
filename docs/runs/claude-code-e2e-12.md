# Claude Code end to end, twelfth run

Two epics at once, run by Claude Code agents inside the owner's installed Toknila.app 0.3.3 on the
owner's own TOKNILA project in **local** merge mode, the afternoon and evening of 15 November 2026.
The first epic, "Асистент показує, що робить" (TOKNILA-127, 128, 129 and the wildcard 140), was
drafted by the owner's assistant from the owner's words the night before and ran on `opus`: the
daemon learns the assistant's turn and step from hooks and the window shows them. The second,
phase 10 "The backlog, closed" (TOKNILA-130 to 139), was written by the orchestrator from the
candidates four phases had left in its notes, after the owner asked how much was left and said to
go on. Both epics were started by hand (the owner's epic is manual; phase 10 autostarted); the
person accepted after the daemon's check, its reviewer and the person's own gates (web, e2e-web,
verify) on a merge of `main` with the ship.

Result: every card reached **Merged**; phase 10 released **0.3.4**, the owner's epic **0.3.5**.
Fourteen cards in about five and a half hours of wall-clock time with two or three agents at
once; 7.7 hours of agent time; one Return by the person, one fix on `main` by the person.

| Card | Title | Hand-ins | Note |
|---|---|---|---|
| TOKNILA-127 | The daemon knows the assistant's turn and its current step | 1 | asked with choices when the brief and a window test disagreed, see finding 1 |
| TOKNILA-128 | The assistant reports its turn and each step through hooks | 1 | proposed the wildcard 140 from a real run |
| TOKNILA-129 | The window shows the assistant's live step and a turn that ended | 2 | design first; returned once by the person after `main` moved under it, see finding 3 |
| TOKNILA-140 | A step that reads the repository's own folder says "Reading the repository" | 1 | wildcard, approved by the person |
| TOKNILA-130 | Accept checks again when main moved, and runs the checks the changed files ask for | 1 | |
| TOKNILA-131 | A release in local mode can push main and the tag | 3 | two check rounds lost to a test it did not touch, see finding 2 |
| TOKNILA-132 | A card merged by any path retires its ship | 3 | the reviewer marked the unreproduced cause PERSON, see finding 5 |
| TOKNILA-133 | Three loose ends: the reviewer's npm rules, a second thread's refusal, a pty that is not there | 1 | its pty lock had the wrong scope, see finding 4 |
| TOKNILA-134 | A step a person must do by hand is a typed part of the hand-in | 2 | |
| TOKNILA-135 | A second message to the assistant keeps its note | 2 | |
| TOKNILA-136 | The New epic dialog asks for a title a person reads | 2 | returned by the reviewer to see its e2e red first |
| TOKNILA-137 | An e2e run leaves no daemon and no fake agent behind | 1 | 51 specs moved to one `session()` helper |
| TOKNILA-138 | cargo xtask e2e-web makes sure Playwright's browser is there | 1 | asked before moving the shared browser cache aside |
| TOKNILA-139 | A connection the daemon reset is never an invalid argument | 1 | added during the run from finding 2 |

## Setup

- Toknila.app 0.3.3; worker and reviewer model `sonnet` (the owner's epic: `opus`); command
  policy `auto`; review mode `both`; check command `cargo xtask verify`; setup command
  `npm ci --prefix apps/web --no-audit --no-fund` (set at the start of the run, missing since
  TOKNILA-81); two or three slots, lowered to two while the machine was loaded.
- Phase 10 imported from `plans/phase-10.toml`; 139 added with `tok ticket new` during the run.
- Pre-Accept gates as in runs 8 to 11: `gates.sh <ship-branch>` in a persistent gate worktree.
  One exception: 140, a one-file core change with a green check, was accepted on the daemon's
  check alone.

## Paths seen for the first time

- **An agent asked with choices and the person pressed one** (TOKNILA-119, from phase 9): 127
  when its brief and a window test disagreed, 131 when the check failed on a test it did not touch,
  138 before touching the shared browser cache. Each question was answered in one command and no
  round was lost to guessing.
- **The owner's assistant drafted an epic that ran on a stronger model**, with one design-first
  story whose thread the person approved after half an hour without the owner.
- **A wildcard from a real run** (140): the agent of 128 saw the step line read "Reading repo" in
  a real Claude Code run and proposed the fix; approved, done in ten minutes.
- **The person fixed `main` between two Accepts** (finding 4) and the next gates ran on the fix.
- **The machine was loaded by work outside Toknila** (the owner's iOS simulator tests; load
  averages of 150 to 230). The orchestrator lowered the slot limit and raised it again when the
  load fell; nothing was killed.

## Findings, ranked by how much they hurt daily use, and what became of them

1. **A brief forbade a folder that a pinned list lives in.** 127's brief said "touch no file of
   apps/web/src beyond the generated types"; the window's test wants a sentence for every kind of
   journal entry, and 127 adds one. The agent asked with three choices and recommended the one
   line; the person pressed it. Lesson for briefs: a folder ban names the exceptions the pinned
   lists need (event words, snapshots, generated types).

2. **A check failed on a test the card did not touch, twice, and cost two of its three rounds.**
   131's `cargo xtask verify` failed on crates/client's EINVAL test (from TOKNILA-121) while the
   load average was above 150. The agent asked instead of fixing a load problem; the person
   allowed one empty commit; the third check passed. The cause went into a new card the same hour:
   139 found that `body::lost` stopped at the first io error and never saw the EINVAL wrapped by
   ureq's header reader, so the GET was never repeated. Product candidate kept in the notes: a
   check that fails only in files the ship did not change says so and does not count the round.
   Also seen: an answer given with `tok answer` while the agent waited for a slot stayed in the
   inbox as an open question until it was delivered.

3. **A card in flight broke on the merge with a helper another card changed.** 137 moved every
   e2e spec to one `session()` helper and made `cleanup()` refuse a home it did not make; 129's new
   spec, written before that, still used `mkdtempSync` + `cleanup()`. The daemon's check runs
   `verify` only, so it passed; the person's gates ran e2e-web on the merge and failed with the
   exact sentence; the Return named the fix and the agent came back in ten minutes. From 0.3.4 on
   the project can name an extra check for `apps/web/**` (TOKNILA-130), so the daemon's check turn
   covers this and the hand gates can stop.

4. **A machine-wide lock made four verify runs wait on each other.** 133's pty tests take turns
   through a lock file in the temp folder, shared by every checkout on the computer; with the
   daemon's checks of three ships and the person's gate running at once, 128's gate waited for its
   turn until the ten-minute deadline failed four tests. Fixed on `main` the same hour (bed94e3):
   the lock is named by `TOKNILA_TEST_ROOT` (one per verify run) or the workspace root; AGENTS.md
   says a lock shared between test processes is scoped to the run or the checkout, never the
   machine. Lesson for briefs: a brief that asks for "a lock" names its scope; the reviewer
   approved without asking about other checkouts.

5. **A cause that could not be reproduced was marked PERSON, not PASS.** 132 could not reproduce
   why 13 merged cards of the owner's second project kept their ships; it made every path to
   Merged retire the ship, made the ticker retry a failed removal each round and recovery retire
   the ships of finished cards, and said so. The reviewer (TOKNILA-101's rule) marked the item
   PERSON and the person accepted: the retry and the recovery cover the leftover whatever its
   cause, and the owner's 13 ships are the check once 0.3.4 is installed.

6. **A resume took a fourth slot.** 127 waited on its question (its slot freed, TOKNILA-92), 133
   took the slot, and the answer resumed 127 at once: four agents at a limit of three, load 82.
   Candidate: a resume after a question waits for a slot like a start, with the `waits_for_slot`
   sentence of TOKNILA-118.

7. **Reviewers of web cards still could not run vitest or npx** (135, 138: "Bash denied in
   don't-ask mode"). 133 adds npm rules for a package.json under `apps/web`, but those reviewers
   ran before 133 merged. To check on the first web card of the next phase.

8. **Two epics planned the same version.** The owner's "Сховати проект зі списку" shipped in
   0.3.3 but kept a planned release node at 0.3.4, so `tok release steps 0.3.4` answered "2
   releases have version 0.3.4; use the release id". Candidate: an epic whose stories all merged
   before an earlier tag marks its node released with that tag, and the CLI takes the epic's name.

## What the owner can do now that was not possible before

- See what the assistant is doing while it works: the daemon knows its turn and its step from
  hooks, the chat's waiting line shows the live step, a turn that ended without an answer says so,
  the empty epic's box shows that the message went, and the Assistant button and sidebar row show
  a turn in progress (127, 128, 129, 140).
- Accept a story without the orchestrator's hand gates: Accept checks again when `main` moved
  since the check, and the project can name extra checks per path (130).
- Release and have `main` and the tag pushed by the release itself in local mode (131).
- Trust that a merged story leaves no workspace behind, by any merge path, and clean up old
  ones from Settings (132).
- Read in the hand-in what they must do by hand ("For you"), and see it in Needs you, the inbox
  row, `tok inbox` and `tok show` (134).
- Send a second message to the assistant and keep its note until the assistant reaches it; a
  stopped assistant offers Send again (135).
- Make an epic with a title a person reads; the slug is derived (136).
- Run the browser specs and leave nothing behind; e2e-web installs its browser when the cache
  was pruned (137, 138).
- Not read "Invalid argument" for "the daemon closed the connection"; an idempotent call is
  repeated once (139).
