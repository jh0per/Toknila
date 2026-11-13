# Claude Code end to end, ninth run

The phase-7 chain, run by real Claude Code agents (Sonnet) inside the owner's installed Toknila.app
0.3.0, on the owner's own TOKNILA project in **local** merge mode, 12 and 13 November 2026. The
chain was written by the orchestrator from the owner's messages of the evening (the assistant at
two levels, the sidebar look, voice, the agile words, the signing identity) and from what the
runs of phase 6 left open (the reviewer's PERSON verdict, a conflict at Accept, lost cards, timing
tests, CI time); six more cards came in during the run from the owner's screenshots and remarks
(the assistant answered a question with a plan; the chat panel is small; epic names such as
`010-midi-controllers`; a card in the wrong epic; "spend today" and a project switcher seen in
another dashboard). Review mode `both`; the chain did not autostart, the orchestrator started each
card when its dependencies had merged; the person accepted after the daemon's check, its reviewer
and the person's own gates on a merge of `main` with the ship in a gate worktree.

Result: every card of the chain reached **Merged** into the local `main`; the chain released
**0.3.1**.

| Card | Title | Hand-ins | Note |
|---|---|---|---|
| TOKNILA-97 | One assistant at two levels | 1 | design thread |
| TOKNILA-98 | The sidebar tells projects, nodes and chains apart; its foot is a menu | 2 | design thread; one hand-in conflict with 105 |
| TOKNILA-99 | The assistant plans epics and releases on the roadmap | 1 | |
| TOKNILA-100 | The assistant listens: speak instead of typing | 1 | design thread; the real-window spike was left to a person, see finding 7 |
| TOKNILA-101 | The reviewer judges only what it can see; the rest is a person's | 1 | |
| TOKNILA-102 | A conflict at Accept returns the card to its agent | 3 | two conflicts with 101 in `review.rs` on the way, one reviewer round, one person round |
| TOKNILA-103 | Three loose ends of waiting and recovery | 1 | |
| TOKNILA-104 | Tests that hold under load and a daemon that stops | 2 | returned once by the reviewer for a missing test; found what held the stop open, see finding 2 |
| TOKNILA-105 | A chain row in the tree shows its name on one line | 1 | |
| TOKNILA-106 | An update keeps what macOS let the app do: one signing identity | 1 | its workflow step was never run and broke Release [main], see finding 4 |
| TOKNILA-107 | The window speaks agile: epics, stories, tasks | 2 | design thread (the word table); returned once by the reviewer for four leftover words |
| TOKNILA-108 | cargo xtask verify fits CI's half hour again | 3 | one hand-in conflict, one reviewer round for missing evidence; proposed a wildcard already fixed on main, see finding 9 |
| TOKNILA-109 | The assistant answers the question it was asked before it plans | 2 | returned once by the person: the owner's project key in an example |
| TOKNILA-110 | The assistant panel reads like a chat | 1 | design thread |
| TOKNILA-111 | An epic has a title a person reads; the slug stays for folders and branches | 1 | |
| TOKNILA-112 | A card moves to another epic; the assistant points out a card in the wrong one | 1 | |
| TOKNILA-113 | Today's spend and the quota windows where the owner works | 1 | |
| TOKNILA-114 | A project switcher in the top bar; the sidebar shows one project's tree | 1 | design thread |

## Setup

- Toknila.app 0.3.0; `claude` 2.1.322; worker and reviewer model `sonnet`; command policy
  `auto`; review mode `both`; check command `cargo xtask verify`; four slots.
- The chain did not autostart (`tok chain new --no-autostart`): a card started by hand once its
  dependencies merged, which kept the planner-instruction cards (99, 109, 107, 111, 112) in one
  file-safe order.
- Pre-Accept gates as in run 8: `gates.sh <ship-branch>` in a persistent gate worktree (merge
  `main` with the ship, `cargo xtask web`, `e2e-web`, `verify`).
- The owner's second project ran on the same daemon; its first real card stayed Blocked
  `runner_lost` all run (a question about a manual phone check nobody answered).

## Paths seen for the first time

- **A conflict at Accept returned the card to its agent** (TOKNILA-102's own feature, on itself
  and on 98 and 108 at hand-in): the agent merged `main`, kept both sides and handed in again, no
  person step, no Blocked card.
- **A card waiting on a design decision freed its slot** (TOKNILA-92 of phase 6): five design-first
  cards waited without holding the pipeline; each resumed within seconds of the approval.
