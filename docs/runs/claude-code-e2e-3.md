# Claude Code end to end, third run

Card TOKNILA-23 of the phase-2 chain. Two phase-2 cards were run by real Claude Code agents
(Sonnet, through the per-role model setting) inside the owner's installed Toknila.app 0.2.4, on the
owner's own TOKNILA project in **local** merge mode. The orchestrating session played the person
through Toknila's own surfaces only: the inbox and answer routes of the HTTP API, and `tok`
(`start`, `comment`, `return`, `accept`, `unblock`). The aim was to see the paths runs 1 and 2 had
not: a Deny, a Return round, an instruction delivered mid-run, and a crashed agent brought back.

Result: both cards reached **Merged** into the local `main`.

| Card | Title | Hand-ins | Merged (local) |
|---|---|---|---|
| TOKNILA-17 | tok watch prints when things happened | 2 (one Return round) | `ab30827` |
| TOKNILA-18 | Profile errors point at the right line | 1 | `b070782` |

## Setup

- Toknila.app 0.2.4 (`main` at `8910f7a`), the owner's real data home; `claude` 2.1.319.
- Project TOKNILA: merge mode `local`, base `main`, parallel limit 2, review mode `human`,
  `check_command` = `cargo xtask verify`, worker and reviewer model `sonnet`, built-in
  `claude-code` profile.
- The owner's working copy `~/Work/Toknila` was level with `origin/main` when the cards started.
- Permission requests were answered by the person from the inbox; anything heavy or outside the
  card was to be denied with a reason.
- Four other agents built and tested in worktrees of the same repository on the same machine
  during the run (finding 4 and 7).

## Timeline

Local times (UTC+2), 8 November 2026.

| Time | What | Who |
|---|---|---|
| 06:47 | both cards started, ships created, runners up | person (Start), system |
| 06:47:34 | first permission request of TOKNILA-18: a `python3` heredoc editing `parse.rs` (finding 2) | agent |
| 06:47:43 | first permission request of TOKNILA-17, the same shape | agent |
| 06:48 to 06:54 | every `cargo` call asked permission: each was prefixed with `export PATH="$HOME/.cargo/bin:$PATH"` after the first call failed with exit 127 (finding 1) | agent, person (Allow) |
| 06:54:34 | TOKNILA-17 `finished`: Running -> Review | agent, system |
| 06:56 | **Return** with two precise remarks (a doc comment typo, `chrono` as a workspace dependency); card back to Running | person |
| 06:57 | instruction comment to both cards: use the Edit and Write tools, not shell edits; delivered | person, system |
| 07:05:07 | TOKNILA-17 asked to make a worktree of `HEAD~1` and run the whole workspace test suite three times to prove a flaky test was not its doing; the inbox text was cut at 300 characters, the full command was read from the agent's transcript (finding 6) | agent |
| 07:05:30 | **Deny** with a reason (the flaky test belongs to another card; three full runs would load the machine); the agent read it and cleaned up | person, agent |
| 07:09:06 | both agents died with exit 143 within 0.3 s: another tool on the machine ran `pkill -f xtask`, and the agents' command line carries the check command (finding 4); cards Blocked "crashed: exit 143" | outside |
| 07:16 | **Unblock** both; new runs started on the same ships with the committed work intact | person, system |
| 07:21:15 | TOKNILA-17 `finished` again: both remarks done plus a pinned snapshot of the plain `watch` line | agent |
| 07:21:36 | TOKNILA-18 committed and `finished` | agent |
| 07:22:12 | **Accept** TOKNILA-17; merged into local `main` by the daemon | person, system |
| 07:23:21 | **Accept** TOKNILA-18; merged | person, system |

About 36 minutes from Start to the last merge, of which 7 were the crash and restart.

## Paths seen for the first time

- **Deny with a comment.** The agent received the reason, stopped the plan it had, cleaned up the
  worktree it had started to make, and handed in.
- **Return round.** The returned comment reached the agent as an instruction; both remarks were
  done exactly, and the second hand-in added a test the first lacked.
- **Instruction mid-run.** Delivered and marked delivered; the agent acknowledged it. The run
  that replaced it after the crash did not see it again (finding 5).
- **Crash and Unblock.** Two runs killed from outside; `Unblock` started fresh runs on the same
  ships; nothing committed was lost; uncommitted edits of TOKNILA-17's second round were redone.
- **Local merge on Accept.** Each accepted ship merged into `main` of the working copy with a merge
  commit naming the card.

Still not seen in a real session: `finished` refused for an uncommitted ship, a reviewer
permission prompt (review mode was `human`), a pull request with no checks (local mode).

## Findings, ranked by how much they hurt daily use

1. **Agents cannot find `cargo`.** The daemon's PATH, built from the login shell, has no
   `~/.cargo/bin` on this machine, so the first `cargo` call failed (exit 127). Both agents then
   prefixed every command with `export PATH=...; cd "<ship>";`, which defeats the allow rule
   `Bash(cargo xtask verify *)`: about twenty permission requests for two small cards, almost all
   of them test runs. Fixed in P2-08: the runner appends the per-user tool folders that exist, the
   system check verifies the check command resolves, and the instructions say to run the check
   command exactly as given.
2. **Agents never used the Edit tool.** A transcript showed ToolSearch 1, Bash 8, Read 6, Edit 0;
   files were changed with `python3`, `sed` and `perl` heredocs, each asking permission under
   `acceptEdits`. P2-08 adds the instruction to change files with Edit and Write.
3. **Ships were made under the data home** (`~/Library/Application Support/toknila/ships/...`, a
   path with a space), not `~/.toknila-ships`: the shell passed `TOKNILA_HOME` explicitly and the
   daemon treated it as a side instance. Fixed in P2-08.
4. **An agent's command line carries the check command**, so a `pkill -f xtask` from another tool
   on the machine killed both agents. Toknila recovered well (Unblock restarted them), but the
   card said "crashed: exit 143"; a signal exit should say the agent was terminated from outside
   and that Unblock restarts it.
5. **A restarted run does not get the instructions delivered to the run before it.** The Edit-tool
   instruction was delivered once; the run after the crash went back to shell edits. Recent
   instructions should be part of the kickoff of a replacement run.
6. **The inbox cuts a permission request at 300 characters.** For the one request that deserved a
   Deny, the decision could not be made from the inbox text; the full command was read from the
   agent's transcript. Show the full command behind a disclosure.
7. **Test suites leak daemons.** Interrupted or parallel `cargo test` runs left up to 150
   `tok daemon` processes alive (the daemon tests spawn one each); with several workspaces testing
   at once the load average reached 450 on 12 cores. The daemon tests need a guard that stops
   their daemon on drop and on test-binary exit (card TOKNILA-19), and Toknila could run the check
   command at a lower priority.
8. **Agents re-verify after handing in.** TOKNILA-17 asked to run `cargo xtask verify` seven
   seconds after `finished`; harmless, but a wasted run and a wasted prompt.

Findings 1 to 3 are fixed on `main` after this run; 4 to 8 are open.
