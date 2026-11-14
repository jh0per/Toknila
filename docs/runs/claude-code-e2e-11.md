# Claude Code end to end, eleventh run

The phase-9 chain, run by Claude Code agents (Sonnet) inside the owner's installed Toknila.app 0.3.2
on the owner's own TOKNILA project in **local** merge mode, the evening of 13 November and the
night to 14 November 2026. The chain was written from one afternoon of the owner's real use: a
story of their second project went dead after an app update, the owner's answers went nowhere, a
question read like a developer's note, the owner did not know what Accept and Return do, the
assistant made an epic without its stories and went quiet for minutes, and a release node forgot
the stories added after it. Five cards were written first; four more came in during the run from
what the owner said and what CI showed. The epic autostarted; the person accepted after the
daemon's check, its reviewer and the person's own gates (web, e2e-web, verify) on a merge of
`main` with the ship.

Result: every card reached **Merged**; the chain released **0.3.3**. The owner's own epic
"Сховати проект зі списку" (TOKNILA-122, 123), drafted by the assistant and accepted by the owner
in the window, merged the same night and ships in 0.3.3 too.

| Card | Title | Hand-ins | Note |
|---|---|---|---|
| TOKNILA-117 | An update does not stop a story: a lost agent comes back by itself | 1 | lost runner = paused agent; a double loss within ten minutes blocks |
| TOKNILA-118 | A person's answer always reaches an agent, and the window says what happens to it | 1 | |
| TOKNILA-119 | A question a person can answer, with choices to press; the agent sees the checklist | 2 | returned once by the person for the screen-and-control sentence; merged after an Accept-time conflict with 117, see finding 3 |
| TOKNILA-120 | A question in a story's chat gets an answer when the agent is gone | 2 | its first run was lost to the owner's update; the second hit a full disk, see findings 1 and 4 |
| TOKNILA-121 | tok planner stop never fails with an OS error | 1 | the third red macOS CI from one test in two days; cause found by reading, not reproduced |
| TOKNILA-124 | The assistant builds the epic with its stories in one go | 1 | found that the roadmap context had no rule in the daemon, only prose |
| TOKNILA-125 | The assistant says what it is doing while it works; the panel counts the minutes | 1 | accepted by the owner in the window |
| TOKNILA-126 | A release waits for every story of its epic, including the ones added later | 2 | returned once by the person: the gates caught a regression the hand-in's NOT_RUN had skipped, see finding 2 |

## Setup

- Toknila.app 0.3.2; worker and reviewer model `sonnet`; command policy `auto`; review mode
  `both`; check command `cargo xtask verify`; four slots.
- The chain autostarted; cards added during the run were made with `tok ticket new` (124, 125,
  126 with `--after`, 121 free).
- Pre-Accept gates as in runs 8 to 10: `gates.sh <ship-branch>` in a persistent gate worktree.

## Paths seen for the first time

- **The owner updated the app while two cards ran.** Both agents were lost and the cards went
  Blocked (runner lost) on 0.3.2, the very defect TOKNILA-117 had merged into `main` an hour
  before; recovered by hand (unblock, pause, resume) and the replacement runs read their branches.
- **An Accept whose base sync conflicted** (119 against 117's snapshot) went to the agent, which
  merged `main`, passed the check and the reviewer, and merged on the earlier Accept with no
  second person turn.
- **A card asked for disk space.** 120's second run hit a full disk during the daemon's check,
  was returned by the system, and asked the person what to do with a Return that needed no code.
- **The owner used the assistant for a feature of their own** ("hide a project, not archive") and
  accepted the two stories it drafted, including a design thread, without the orchestrator.

## Findings, ranked by how much they hurt daily use, and what became of them

1. **An update stopped every running story and silenced every answer.** The owner's words:
   "о так це фігня шо після оновлення не працює". TOKNILA-117 (a lost runner is a paused agent
   that comes back when a slot is free, or when the person answers) and TOKNILA-118 (one Send on a
   lost card both restarts and delivers; the bubble says what the daemon does with the text) are
   the fix; the owner's own update in the middle of the run showed the before once more.
2. **The gates earn their run.** TOKNILA-126's hand-in said NOT_RUN for the browser gates because
   `apps/web` had not changed, but the daemon's new rule changed what the window shows: the
   planning spec failed (a second story added by hand no longer followed the first). The cause
   was in core (`tail_tickets` counted the edge to the release as a dependent). Rule for cards:
   a daemon change that alters what a page lists runs e2e-web whether or not the page's code
   changed; the person's gates run it regardless.
3. **An Accept while `main` moved is a merge without a second look.** After 119's conflict round
   the daemon merged on the person's earlier Accept once the check and the reviewer passed, so the
   person's gates never saw the merged result. Practice from now on: when `main` is not an
   ancestor of the ship, run the gates first and Accept only on a clean merge, or Return with
   "merge main".
4. **A full disk costs a round.** Two build caches of the orchestrator's own (the gate worktree
   and the main checkout, 115 GB together) filled the volume; the daemon's check failed 120 for
   it and the gates failed 121. Deleting the caches freed 113 GB. The agent's question ("may I
   clear this ship's target?") was the right one; the answer became an instruction (an empty
   commit that names the cause, since a hand-in at the same commit is refused).
5. **The reviewer approved a gap it had named** (run 10) and, on 119, approved a hand-in whose
   round-2 text had not seen the e2e red. The reviewer's instructions on `main` now say a gap it
   can name is a request_changes point; the 0.3.2 reviewer had not read them yet.
6. **One macOS test made CI red three times in two days.** TOKNILA-121 found that the planner
   stop route never read its request body, so a late segment met a closed connection and macOS
   reported EINVAL; the client's errors now name the call and the step. The cause was read, not
   reproduced; the CI runs after 0.3.3 are the check.
7. **The assistant's first feature request** showed three gaps at once: it made the epic and told
   the owner to open it and say "plan" (the roadmap context had no rule in the daemon, only
   prose, TOKNILA-124), it went quiet for three and a half minutes while reading code
   (TOKNILA-125), and the epic's release node depended on none of the stories made after it
   (TOKNILA-126).
8. **Words a person can act on.** The owner did not understand "turn a knob on a long sound and
   tap the pad" (the knobs and the pads are on different screens of their app) nor what Accept and
   Return do. TOKNILA-119's rule: a step a person must do names the screen and the control as the
   window shows them; TOKNILA-120: a Needs you list, a first-time hint and a Guide section.
9. **Leaks.** A test daemon and its fake agent from a phase-7 ship outlived their card by a day
   (parented to launchd); Playwright's Chromium vanished from the cache and failed e2e-web on
   `main` until reinstalled. Both in the backlog.

## What the owner can do now that was not possible before

Update the app while stories run and watch them continue; answer a question on a story whose
agent is gone and see the agent start with the answer; press a choice under an agent's question
instead of typing; read "Needs you" above Accept and Return and ask the assistant what to do in
the story's chat; ask for a feature on the roadmap and get the epic with its stories in one turn,
with the assistant saying what it is reading while it works; trust that an epic's release waits
for every story in it; and hide a project from the sidebar without archiving it.
