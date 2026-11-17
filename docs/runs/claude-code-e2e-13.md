# Claude Code end to end, thirteenth run

Two epics, run by Claude Code agents inside the owner's installed Toknila.app 0.3.5 on the owner's
own TOKNILA project in **local** merge mode, on 16 and 17 November 2026. The owner said "доробити
весь беклог" (finish the whole backlog) and the orchestrator planned the rest. The first epic,
phase 11 "The rest of the backlog" (TOKNILA-141 to 149 from `plans/phase-11.toml`, plus 150 (a wildcard)
and 151, added during the run), shipped as 0.3.6. The second, phase 12 "What phase 11
found" (TOKNILA-152 to 156 from `plans/phase-12.toml`), is merged and waits for release 0.3.7.
Worker and reviewer model `sonnet`; 149 ran on `opus`, architecture first.

Result: all sixteen cards reached **Merged**; phase 11 is released as **0.3.6**, phase 12 goes
out as 0.3.7 next. For the first time the orchestrator ran no hand gates.

| Card | Title | Hand-ins | Note |
|---|---|---|---|
| TOKNILA-141 | A release is published only after CI passed on the same commit | 3 | returned twice by the person, see below |
| TOKNILA-142 | An agent answered while it waits starts only when a slot is free, and the inbox says it is answered | 2 | the reviewer asked for a reproduction and the agent found the path |
| TOKNILA-143 | A check that fails once and passes again is a flaky test for the person, not a round for the agent | 3 | merge conflict at its Accept re-check, then red-first evidence and a CLI test asked; merged on an earlier Accept, finding 5 |
| TOKNILA-144 | A version belongs to one epic; an epic shipped inside another's tag is marked released with it | 1 | |
| TOKNILA-145 | The reviewer reads the hand-in it reviews, and its git commands pass in don't-ask mode | 3 | returned once by the person for a contradiction in the reviewer's instructions ("quote every path" against "no quotes") |
| TOKNILA-146 | Test runs stay inside: the leak checks work on Windows, and a test can never start a real agent | 2 | its new guard caught a third test that started a real agent (a CLI planner test) |
| TOKNILA-147 | An unknown query parameter is refused, as an unknown body field is | 1 | |
| TOKNILA-148 | On Windows the terminal of an agent keeps the first lines of its output | 1 | two pushes to a `ci/` branch; loss not reproduced, see below |
| TOKNILA-149 | The planner's routes are the card and epic routes, scoped by who calls them | 1 | architecture first; the agent built aliases, finding 4 |
| TOKNILA-150 | Remove the planner's deprecated /planner/... aliases after the next release | 1 | wildcard; removed the 18 aliases, about 3,000 lines |
| TOKNILA-151 | An approval's own words win over the version they approve | 2 | added during the run from finding 4 |
| TOKNILA-152 | A request for changes after an Accept voids the Accept; the person sees the final work | 2 | accepted by the owner in the window; its re-check ran twice because another card merged during each |
| TOKNILA-153 | A hand-in never adds a build artifact to the repository | 1 | |
| TOKNILA-154 | Checks run side by side while the machine has room, and an Accept's re-check goes first | 2 | from finding 3 |
| TOKNILA-155 | A test leaves no daemon behind however it is run, and an old leftover is always reported | 3 | root cause: every cleanup ran inside the spec's own process, so a SIGKILL ran none; fixed with a detached watcher |
| TOKNILA-156 | The reviewer may run the exact test commands the hand-in names when they run a file of the repository | 1 | |

## Setup

- Toknila.app 0.3.5; worker and reviewer model `sonnet` (149: `opus`); project TOKNILA in local
  merge mode; two or three slots, lowered to two under load.
- The first run with no hand gates by the orchestrator. The project's extra check
  `apps/web/** = cargo xtask web && cargo xtask e2e-web` (TOKNILA-130) and the Accept re-check ran
  in the daemon. The person accepted after the daemon's check and the reviewer.
- 0.3.6 was cut by the epic's own release run with its Push step (TOKNILA-131). GitHub published
  only after CI passed on the tag's commit: the wait-ci job of TOKNILA-141, in its first real use.
- Phase 11 imported from `plans/phase-11.toml`; 150 and 151 added during the run. Phase 12
  imported from `plans/phase-12.toml`.

## Paths seen for the first time

- **A release that pushed by itself and a publish that waited for CI** (0.3.6).
- **An Accept re-check that returned a conflict to the agent** without the person (143).
- **An architecture-first card** (149): the thread was approved by the person with the message
  "no alias period", but the agent followed the pinned version, see finding 4.
- **The owner accepted a card in the window** while the orchestrator worked (152).
- **An agent pushed to a `ci/` branch to use the Windows runner** (148), under a rate rule: at
  most six pushes, one look per 15 minutes. It pushed twice. The loss of first lines was not
  reproduced; the old relaxed pattern had hidden CRLF line ends. The spec is strict on every OS
  again, with probes that name the layer if the loss returns.

## Findings, ranked by how much they hurt daily use, and what became of them

1. **A release raced with merges and with load.** 0.3.6 took three attempts. The release's Check
   failed once on a timing test while the load average was above 150. Then the Tag step refused
   "main moved while the release ran", because the orchestrator accepted a card during the Check.
   Candidates kept in the notes: merges wait while a release runs; the release's Check joins the
   machine-wide check line.

2. **Load from outside Toknila.** About 280 iOS simulator processes of the owner's other work
   pushed load averages to between 100 and 330. One check ran four hours; two agents asked about
   failures in tests they had not touched. The second run of a failed check (143) was on `main`
   but not yet in the installed 0.3.5.

3. **The check line ran one check at a time.** Six cards waited up to 40 minutes on an idle
   machine. Became 154.

4. **An approval message was ignored.** The agent followed the pinned version of the thread and
   built the aliases the person had said not to build. Became 151; 150 removed what it had built.

5. **An Accept survived a reviewer's request for changes** (143). Became 152.

6. **A build artifact was committed and passed the check and the reviewer** (141, a `.pyc` file).
   Became 153.

7. **The reviewer ended with nothing run after a Return, twice** (141's Python test script,
   146's new test). Became 156; 145 gave it the hand-in and command forms that pass.

8. **Leftover test processes.** A spec killed with SIGKILL left its daemon (155). The
   orchestrator's own hand run of a spec left one for twenty hours.

9. **Two small ones.** When an extra check fails, the round's headline names the main command and
   its passing exit code. An accepted card's re-check can chase `main` while other cards land.
   Both kept as candidates.

10. **The orchestrator fixed CI by its own hands after 0.3.5** (four commits). The owner's rule,
    restated: the orchestrator hands work out and picks the model and effort; every fix is a card
    or a delegated task.

## What the owner can do now that was not possible before

- Release and have GitHub publish only after CI passed (141).
- Never see a paused agent take a slot it does not have, and see an answered question as answered
  (142).
- Have a test that fails once and passes again reported as flaky instead of costing a round (143).
- See one version per epic, and epics shipped inside another's tag marked released (144).
- Have the reviewer read the hand-in and run the tests it names (145, 156).
- Be sure tests never start a real agent and leave no process behind (146, 155).
- Get 400 for a misspelt query parameter (147).
- Trust that an approval's own words win (151).
- Have an Accept withdrawn when changes were asked after it (152).
- Never get a build artifact in a hand-in (153).
- Run checks side by side when the machine has room (154).
- Rely on one rule, `access::may`, to decide what a person, the assistant, a story's agent and
  the reviewer may do (149, 150).