- **A wildcard card proposed by an agent and rejected** (TOKNILA-115 from 108's agent).
- **The assistant's two levels**: the owner asked a feasibility question on the roadmap of their
  second project and got a status of the plan instead of an answer, see finding 6.
- **CI ran the whole suite on Ubuntu and Windows for the first time in a week** and showed what
  the cancelled runs had hidden, see finding 3.

## Findings, ranked by how much they hurt daily use, and what became of them

1. **The assistant planned when it was asked a question.** On the roadmap the owner asked
   whether a device family is hard to support; the assistant answered "Plan as it stands: ..."
   with a next step and never the question. The instructions said "You plan" and, for the
   roadmap context, "Plan the top level" as an order. TOKNILA-109 (merged): the identity is a
   helper who answers, plans when asked and turns a request into a card; a context says what
   may be changed there, not what to do; a picture with "make mockups" becomes a design-first
   card; the composer reads "Ask, or say what to build". The owner's picture of the assistant,
   in their words, is in the planner instructions.
2. **A daemon that would not stop.** The e2e helper printed "did not exit after stop; killing
   it" for a week. TOKNILA-104 found the cause: not the page's WebSocket but an agent's pending
   permission long-poll, which axum's graceful shutdown waits for; the daemon now gives open
   connections one second and leaves.
3. **CI was red for reasons the cancelled runs had hidden.** The Ubuntu and Windows jobs were
   not slow but hung on 93's client test (a macOS-only reset reproduced with a fake server that
   waited forever for a second connection); once past it, four more failures surfaced one by one:
   the same test's assertion that a race goes one way (flaky on macOS), 77's test passing a
   `\\?\` path to git on Windows, 96's keystroke test under ConPTY (and its helpers as dead
   code), and 89's terminal-copy spec meeting CRLF and a shorter scrollback on Windows. Six
   commits on `main` the same night; AGENTS.md and the worker instructions carry the rules
   (TOKNILA-108): a wait in a test has a deadline, an OS-specific test is cfg-gated and says why,
   a test of a race asserts the invariant, waits are for conditions not time. The workflows'
   `concurrency` rule explains every "unexplained" cancellation of the week: a push to `main`
   cancels the run of the previous push.
4. **A workflow step nobody ran broke every Release [main] run.** TOKNILA-106's signature check
   looked for Tauri's bundle path and expected Tauri to sign, but `cargo xtask app` builds and
   signs the bundle itself in `dist/app`. The hand-in said NOT_RUN and the person accepted it on
   that; the next push failed the release. Fixed on `main` (xtask signs with
   `APPLE_SIGNING_IDENTITY` when set, the step checks `dist/app`); AGENTS.md: a step or doc that
   names a path or a command is proven by running it, and "not runnable locally" is a reason to
   return the card.
5. **A browser spec started a real agent.** Since TOKNILA-95 a new project runs `claude-code`
   by default; the planner spec created its project with no `--sim`, so the Start button spawned
   `claude`: missing on CI (the card blocked, the spec failed), present on the developer's Mac
   (a real agent started in every gate run). Fixed; AGENTS.md: a spec that starts a card creates
   its project with `--sim` or a fake profile.
6. **The reviewer earns its round.** It returned 104 for a point without a test, 107 for four
   visible words it grepped, 108 for missing evidence (no list of slowest tests, no CI run named),
   and marked a design thread NOT_RUN on 107 although TOKNILA-101 had put the thread record in
   its instructions (a slip of the model, not of the record). One cost: it cannot call `gh`
   (allow list), so a CI claim is checked by the person.
7. **An agent cannot see a window it opens.** TOKNILA-100 was told to measure the Web Speech API
   in the real window and handed in the recognizer with a fake in e2e, the real run "a person's".
   The orchestrator settled it with a fifteen-line Swift probe (a plain WKWebView has
   `webkitSpeechRecognition`, no `navigator.mediaDevices`); whether the window gets the
   microphone is the owner's first try after 0.3.1. Lesson for cards: a real-window check names
   who performs it and how (a headless probe, a person), never "measure it yourself".
8. **The design decisions were all the orchestrator's.** Five design threads (98, 107, 110, 114,
   100) and one question (106) waited for the owner, who was present early in the evening and
   never answered one; each was approved after thirty minutes on the agent's recommendation.
   Either the owner trusts the defaults or the inbox does not pull them in; worth asking.
9. **A wildcard that duplicated a fix on `main`.** 108's agent found the Windows keystroke
   failure and proposed TOKNILA-115 while `main` already carried the fix from an hour before.
   Candidate for the instructions: before proposing a wildcard, read `git log main` for the
   subject.
10. **The orchestrator's own tools lied twice.** Its API helper printed the first 4000 characters
    of a body, so an inbox with a long hand-in parsed as empty (the "monitor went blind" of run 8);
    and `gh run watch` after six pushes exhausted the GitHub API limit for the owner's account,
    together with 108's polling of its `ci/` branch. Both fixed in the tools; one `gh run view`
    per fifteen minutes from now on.
11. **Load.** Four slots plus the gates pushed the load average past 30 twice; one gate flake
    (the race test of finding 3) came at that load. Nothing else flaked.

## What the owner can do now that was not possible before

Talk to one assistant on the roadmap and inside an epic, and get an answer to a question before
any planning; drop a screenshot with "make mockups" and get a design card; speak instead of
typing; read the window in agile words (epic, story, task) with readable epic titles; move a card
to another epic from its menu, by drag, or by asking the assistant, which points out a card in
the wrong epic; see today's spend and the quota windows in the top bar and a card's cost with its
running time; switch projects from the breadcrumb (Cmd/Ctrl+K) with the sidebar showing one tree;
use a chat panel with room, padding and a composer that grows; keep macOS permissions across
updates once a signing identity is set; and trust a CI that finishes in a quarter of an hour on
Ubuntu, runs its tests with nextest, and uploads its timings.
