# Claude Code end to end, fifteenth run

One epic, run by Claude Code agents inside the owner's installed Toknila.app on the owner's own
TOKNILA project in **local** merge mode, on 18 and 19 November 2026. The epic is phase 14, "Ready
for strangers" (`plans/phase-14.toml`, version 0.3.9). It started with two stories and ended with
nine: one added for the licence, two added by the owner's requests during the run (sounds, and the
limit on agents), two proposed by agents as wildcards and approved (TOKNILA-165, 168), and two
added from findings of the run itself (169, 170).

Result: all nine stories reached **Merged**; release **0.3.9** was published on 19 November 2026
at 15:17 UTC. The release ran Check, Bump, Tag and Push without a retry. CI on the pushed main
(`501aee0`) and on the release commit (`7a63d43`) was green on Ubuntu, Windows and macOS on the
first attempt. It is the first release in this series that needed no rerun of a Windows job. The
updater signature of the Apple silicon archive and its SHA-256 were verified.

Cost at API prices was about $71.82 in total. The two `opus` stories are $62.56 of it.

| Story | Title | Model | Cost | Messages | Minutes | Sent back |
|---|---|---|---|---|---|---|
| TOKNILA-162 | Windows CI holds: terminal lines, slow daemon start | `opus` | $32.58 | 331 | 671 | no |
| TOKNILA-163 | README and install guide for a stranger, four pictures from the e2e screenshot tool, a dead-link check | default | $2.93 | 104 | 69 | 3 times, by the reviewer agent |
| TOKNILA-164 | Licence: PolyForm Shield 1.0.0, a contributor agreement, a CLA check, CONTRIBUTING, issue forms | default | $1.34 | 43 | - | once, by the person, finding 5 |
| TOKNILA-165 | A daemon test no longer waits 300 s for the pty lock it holds itself | default | $0.53 | 21 | 7 | no |
| TOKNILA-166 | Three sounds made in code, switches in machine settings, off by default | default | $2.16 | 72 | 24 | no |
| TOKNILA-167 | "Agents at once" is a limit that holds on every path that starts an agent | `opus` | $29.98 | 372 | 502 | once, by the person, after an independent read, finding 3 |
| TOKNILA-168 | Importing stories already done starts no agent and makes no ship | default | $0.51 | 30 | 10 | no |
| TOKNILA-169 | The reviewer agent is shown what the person told the story's agent | default | $1.15 | 51 | 10 | no |
| TOKNILA-170 | The assistant chat's suggestion line shows with its story key | default | $0.64 | 32 | 21 | no |

## Setup

- Toknila.app on the owner's Mac; project TOKNILA in local merge mode.
- Phase 14 imported from `plans/phase-14.toml`. Seven further stories were added during the run.
- Agent time includes two sleeps of the laptop, finding 6.

## What each story did

