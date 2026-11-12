# Claude Code end to end, eighth run

The phase-6 chain, run by real Claude Code agents (Sonnet) inside the owner's installed Toknila.app
0.2.12, on the owner's own TOKNILA project in **local** merge mode, 12 November 2026. The chain was
written in two parts: the owner's assistant (the planner, started by the owner from the window)
turned the owner's list of thirteen complaints into fourteen cards (TOKNILA-67 to 80) in one
conversation; the orchestrator added sixteen more (81 to 96) for the items the assistant did not
see (the assistant as a chat, a panel beside the work, a roadmap of epics, a model per card, the
agents list) and for what the runs of the day uncovered. Review mode was `both` throughout; the
person accepted after the daemon's check, its reviewer and the person's own gates on a merge of
`main` with the ship branch in a gate worktree (never a commit in the ship, as TOKNILA-52 asks).

Result: every card of the chain reached **Merged** into the local `main`; the chain released
**0.3.0**.

| Card | Title | Hand-ins | Note |
|---|---|---|---|
| TOKNILA-67 | The assistant can read a plan of any size | 1 | written by the assistant about itself: its own `show_plan` answer was 263,000 characters |
| TOKNILA-68 | Add project: an error goes away when the folder changes | 1 | |
| TOKNILA-69 | A folder that is missing or is not a repository can be made ready | 1 | |
| TOKNILA-70 | One design language: Material 3 tokens and base components | 1 | design thread; the owner being away, the orchestrator approved the recommendation (compact density, fonts kept) after three hours, see finding 1 |
| TOKNILA-71 | A screenshot dropped or pasted on a card joins the library and the card | 1 | design thread |
| TOKNILA-72 | Add project: fix the folder in place, and the main branch is always filled | 1 | design thread; the approval did not wake the agent, see finding 1 |
| TOKNILA-73 | Settings, onboarding and dialogs follow the design language | 1 | design thread |
| TOKNILA-74 | The chain screen and the card panel follow the design language | 2 | one check round (a load flake) |
| TOKNILA-75 | The assistant can look at a design added after it started | 1 | |
| TOKNILA-76 | A screenshot attached to a message reaches the assistant | 1 | design thread |
| TOKNILA-77 | Inbox and usage follow the design language | 3 | returned once by the person (group worktrees by repository, not by Toknila's own paths); once by the reviewer for items it cannot judge, see finding 4 |
| TOKNILA-78 | Sidebar: one tree per project | 1 | design thread (a Roadmap row added at approval) |
| TOKNILA-79 | Sidebar tree: inbox and agents under their project | 1 | design thread |
| TOKNILA-80 | A guide in the window: from a folder to a release | 2 | design thread; returned once by the person: the pictures showed a temporary folder path and the simulated-agent banner |
| TOKNILA-81 | A ship is ready to work in: the project's setup command | 1 | |
| TOKNILA-82 | The reviewer gets the gate's result instead of running it again | 2 | one check round: a wiring test still wanted the check command in the reviewer's instructions |
| TOKNILA-83 | `sed -i ''` after `cd &&` is a shell edit too | 2 | returned once by the person with the real cause, see finding 2 |
| TOKNILA-84 | Recovery compares with the last snapshot, not the last event | 1 | |
| TOKNILA-85 | The agents of a project, the assistant among them | 1 | |
| TOKNILA-86 | A model for the assistant, a model per card | 1 | |
| TOKNILA-87 | A roadmap of epics: chains in order, a new one at the end | 1 | design thread |
| TOKNILA-88 | Talking to the assistant is a chat with a record; the console is a tab | 1 | design thread; the largest card of the chain (56 files) |
| TOKNILA-89 | Text in a terminal can be selected and copied | 1 | its browser spec flaked under load twice, see finding 5 |
| TOKNILA-90 | The assistant works beside you: one panel that knows what is open | 2 | design thread; Accept's base sync met a conflict with 76 and blocked the card, see finding 11 |
| TOKNILA-91 | The CLI reaches a daemon that is still writing its address file | 3 | the agent disproved the card's hypothesis and found a real cause, see finding 3 |
| TOKNILA-92 | A card that waits for a person frees its slot and resumes on the answer | 2 | one check round: a race between the paused process's exit and the resume |
| TOKNILA-93 | A call on a connection the daemon closed never fails or repeats | 1 | found the second cause of the same family, see finding 3 |
| TOKNILA-94 | The terminal-copy spec waits for what it asserts | 1 | |
| TOKNILA-95 | A new project runs a real agent, never the simulated one by accident | 1 | from the owner's second project, see finding 6 |
| TOKNILA-96 | A nudge never answers a first-run screen; the person does | 1 | from the owner's second project, see finding 6 |

## Setup

- Toknila.app 0.2.12 (updated from 0.2.11 at the start of the day; the store's 24 legacy
  migrations were seeded by name, 257 comments intact); `claude` 2.1.322; worker and reviewer model
  `sonnet`; command policy `auto`; review mode `both`; check command `cargo xtask verify`.
- Parallel limit 2 at the start, raised to 3 and then 4 while design-first cards sat Waiting on a
  design decision and held their slots (TOKNILA-92 ends that), lowered to 2 when the load average
  passed 15 and raised again when the pipeline emptied.
- The owner ran a second project (github merge mode) on the same daemon through the day; its first
  real card is what findings 6 describe.
- Pre-Accept gates: `~/Work/Toknila-tools/gates.sh <ship-branch>` in the persistent gate worktree
  `~/Work/Toknila-gate`: detach at `main`, merge the ship branch, `cargo xtask web`, `e2e-web`,
  `verify`. Nothing is committed into a ship.

## Paths seen for the first time

- **The assistant planned its own phase.** The owner typed the list into the assistant's box;
  fourteen cards came back with file paths, reducer names and acceptance lists, and a card about
  the assistant's own blindness (its plan answer was too long for Claude Code to hand it back).
- **Design threads without the owner.** Ten cards were design-first. The agents posted a thread
  within a minute or two of starting (HTML artifacts with light and dark mock-ups, or Markdown
  with drawn states and a list of decisions). The owner was away; after three hours on the first
  one the orchestrator approved the agents' own recommendations and did so for the rest within
  minutes of each thread.
- **The approval did not wake the agent.** Twice (72, 70) a Waiting agent did not move after
  `tok thread approve`; its terminal showed its own "still blocked on approval" loop inside a tool
  call. A comment, delivered as an instruction, woke it in under a minute each time.
- **A first real card on a new project folder died in two minutes.** Claude Code's trust dialog,
  a `screen` question, a nudge typed into it, Enter on "No, exit".
- **A reviewer that cannot judge what it is asked.** Acceptance items about design threads and
  screenshots were marked FAIL by the reviewer, which sees neither.

## Findings, ranked by how much they hurt daily use, and what became of them

1. **A card waiting on a person holds a slot and does not wake on the answer.** Nine design-first
   cards each held a slot while Waiting; with two slots the pipeline stalled behind one design
   decision. Two agents did not resume on the thread approval and needed a comment. TOKNILA-92
   (merged): a Waiting card loses its process after a grace, holds no slot, and the answer
   restarts it with Claude Code's resume form and the answer as its first input. The approval path
   is now a restart, not text typed into a pty.
2. **The shell-edit refusal was never asked.** The `sed -i ''` of run 7 passed because the rule
   lived in the PermissionRequest hook, which fires only when Claude Code would ask; under command
   policy `auto` the classifier allowed the call. TOKNILA-83 (merged): the PreToolUse hook asks the
   daemon's `pre-tool` route for the automatic decisions and prints Claude Code's deny; on any
   error it prints nothing. AGENTS.md: a rule that must hold in every permission mode lives in
   PreToolUse.
3. **"Invalid argument (os error 22)" had two causes, neither the one the card guessed.** The
   card said: a cut address file. TOKNILA-91's agent showed the daemon already wrote it atomically
   and found ureq's overall timeout re-arming the socket timeouts per read, which macOS refuses
   once the daemon has closed; TOKNILA-93 found the second: after a complete answer ureq hands the
   socket back to its pool with `set_read_timeout(None)`, refused the same way when the daemon
   reset the connection. Both fixed in the client; twenty loaded runs green where two of twenty
   failed before. Lesson for cards: name the evidence, offer the hypothesis as one, ask for a
   reproduction before a fix.
4. **The reviewer failed items only a person can judge.** TOKNILA-77 lost a round to FAILs about a
   design thread the reviewer cannot see and screenshots that live in the agent's scratchpad by
   policy. Candidate (phase 7): the reviewer's kickoff carries the card's threads with their state,
   items a person alone can judge are reported as `PERSON`, never FAIL, and the reviewer never asks
   for files the card did not ask for.
5. **Browser specs under load.** The terminal-copy spec of TOKNILA-89 failed two gate runs in
   different places at load averages over 30; TOKNILA-94 made it poll for what it asserts (the
   clipboard, the selection, the marker rows) and clear the clipboard between steps; ten loaded
   runs green. Three timing tests of the daemon's `effects_outlive` family failed under load and
   passed alone; candidate: a virtual clock or load-tolerant bounds.
6. **A new project ran nothing, twice over.** The owner's second project got the agent profile
   `sim` because the add-project form sends none and the daemon defaulted to it; its cards read
   "running" with no process. Once switched to the real agent, the first card died in two minutes:
   the startup rule raised a `screen` question for Claude Code's trust dialog but the card stayed
   Running, the two-minute nudge typed into the dialog, Enter took "No, exit". TOKNILA-95 (the real
   agent is the default; `sim` only by name; a simulated project shows a banner and its cards read
   "simulated") and TOKNILA-96 (no nudge before the first sign of life or during a screen question;
   a screen question is Waiting with the dialog's two answers in the inbox; trust the project's
   ships folder once) are merged.
7. **Load.** Three agents building, their test daemons, the daemon's own checks, the orchestrator's
   gates and a second project pushed the load average past 30; the pty count stayed well under the
   limit, but every flake of the day came at that load. Rule kept: gates and three agents never at
   once; parallel 2 when the load passes 15.
8. **The machine's agents list lied.** The sidebar listed every running card of every project
   under the open project's slot stepper, and never the assistant. TOKNILA-85 (merged): `/agents`
   carries the runs of each project, the assistant first.
9. **Screenshots in the repository.** Two cards committed before/after screenshots under
   `docs/screenshots/<card>/` (about 1.3 MB); two kept them in their scratchpad. Policy to settle
   in the instructions: the scratchpad, with the path in the hand-in.
10. **A reviewer that cannot run the web gates** writes `NOT_RUN` for `npm run gen:check` and the
    browser specs (no Chromium, no npm in its allow list); the person's gates cover them.
11. **A conflict at Accept blocks the card instead of returning it.** TOKNILA-90 was handed in
    before 76 merged; Accept's base sync met a conflict in two files and the card went Blocked
    (`merge_conflict`, "run `git merge main` in the ship … then finish again"), while the same
    conflict at hand-in time goes to the agent as a round. Unblock put it back in Review and the
    daemon retried the merge within seconds, so Return was refused; pause, resume, a comment and
    start got the agent back, and it resolved the conflict in one run. Candidate: Accept's conflict
    returns the card to the agent with the files named, no person step.
12. **The orchestrator's watcher went blind for two hours** on the same card: its inbox poll
    found nothing while the inbox held the row (the call worked by hand afterwards); 90 waited
    for its gates that long. The script gets a log line per poll.

## What the owner can do now that was not possible before

Add a project from an empty folder and have Toknila create the folder, make it a repository and
the first commit, with the main branch filled; a new project runs a real agent; the assistant reads
a plan of any size, talks in a chat with a record and a Copy on every message, keeps the console as
a tab, sits beside whatever is open with a chip that says what the message is about, takes a
screenshot attached to a message or dropped on a card, and chooses a model per card; the roadmap
of epics is the project's page with a new epic always at the end and the next version suggested;
the sidebar is one tree per project with its roadmap, assistant, inbox, agents, chains, designs
and settings; every screen follows one design language with a guard against raw values; text in a
terminal can be selected and copied; a card that waits for a person frees its slot; the reviewer
gets the gate's result; a ship is prepared by the project's setup command; the shell-edit refusal
holds in every permission mode; the address file and the client's timeouts no longer fail under
load; the guide in the window walks from a folder to a release.
