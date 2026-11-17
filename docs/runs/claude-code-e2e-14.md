# Claude Code end to end, fourteenth run

One epic, run by Claude Code agents inside the owner's installed Toknila.app 0.3.6 on the owner's
own TOKNILA project in **local** merge mode, on 17 November 2026. The owner agreed to the plan in
two words. The epic is phase 13, "A release that takes its turn, and a Mac app that opens"
(TOKNILA-157 to 160 from `plans/phase-13.toml`). All four cards merged the same afternoon, in
about three and a half hours. Agent time was 3.4 hours in total.

Result: all four cards reached **Merged**; release **0.3.8** goes out next.

| Card | Title | Hand-ins | Note |
|---|---|---|---|
| TOKNILA-157 | Merges and a release take turns: one merge line per project | 2 | `opus`; returned once by the person: in pull-request mode the card held the turn while its pull request waited for GitHub (from its own Left note; the reviewer had passed it), finding 3 |
| TOKNILA-158 | Three loose ends: two tests wait for what they assert, and a round names the check that failed | 1 | found and fixed two more tests of the same shape |
| TOKNILA-159 | A release can run the project's own build step on this computer, after the tag | 1 | the Build step, resumable without redoing the tag |
| TOKNILA-160 | The Mac app is signed with a Developer ID and notarized on the maintainer's Mac, with no secret on GitHub | 2 | `opus`, flow thread first; returned once after an independent read found a defect that would have blocked every release, finding 1 |

## Setup

- Toknila.app 0.3.6; project TOKNILA in local merge mode; three slots.
- The orchestrator handed out every piece of work and chose the model and effort per piece (the
  owner's rule, restated that day). `opus` for the merge line and the signing story, the default
  model for the other two. A small fix to main and both run documents went to delegated helpers
  on the smaller model.
- The project's extra check for `apps/web` and the Accept re-check ran in the daemon. No hand
  gates.
- Phase 13 imported from `plans/phase-13.toml`.

## Paths seen for the first time

- **An approval with decisions, followed point by point.** TOKNILA-151 from phase 11 worked in a
  real run: the hand-in listed how it followed each decision and each of three additions.
- **A card asked one question with choices** about where its tests should live. The answer
  changed the plan: Python tests, but run by `cargo xtask verify` on every push.
- **A wildcard that must wait for an event outside the epic.** TOKNILA-161 removes the CI path and
  every secret name from the release workflow after the first signed release. It was approved
  and parked in a separate manual-start epic, "After the first signed release", so it neither
  starts by itself nor blocks the release.
- **An independent review by a second agent before the person's Accept**, on the story the
  project's check cannot fully test.

## Findings, ranked by how much they hurt daily use, and what became of them

1. **A defect that the check, the reviewer and the hand-in all missed, and that would have
   blocked every release.** Test-only helpers in the new xtask code were used only by tests gated
   to macOS or unix. So clippy with `-D warnings` fails on Linux and Windows CI, and since
   TOKNILA-141 a release publishes only after CI passes. An independent read found it: it copied
   the crate, flipped the cfgs to imitate each system and ran clippy. The same reader confirmed
   the fix the same way, with a control. This is the third time in three days that OS-gated test
   code broke another system (after the `check_line` import and the first signing commit). The
   card added an AGENTS.md rule: when a test is gated by OS, its helpers and imports carry the
   same gate. Candidate kept in the notes: the project's check cannot see other operating
   systems, so a hand-in that adds OS-gated code could be flagged for the person.

2. **Six smaller holes in the release path, found by the same read and fixed in the same
   round.** The tag was not checked on GitHub before two builds and two notarizations. A rerun
   could overwrite the files of a published release. A half-uploaded set of files could be
   published. One GitHub error ended a 60-minute wait. The mode's spelling was compared two ways.
   A debug print could have shown the updater password.

3. **The reviewer agent approved 157 with a throughput regression written in the hand-in's own
   Left note** (pull requests merging one per CI run). The person read the note and returned the
   card. Lesson for briefs and for the reviewer: a Left note that names a behaviour change is a
   point to judge, not a footnote.

4. **0.3.7 was tagged but not published for a short time before this run.** CI on the bump
   commit failed on two tests that read an agent's output once. The release's wait step refused
   to publish and named the run and the jobs. A rerun passed. TOKNILA-158 fixed those two tests
   and two more.

5. **What only the maintainer's Mac can prove is NOT_RUN in 160's hand-in:** signing with the
   real certificate, Apple accepting the submission, the stapled tickets, the microphone under
   the hardened runtime, and the first update from the ad hoc app to the signed one.

## What the owner can do now that was not possible before

- Accept cards during a release without breaking it: they merge after the tag (157).
- Accept several cards and have each re-check once, in order (157).
- Read, in a failed round, which check failed (158).
- Give a project an artifact command and have the release run it after the tag, with "Build
  again" on a failure (159).
- Run `cargo xtask mac-release` to build, sign, notarize, staple and upload the Mac app from their
  own Mac, with settings that are names in a file outside the repository and no value printed
  anywhere (160).
- Keep releasing as before until they switch one repository variable (160).
- After the first signed release, delete the two signing secrets from GitHub (160, 161).