- **TOKNILA-162.** Two layers lost lines on Windows. ConPTY lost the head of the output (lines
  1 to 22 in 2 of 10 probe runs) when the screen was shrunk while the agent still printed. The
  runner now holds a smaller size on Windows until the output is quiet (300 ms, at most 2 s); the
  record was complete in 12 of 12 runs. The page's terminal lost a line after its panel changed
  size (line 70 in 18 of 18 runs): the page resizes at once and the console is told about 0.15 s
  later. A one-setting fix (the terminal library's Windows mode) was not enough. The proper fix,
  that the page takes its size from the daemon, was left for a later story by the person's
  decision and is documented in `docs/install.md`. The copy spec asserts a stated bound on Windows
  only. A slow daemon start took up to 13.97 s under the parallel test suite with the virus
  scanner off, against a 15 s wait. Tests now wait 60 s through a test-only setting; a person's
  wait stays 15 s. The agent used seven of eight allowed pushes to a `ci/` branch and saw four
  green Windows attempts in a row on the last one.
- **TOKNILA-163.** README and install guide for a person who has never seen Toknila, four
  pictures made by the e2e screenshot tool, and a check for dead links.
- **TOKNILA-164.** PolyForm Shield 1.0.0 verbatim (its SHA-256 is checked by a test), a
  contributor agreement adapted from the Apache ICLA V2.2, a CLA check on pull requests pinned to
  a commit, CONTRIBUTING and issue forms. Versions up to 0.3.8 stay under the Elastic License 2.0.
- **TOKNILA-165.** A daemon test waited 300 s for the pty lock it held itself (a second rig was
  made while the first was alive). On Windows the test went from 300.2 s to 0.331 s and the suite
  to 689 s.
- **TOKNILA-166.** Sounds for a story done, an epic finished and something that went wrong.
  Not proven: sound while the window is behind other windows or minimised; that is for the person
  to hear.
- **TOKNILA-167.** The limit "Agents at once" holds on every path that starts an agent. A story
  that must go on waits first in line and says so.
- **TOKNILA-168.** Importing stories that are already done starts no agent and makes no ship.
- **TOKNILA-169.** The reviewer agent sees the person's instructions and answered questions.
- **TOKNILA-170.** The suggestion line of the assistant chat showed without its story key until
  the page knew the card. It is a page defect, reproduced with a delayed response, first seen as
  one failure of a browser spec on the macOS CI runner.

## Findings, ranked by how much they hurt daily use, and what became of them

1. **An agent that waits on hosted CI is expensive.** TOKNILA-162 ran eleven hours, most of it
   waiting for Windows runs of about an hour each. Every look at a run re-read about 357 thousand
   tokens of context: at that point it was 324 messages and 115.7 million tokens processed, 99.5 %
   of them cache reads. The owner noticed. After that the briefs said: find the cause by reading
   and by logs that already exist, at most two pushes, hand in right after a push and mark the
   Windows result NOT_RUN; the person reads the run before accepting. TOKNILA-165 under those
   rules handed in in nine minutes.

2. **The limit on agents did not hold.** The owner saw "2 / 1": a story sent back by the reviewer
   started at once while another ran, which the architecture document allowed in so many words.
   TOKNILA-167 changed the rule.

3. **The built-in reviewer approved TOKNILA-167, and an independent read found three defects.**
   The read was done by a separate agent on the `opus` model, read-only. A process that ends by
   itself freed its slot without a schedule pass (a stalled line at limit 1). A reviewer that was
   due was dropped when the daemon restarted. A story blocked by hand kept its agent alive but
   gave its slot away. The read also confirmed that data stored by 0.3.8 still reads. The story
   was sent back, fixed all points with tests seen red first, and found one more path itself (a
   simulated start on a project with a real agent).

4. **The reviewer could not see what the person told the agent.** In TOKNILA-162 it wrote that
   the person's answer was one "which I cannot see". In TOKNILA-163 it failed the story for
   following the person's instruction about the licence wording. TOKNILA-169 fixed this.

5. **A brief that says "keep the wording" keeps wording that does not fit.** One sentence about
   nonprofit status and bylaws, which fits only a foundation, was left in the contributor
   agreement. The agent flagged it; the person removed it (TOKNILA-164, sent back once).

6. **The laptop slept twice during the run**, the first time for about three and a half hours.
   Every agent stopped. The times above include that.

## Left open

- On Windows the page's terminal may lose up to two lines and repeat up to a screen after a
  resize. The record is complete. The design is in TOKNILA-162's hand-in.
- A reviewer process of an earlier round that ends by itself while the next round's reviewer is
  due still sends the card to a person.
- The waiting words do not name which story holds the agents.
- `tok show` prints the first rounds of a review after a person's Return instead of the latest.
- The pty lock test failed once under load in a full local check.
- The Mac app is still not notarized. The owner's signing steps are pending; TOKNILA-161 waits
  for them.
- Sound behind other windows is unproven.
