# Architecture

Toknila is a Rust workspace with a pure domain core, a long-running daemon that owns state, and
thin clients (CLI, web page, desktop shell) that talk to the daemon over HTTP and WebSocket on
`127.0.0.1`. This page is the target shape of the code and the rules that keep it there. Where
the code does not match it yet, the refactoring plan below says which step fixes it.

Decision records: [ADR 0001](adr/0001-layered-architecture-and-mvi.md),
[ADR 0002](adr/0002-wildcard-cards.md) (wildcard cards),
[ADR 0003](adr/0003-card-threads.md) (threads on a card).

## Layers

Dependencies point inward, towards the domain. A layer may use the layers below it, never the
ones above. `cargo xtask guards` checks the rules it can check mechanically.

```
 presentation   crates/cli          apps/web            apps/desktop
                    |                   |  HTTP + WS        | sidecar
 client/protocol toknila-client  ---HTTP--->  toknila-daemon (adapters)
                    |                   \           |
                 toknila-api-types   <---+     toknila-app (use cases, ports)
                    |                               |
 domain          toknila-core  <--------------------+
```

| Layer | Crate | Holds | Must not hold |
|---|---|---|---|
| Domain | `toknila-core` | Entities, the ticket state machine, the chain graph (with wildcard cards: proposed work outside the plan until approved), chain files, the comment template, typed domain events, the agent event mapping, threads on a card (`thread`: open, post, approve, the hand-in gate, the computed phase), agent profiles (`profile`: parse and validate a profile file, the catalog of built-in and file profiles), parallel agents (`schedule`: slots, the queue, who starts next), the system checks and the folder check of a project (`system`, `repo`: what the tools said and what is wrong, as data), the inbox (`inbox`: what waits on a person, and the leftover decisions), entity constructors and defaults | I/O, the clock, id generation, async runtimes, storage, transport |
| Application | `toknila-app` | Every use case (`projects`, `chains` with import and export, `tickets` with typed `TicketAction`, `wildcards` (proposals), `threads`, `releases` with `ReleaseAction`, comments and subtasks, agent events), the chain unit of work (`with_chain`: change, settle, save, journal, deliver, then start the cards the schedule gives a slot), reference resolution, startup recovery, `AppError`, and the command inputs (`NewTicket`, `NewChain`, ...) the API accepts as request bodies. Ports: `Reader` and `UnitOfWork` (which carries the command's clock and actor); a `Runner` port arrives with real runners in phase 1 | `axum`, `rusqlite`, `tokio`, HTTP status codes |
| Protocol | `toknila-api-types` | Read models (`ChainDetail::from_view`, `TicketDetail`, list items), `ErrorBody`, `EventRecord`, query and small request types; re-exports the command inputs of `toknila-app` and the agent events of core. OpenAPI is generated from them (feature `openapi`), TypeScript types from OpenAPI | Logic, transport |
| Adapters | `toknila-daemon` | `http/` (`routes/*`: one module per resource, each handler parses, runs one use case, answers; one `ErrorBody` for every failure including 401), `server/` (router, auth, lifecycle, logging), `store/` (SQLite implementation of the ports), `stream`, `runner/` (sim now, PTY in phase 1), lifecycle (`daemon.json`, signals), paths, `profiles` (reads the profiles folder), logging, embedded web assets | Business rules |
| Client | `toknila-client` | Typed blocking HTTP client of the API: one method per route, `ClientError` carrying the daemon's `ErrorBody`, the stream address | Output formatting |
| Presentation | `crates/cli`, `apps/web`, `apps/desktop` | Argument parsing and output (CLI); MVI screens (web); the Tauri 2 shell that starts or attaches to the daemon | Domain rules |

A rule that two places need moves one layer down; it is never copied.

## Data flow

Commands and state travel separately.

1. A client sends a command (`POST /v1/...`).
2. The handler turns the DTO into an application command and calls one use case.
3. The use case opens a unit of work on one chain; the domain (`machine::apply`,
   `ChainView` graph rules) returns the new state and its effects.
4. The store adapter saves the state and journals typed domain events in the same SQLite
   transaction. Nothing commits half.
5. After the commit the events are broadcast; `/v1/stream` sends them to every client.
6. The handler answers with the fresh state.

Reads do not go through use cases with side effects: query functions build read models
(chain detail, ticket list). The database is the source of truth; computed values (chain status,
progress, where a chain stopped) are never stored, here or in a client.

## Agent profiles

A profile says how to launch one agent: `command` (an argv list, never a shell string), `env`,
`role` (`worker`, `reviewer` or `release`), `instructions_file`, `instructions_flag`, `mcp` and `hooks` wiring, `output`
rules and `nudge` thresholds. The working directory is not in it: the ship supplies it at launch.
A new agent is a new file, not new code.

- Built in: `sim` (starts no process; a person sends its events with `tok sim`) and `claude-code`
  (launches `claude` with the MCP server and the hooks wired in, see "Agent access" below), and
  their reviewers `sim-reviewer` (no process; a person sends its verdict with
  `tok sim <ticket> review`) and `claude-code-reviewer` (the same launch, role `reviewer`).
  The reviewer of a worker profile `x` is named `x-reviewer` by convention, unless a review
  setting names another (see "Review" below).
- Files: `<name>.toml` in the `profiles` folder of the data home (`Paths::profiles()`):
  `~/Library/Application Support/toknila/profiles` on macOS, `%APPDATA%\toknila\profiles` on
  Windows, `~/.toknila/profiles` on Linux, or `$TOKNILA_HOME/profiles`. The file name is the
  profile name (lowercase letters, digits, `-`, `_`). A file named like a built-in profile is
  rejected, not an override. The format is documented at the top of `crates/core/src/profile/mod.rs`.
- Every `GET /v1/profiles` reads the folder again, so an edit applies at once. Core validates
  (`profile::parse`, typed `ProfileError` with line and column) and builds the catalog; the daemon
  only reads the files. An invalid file does not hide the others: the answer lists the usable
  profiles and the problems (`file`, `line`, `column`, `message`), and `tok profile ls` prints
  the problems on stderr as `path:line:col: message`.

## Ships

A ship is the git worktree and branch a ticket works in. Core decides: `machine::apply` emits
`PrepareShip` (with `EnsureRunner`, on Start, Unblock into Running and Return) and `RemoveShip`
(when the merge landed, see [Merging](#merging)), and `ShipSpec::for_ticket` names the branch `ship/<chain>/<slug>` (slug: the ticket
key and title, path-safe). The daemon implements the `Shipyard` port (`toknila-app`) with `git`
(`shipyard.rs`: `std::process::Command` with an argument list, no shell, through the small `git.rs`
runner that the merger shares; a missing `git` is `ShipError::GitMissing`).

**After-commit effects.** Nothing external runs inside a unit of work. The use case queues an
`ExternalEffect` (`UnitOfWork::defer`); the store commits the command, runs the queue on a
blocking thread (`external::execute`, no database), then opens a second transaction that
`external::apply`s each `ExternalOutcome`: the ship row and journal, and a `TicketEvent` through
the machine (`ShipReady` tells the ticket its branch, `Block(ShipFailed)` blocks it). A ticket
that moved on meanwhile refuses the event and the outcome is dropped. The command's own answer
is the committed state, so the ship shows up in the next read and on the stream. The runner
(below) uses the same path.

- Needs a project with `repo_path` (`tok project new KEY --repo <path> [--base develop]`; the base
  is the project's `work_branch`, default `develop`). Without a repo the ticket runs without a
  ship, as sim tickets do.
- Worktrees live under `<home>/ships/<project>/<chain>/<slug>`. The path is stored in the `ships`
  table (one row per ticket), so a daemon restart keeps track; `TicketDetail.ship` exposes it and
  the journal has `ticket.ship_created`, `ticket.ship_removed` and `ticket.ship_remove_failed`.
- Idempotent: an existing worktree is reused, an existing branch is checked out as it is, a
  worktree deleted by hand is pruned and made again.
- Merged removes the worktree after the commit (forced; the work is merged) and keeps the
  branch. A removal that fails does not undo the accept: the journal gets `ticket.ship_remove_failed`
  and the path stays recorded.
- Cancelled leaves worktree and branch alone: nothing a person may still want is destroyed;
  cleaning up cancelled ships is a manual step for now.
- A ship that cannot be made moves the ticket to Blocked with `ship_failed` (details from git)
  through the machine; `tok unblock` retries.

## Merging

Accept does not move a card straight to Merged: the work has to land in the project's base branch
(`work_branch`, default `develop`) first.

- **State.** There is no new status. Accept (a person's, or the reviewer agent's approving
  verdict in `agent` mode) records the round and leaves the card in Review with
  `review.turn = merging` and emits the `Merge` effect. A merging card takes no second `accept`
  and no `return` (the table refuses them, so `allowed_actions` hides them; `cancel` still
  works). The worker's runner stays alive until the merge lands, so whoever has to fix a
  conflict can do it on the ship.
- **Effect and outcome.** `app::merges` queues `ExternalEffect::Merge` (a `MergeSpec` from
  core: repo, base, the ship's branch, and the message `Merge <branch>: <KEY> <title>`). After the
  commit the host runs it through the `Merger` port (`toknila-app`; the daemon's `GitMerger`
  implements it, a pull-request mode can implement the same port) and the outcome returns
  through the machine: `MergeDone` makes the card Merged (and then `RemoveShip` removes the
  worktree and keeps the branch, `StopRunner`, and dependents become ready); anything else is
  `Block` from Review with a typed reason. A card without a ship (no repository) has nothing to
  merge and the host answers at once. The accept's HTTP answer is the committed state (merging);
  the outcome follows on the stream.
- **Reasons.** `merge_conflict` (details name the conflicting files), `base_checkout_dirty`
  (names the path of the checkout and what to do) and `merge_failed` (git missing or older than
  2.38, a branch missing, the base kept moving). **Unblock retries the merge**; the person or the
  agent resolves a conflict on the ship first (merge the base branch into it, fix, commit).
  A daemon restart while a card is merging queues the merge again (`merges_resumed` in the
  recovery report); repeating a merge that landed is harmless (`UpToDate`).
- **The person's checkout is never disturbed.** `GitMerger` needs `git >= 2.38` and works
  without a working tree: `git merge-tree --write-tree --name-only -z <base> <ship>` gives the
  merged tree or the conflicting files, `git commit-tree -p <base> -p <ship>` writes the merge
  commit (two parents, so history shows the card; a machine without a git identity gets a
  fallback one for this commit only). Then the base moves, safely:
  - the base branch is checked out in some worktree (`git worktree list --porcelain`; usually
    the person's own): if tracked files there have local changes the merge stops with
    `base_checkout_dirty` and nothing changes; otherwise `git -C <it> merge --ff-only <new>`
    moves branch and files together (git itself refuses, changing nothing, if an untracked file
    is in the way, which is reported the same way);
  - checked out nowhere: `git update-ref refs/heads/<base> <new> <old>`, a compare-and-swap.
  - If the base moved after the merge was computed (the fast-forward is no longer one, or the
    compare-and-swap fails), the merge is **computed again** from the new tip, up to 3 attempts,
    then `merge_failed` (unblock tries again). It is cheap and nothing was changed meanwhile.
- **Dependents** start after the merge landed, and a ship is made with `git worktree add -b
  <branch> <path> <base>`, so it branches from the new base tip.
- Limit: a `cancel` that races a running merge cannot stop git; the merge may still land.

## Runners

A runner is the agent's process, started in a pseudo-terminal in the ticket's ship. Core decides
when (`EnsureRunner` on Start, Unblock into Running and Return; `StopRunner` when the merge lands, on Cancel,
Pause and every block that ends the agent; `StartReviewer` and `StopReviewer` for the reviewer,
see "Review"). The application queues `StartRunner`, `StartReviewer`, `StopRunner` and
`StopReviewer` as external effects; the daemon implements the `Runner` port (`toknila-app`) in
`runner/` with `portable-pty` (Unix ptys, Windows ConPTY), which no other crate depends on.

A ticket has at most one live runner **per role**: its worker, and during review a reviewer next
to it on the same ship. `Runners` keys them by ticket and role; `start` keeps the live runner of
the same role, `stop` ends every runner of the ticket, `stop_role` one. The reviewer's token, log
(`runs/<ticket>/reviewer/<run>.log`) and output are its own. The ticket's routes below mean the
worker; the reviewer's output is read, never written, with `?role=reviewer`.

- **Profile.** The ticket's own `profile`, else the project's `default_profile` (`Project::profile_for`;
  `tok project new --profile`, `tok ticket new --profile`). `sim` has no command and starts no
  process, exactly as before. Any other profile is looked up in the catalog when the runner
  starts, so an edit of a profile file applies to the next start. The command is an argument list;
  `env` is added to the daemon's environment.
- **Where.** After the ship is ready, with the ship's worktree as working directory. A project
  without a repository has no ship and so no directory: the runner is refused with a typed error
  (`RunnerError::NoWorkingDir`) and the ticket is Blocked(`crashed`, with that text); use `sim` or
  give the project a repository. A failed ship starts no runner (the ticket is blocked by that).
- **Ensure, not restart.** Starting a ticket whose runner is still live keeps it (`Return` after
  `finished`: the agent may still be there). A stop kills the process; the exit is journaled but is
  not a crash.
- **Exit.** The process's end comes back as `runs::ended` in one unit of work: it journals
  `ticket.runner_exited` (`{run, exit: {code, signal}}`) and, if the ticket's agent was still
  working (Running or Waiting) and the host did not stop it, feeds the machine
  `Block(crashed)`: non-zero code and signals say so (`exit 101`, `killed by signal ...`), and a
  zero exit without a `finished` event is a crash too (`exited without a finished event`), since
  the work cannot be taken as done; `tok unblock` starts the runner again. After the hand-in
  (Review) an exit changes nothing.
- **Lost runners.** Startup recovery runs `runs::mark_lost` before it settles: every Running or
  Waiting ticket whose agent is a process (not `sim`) goes to Blocked(`runner_lost`) through the
  machine and is journaled as an ordinary status change. At startup no process is live, because
  none outlives the daemon: a graceful shutdown kills the runners, and a hard kill closes their
  terminals. `tok unblock` starts the agent again in the same ship.
- **Output.** Raw bytes, ANSI kept. Each run has a log file `<home>/runs/<ticket id>/<run id>.log`
  (all of its output; never read back by the daemon except for a ticket it did not run since the
  last start) and a bounded in-memory scrollback of the last `output.scrollback_lines` lines of
  its profile. Offsets count bytes from the start of the run, so they match the log file.
  A log is capped at the profile's `output.max_log_bytes` (default 20 MiB): when the next write
  would pass the cap the file becomes `<run id>.log.1` (replacing an earlier one) and a new
  `.log` starts, so a run keeps its last one to two caps. `<run id>.base` records the offset of
  the first byte of the current `.log`, which keeps offsets the run's own after a rotation and a
  restart. The scrollback is in memory and is not touched by a rotation. A ticket keeps the logs
  of its 20 newest runs; older ones are deleted when a new run starts.
  Journal entries: `ticket.runner_started` (`{run: {id, ticket_id, key, profile, dir}}`) and
  `ticket.runner_exited`; output itself is not journaled.

| Route | What |
|---|---|
| `GET /v1/tickets/{ticket}/runner/scrollback[?role=reviewer]` | The scrollback of the newest run (the worker's, or the reviewer's with `role=reviewer`) as `application/octet-stream`, with headers `x-toknila-run`, `x-toknila-offset` (of its first byte) and `x-toknila-end` (after its last byte). `404` if the ticket never ran |
| `GET /v1/tickets/{ticket}/runner/stream?run=<id>&from=<offset>[&role=reviewer]` | WebSocket (token as `?token=`). Binary frames: 26 ASCII characters of the run id, the 8-byte big-endian offset of the payload, the raw bytes (at most 64 KiB). Text frame `{"type":"ended","run":"...","exit":{"code":0,"signal":null}}` once the run's output is complete (`exit` absent for a run found on disk after a restart). Without `run` and `from` it starts at the first retained byte, so a late client sees history first; with them it continues after the bytes the client has (scrollback `end`). A new run of the same ticket shows up as frames with another run id |
| `POST /v1/tickets/{ticket}/runner/size` | Body `{"cols": n, "rows": n}`: the size of the panel showing the terminal; the agent's terminal follows it (window-size signal), within 20x5 and 500x200 (`toknila_core::instructions::terminal::clamp`; a terminal starts at 160x40). A person's action; `409` when no runner is live. The web terminal sends it after the panel has been still for 150 ms, and again when the stream goes live |
| `POST /v1/tickets/{ticket}/runner/input` | Body: bytes typed into the terminal as they are (`\r` is Enter on every platform). A person's action: the route acts as a person, and `toknila_app::runs::send_input` refuses any other actor. `409` when no runner is live |

The frame format is `toknila_api_types::runner`; `/v1/stream` stays journal-only.
`tok logs <ticket> [--follow] [--reviewer]` prints the scrollback and, with `--follow`, continues
on the stream until the run ends.

## Review

A card handed in (`finished`) goes to Review and waits for a verdict. Who gives it is the review
**mode**, resolved ticket, then chain, then project (`toknila_core::resolve_review_mode`; default
`human`). Set it with `tok review-mode <human|agent|both> [--ticket K | --chain C | --project P]`
(`PATCH /v1/tickets/{t}`, `/v1/chains/{c}`, `/v1/projects/{p}`); the application resolves it when
the agent hands in and passes it to the machine in the `Finished` event.

| Mode | The card waits for | Then |
|---|---|---|
| `human` | a person | `accept` merges (see [Merging](#merging)); `return` sends the comment back |
| `agent` | the reviewer agent | its `approve` merges (same), its `request_changes` returns the card with its remarks as the instruction |
| `both` | the reviewer agent, then a person | the agent's verdict only records a round; the person then decides with its remarks in view |

- **Rounds.** `ticket.review.rounds` is one flat list, one entry per verdict, numbered from 1
  (`{number, reviewer: human|agent, verdict: approve|request_changes, comment, decided_at}`), so in
  `both` mode the agent's verdict and the person's are two rounds. `ticket.review.turn` says
  whose verdict the card waits for (`person`, or `agent` with `decides`; `merging` once accepted
  and the merge is running), `null` outside Review.
  `TicketDetail.review` adds the resolved `mode` and `reviewer` profile; `TicketDetail.allowed_actions`
  lists what a person may do now. Core computes it (`TicketAction::allowed`: each action tried
  against the transition table), so no client keeps a copy of the rules.
- **A person may always decide**, also while the reviewer is still running: `accept` or `return`
  stops the reviewer (`StopReviewer`) and records the person's round. `accept` takes optional
  remarks (`tok accept <ticket> [comment]`).
- **The reviewer** is a runner of role `reviewer` next to the worker's, started when the card
  enters Review in `agent` and `both` mode (`StartReviewer`), in the worker's ship. Its profile is
  the first of the ticket's, the chain's and the project's `reviewer` setting (`tok review-mode
  --reviewer <profile>`; blank clears it), else `<worker profile>-reviewer`. It is one-shot: its
  token (role `reviewer`) may send only `review_verdict`, then it exits. A reviewer that cannot
  start, or ends while the card still waits for it (no verdict), or is lost with a daemon restart,
  hands the card to a person (`ReviewerFailed`; a report comment says why). The simulated
  reviewer (`sim-reviewer`) has no process: a person sends `tok sim <ticket> review
  approve|request-changes [comment]`.
- **The worker is not replaced.** A worker session that is still alive in Review keeps its
  context: a `return` (a person's, or the reviewer's remarks) goes to that same session (the
  runner is kept, and the comment is an instruction comment like any other: it is typed to the idle session or claimed by its Stop hook, and marked delivered once, see `app::instructions`; the reviewer's remarks take the same path). If the
  worker's process has exited meanwhile (which changes nothing in Review), the return starts a new
  worker run on the same ship. `cancel` stops every runner of the ticket at once; `accept` does so
when the merge lands.
- **Parallel limit.** Slots are counted over runners, not tickets. `schedule::plan` stays pure in
  core and takes the live runners as input (`Reader::live_runners`, which the store fills from
  `Runner::live_runners` when a read or command begins): a card holds a slot while it is Running
  or Waiting, and each live runner of a card in Running, Waiting or Review holds one too, counted
  once. A card in review with its worker and its reviewer alive uses two. A merged, cancelled or
  blocked card's runners are being stopped by the same command, so they already count as free.

## Settings and onboarding

Everything needed to set a project up is possible in the window; the CLI stays a second way in.
The web shows what the daemon says and keeps no rule of its own.

- **Tools.** The daemon runs `git`, `claude` and `gh` through the `Tools` trait (`daemon/src/tools.rs`):
  an argument list, no shell, a timeout of 10 seconds each, the process killed when it hangs. The
  answer is a `toknila_core::system::Probe` (missing, timed out, failed, or ran with code and
  output). `AppState.tools` is the production runner (`SystemTools`); tests inject a fake, so no
  test needs `claude` or `gh` installed.
- **System checks.** `GET /v1/system/checks` runs the probes in parallel and core judges them
  (`system::git`, `claude`, `gh`): git needs 2.38 for `git merge-tree --write-tree`; `claude` must
  answer `--version`; `gh` must answer `--version` and `gh auth status`. Each check has a status
  (`ok`, `missing`, `outdated`, `not_logged_in`, `failed`), the version, a sentence and a fix hint.
  Settings and onboarding show them.
- **Folder check.** `POST /v1/repos/check` (`{repo_path, work_branch?}`, base default `develop`)
  gathers facts (stat the path; `git rev-parse --is-inside-work-tree`; the local branches) and
  core judges them (`repo::judge`): `path_empty`, `path_not_absolute`, `path_missing`,
  `not_a_directory`, `not_a_git_repo`, `git_missing` on field `repo_path`, `branch_missing` on
  `work_branch` (with the repository's branches so a form can offer them). It always answers 200;
  the problems are data.
- **Typed refusals.** `POST /v1/projects` with a `repo_path`, and `PATCH /v1/projects/{p}` with a
  new `work_branch`, run the same check and answer 400 with `{error, code, field}`: `code` is the
  problem code and `field` the input it belongs to (`ErrorBody.field`, absent for errors that
  belong to no field). The same shape carries `profile_unknown` (a default profile that is not in
  the catalog) on `default_profile` and `limit_out_of_range` on `max_parallel`. A project created
  without a repository is not checked (a simulated project needs none).
- **Project settings.** `PATCH /v1/projects/{p}` reads and saves default profile, parallel limit,
  review mode, reviewer profile and base branch; all five live in the project row, so they survive
  a daemon restart. The merge mode arrives with the GitHub ticket and gets its row in
  `ProjectSettings`, where the layout keeps the place.
- **Window.** `/settings` and `/projects/{p}/settings` (`Settings.tsx`): checks, project settings,
  a chain-file import (the browser reads the file; `POST /v1/projects/{p}/import` is the existing
  route), and a form to add another project. With no project the home page is onboarding
  (`Onboarding.tsx`): the checks, then the add-project form, which opens the new project's
  settings. The form first calls the folder check, so every problem shows next to its field. Store:
  `store/settings.ts` (state, reducer) and `store/settingsEffects.ts` (network).
- **Folder picker.** A text field with a paste-friendly path, not a native picker. The desktop
  window loads the daemon's page as a remote URL, so a dialog plugin would need the Tauri IPC
  opened to that page; and a browser file input never yields a path. Revisit if the shell ever
  serves the page itself.
- **Spec folders** still import with `tok chain import <folder>`: reading a folder is a CLI
  adapter (`cmd/spec.rs`) and the window offers the chain file only.

## Agent access

How a running agent talks back, over the same route and use cases as `tok sim` (events in
`docs/events.md`).

- **Tokens.** `runner/tokens.rs` (`AgentTokens`, owned by `Runners`): `Runners::start` issues one
  token per run, scoped to the run's ticket; the watcher thread revokes it when the process ends.
  The token reaches the agent only through its environment (`TOKNILA_URL`, `TOKNILA_RUN_TOKEN`,
  `TOKNILA_TICKET`, `TOKNILA_RUN`, `TOKNILA_ROLE`, in `toknila_api_types::runner::agent_env`); the
  person's token is not known to `Runners` at all. A token carries its run's role: a worker's may
  send every event but `review_verdict`, a reviewer's only `review_verdict`
  (`Principal::may_send`, `403` otherwise).
- **Who is calling.** `server/auth.rs` turns the bearer token into a `Principal` (`Person` or
  `Agent(grant)`) for the handlers. An agent may call only `AGENT_ROUTES` (events, activity and
  the instruction claim of its own ticket); everything else is `403`. The ownership rule is `toknila_app::agent::ensure_granted`
  (`ErrorKind::Forbidden`, `403`). Business rules stay in core: `person_only` still guards the
  use cases, the allowlist keeps agents from reaching them.
- **Wiring.** `runner/wiring.rs`: for a profile with `mcp.enabled` / `hooks.enabled` and the
  flag set, the runner writes `runs/<ticket id>/mcp.json` and `settings.json` under the data home
  (never the worktree, never the user's own Claude settings) and appends `--mcp-config <file>` and
  `--settings <file>` to the command. Neither file holds a token; both name environment variables.
  The files run the `toknila` binary of the daemon (`current_exe`) as `toknila mcp` and
  `toknila hook <name>`.
- **Instructions file.** For a profile with `instructions_flag` (the built-in `claude-code` has
  `--append-system-prompt-file`) the runner also writes `runs/<ticket id>/instructions.md` and
  passes it by that flag. It holds the ticket's title, task and acceptance, the comment template,
  the short-output rules and how to report through the MCP tools
  (`toknila_core::instructions::generate`, pinned by `snapshots/run.md`). It is added to the
  system prompt; the worktree's own `CLAUDE.md` and `AGENTS.md` are never written and still apply.
  The ticket-independent part (`instructions::protocol`) is also the `instructions` field of
  `tok mcp`'s `initialize` answer, a second route to the same text. Verified against `claude`
  2.1.319: `--append-system-prompt-file <missing>` fails on the file path, not as an unknown
  option (`--help` lists only `--append-system-prompt`). `instructions_file` is a different
  field, the file the agent reads on its own (`CLAUDE.md`).
- **Agent side.** `tok mcp` (stdio MCP server) and `tok hook <name>` in `crates/cli`
  (`cmd/mcp.rs`, `cmd/mcp_tools.rs`, `cmd/hook.rs`) are thin clients of the daemon that read the
  environment above and do not start a daemon. The hook-to-event mapping is pure and lives in
  `toknila_core::hook`.
- **Instructions to the agent.** A person's instruction comment on a ticket whose agent is a
  process is queued as `ExternalEffect::DeliverInstruction` (`toknila_app::instructions`, for any
  instruction comment from a person, whoever left it: a typed one or a Return). The host picks the
  way in with `toknila_core::instructions::route(hooks, idle)`: type it into the terminal as one
  line, then Enter as a separate keystroke (`InstructionTyped` outcome), when the profile wires no
  hooks or the agent has printed nothing for 3 seconds; otherwise leave it pending for the
  agent's next Stop hook. `tok hook stop` calls `POST /v1/tickets/{ticket}/instructions/claim`
  (an agent route) and, when instructions were pending, prints
  `{"decision":"block","reason":"<the instructions in the comment template>"}` so Claude Code
  continues with them; with none pending it prints nothing and the agent stops. Each comment is
  delivered once: claiming or typing marks `delivered_at`, journals `comment.delivered` and moves
  a Waiting ticket back to Running. That is also the loop guard (`stop_hook_active`, which Claude
  Code sets while it continues because of a stop hook, and its cap of 8 consecutive
  continuations): the stop that follows finds nothing pending. A comment for a ticket without a
  live runner stays pending. Deliveries run after the command's other effects, so a ship or
  runner queued with them is not held up.
- **Activity.** Hook signals and any call made with a run token are kept in memory per ticket:
  the last MCP call, the last hook and the run's start (`AgentTokens::reported`).
- **Silence and nudges.** A daemon ticker (`runner/silence.rs`, every 5 s) looks at each live run.
  The rule is pure core (`toknila_core::nudge::decide`: last signals, child alive, nudge history,
  now). Silence is measured from the freshest MCP call or hook; only if neither was ever seen,
  from the last output. A delivered instruction and a nudge restart the timer. A live child
  process of the agent (a build, tests; `sysinfo`, exact pin, every OS; Toknila's own `mcp` and
  `hook` helpers excluded) suppresses both nudge and give-up. After `idle_secs` (default 120) the
  agent is nudged, at most `max_nudges` (3) within `window_secs` (900); then the card goes to
  Waiting (silent) through the machine (`AskHuman`). Thresholds come from the profile's
  `[nudge]`. A nudge is an instruction comment written by the system, so it travels the existing
  delivery (typed into an idle agent or claimed by its Stop hook, journaled `comment.delivered`);
  the comments are also the nudge history, so a daemon restart forgets nothing
  (`toknila_app::nudge`). The ticker reads facts outside the store's writer and acts in one command.
- **Back to Running.** `TicketEvent::AgentActive` moves Waiting to Running. The agent events
  `started`, `subtask_done`, `note` and `finished` (not a question, a crash, a proposal or a
  thread, which talk to the owner), and the hooks `post_tool_use` and `user_prompt_submit`
  (`toknila_core::nudge::resumes`), cause it; `stop` and `notification` do not, since they fire when
  the agent stops to wait. This also ends Waiting (silent).
- **Stopping.** `StopRunner` ends the whole process tree. On Unix the agent leads its own process
  group (the pty makes it a session leader): the group gets TERM and KILL after a 3 s grace
  (`nix`, exact pin). On Windows `taskkill /T /F` walks the tree (a Job Object would also catch
  orphans but needs FFI outside the one audited file).

Tests use `fake-agent` (`crates/daemon/src/bin/fake-agent.rs`: prints lines, echoes a line of
input, prints its terminal size, sleeps, exits with a code) as the profile command, so the same tests run on a Unix pty and
on ConPTY without a shell.

## Data safety

The database is the only copy of the plan, so an upgrade must not lose it.

- **Backup before migration.** When the daemon opens a database whose schema is older than the
  build's, and before it applies any migration, it writes a consistent copy (`VACUUM INTO`, never
  a copy of the live file) to `<home>/backups/toknila-v<old version>-<UTC timestamp>.sqlite`. The
  newest 5 stay. A fresh database needs none. If the copy cannot be made, no migration runs and
  the daemon refuses to start with an error naming the backup path; the database is untouched.
- **Check at start.** The daemon runs `PRAGMA quick_check` before anything else. A database that
  fails is refused, and the error names the newest backup.
- **Restore.** Stop the daemon (`tok daemon stop`), move `<home>/toknila.db` and its
  `toknila.db-wal` and `toknila.db-shm` files out of the way, copy the chosen backup to
  `<home>/toknila.db`, and start again. The daemon migrates a restored older backup forward (and
  backs it up first, as always). Anything done after the backup was made is not in it; the
  moved-away file is kept for that reason.

## Parallel agents

A project has a limit of agents working at once, `Project::max_parallel` (default 2, 1 to 64;
`tok project set <key> --parallel N`, `PATCH /v1/projects/{project}`). The rules are core's
(`schedule`); the application only applies them.

- **Slots.** A card holds a slot while it is Running or Waiting (its agent is alive). Review,
  Blocked, Merged, Released, Cancelled and Backlog hold none. Simulated agents count the same, so
  tests can use them. Return and Unblock put a card straight back to Running, and an agent's own
  `started` is a fact, not a request: these may exceed the limit for a while, and the card then
  holds a slot that delays the queue.
- **Queued is a flag on Ready, not a status.** `Ticket::queued` is set by the state machine's
  `Request` event (what `tok start` and the Start action send) and cleared by anything that
  leaves Ready. A new status would touch the transition table, the board columns, dependency
  rules, the database and every client for a state that is only "Ready, and wanted". The flag
  stores the one thing that cannot be derived (a person asked); the place in line is computed
  and never stored.
- **Who starts.** `schedule::plan(chains, limit)` is pure: it returns the slots used, the wanted
  cards that start now and those that wait. A card is wanted when it is Ready, not paused, has
  its dependencies done, and is `queued` or in a chain with autostart on. So a chain with
  autostart off only runs cards a person started; the queue never starts an unrequested card. The
  order is chain order (creation order across the project), then the card's place in its chain's
  dependency order, so a card never goes before the cards it waits for. `Start` is now sent only
  by the schedule; the graph's settle no longer starts autostart cards itself.
- **When.** `with_chain` ends every command on a chain by running the schedule for its project
  (`toknila-app::schedule::run`): the cards that have a slot get `Start` through the machine in
  the same transaction, which emits the usual `PrepareShip` and `EnsureRunner` effects. Nothing
  starts a process inside a transaction: the daemon runs those effects after the commit, as for
  every runner. A change of the limit runs the schedule too, so a higher limit starts waiting
  cards at once, and a lower one starts nothing and never stops a running agent; it only holds
  back the next start. Outcomes recorded after a commit can free a slot (a failed ship blocks its
  card), so the store keeps running "effects, then outcomes" until no more work is queued.
- **Read models.** `GET /v1/agents` lists each project's `limit`, `used` and `queued` cards
  with their place; `TicketDetail` and `TicketItem` carry `queue_position`. The web page
  shows a "queued #n" chip on the card and "used/limit agents" in the top bar, read-only; `tok ls`
  and `tok show` print `(queued #n)`.

## Inbox

The inbox is everything that waits on a person, computed and never stored. The rules are core's
(`inbox`); `toknila-app::inbox::load` only gathers a project's cards, threads and the subtasks
involved, and `GET /v1/projects/{project}/inbox` returns the list, newest first (`InboxItem`:
`kind`, `ticket`, `chain`, `title`, `since`, and `text`, `thread`, `subtasks`, `done_cards` where
the kind uses them).

| Kind | Listed while | The person's action |
|---|---|---|
| `proposal` | a wildcard card is Proposed | Approve, Reject with a reason (the ordinary card actions) |
| `question` | a card is Waiting on a question or a permission prompt | a reply: an instruction comment, which the runner delivers to the live agent |
| `blocked` | a card is Blocked; the item carries the reason | Unblock |
| `review` | a card is in Review and it is the person's turn (not while the reviewer agent has it, and not while its accepted work is `merging`; a merge that failed is a `blocked` item with its reason) | open the card |
| `thread` | a thread is open and its newest version came after the person's last request for changes (`thread::awaiting_person`) | open the thread |
| `leftovers` | a card was done out of order (`Ticket::done_out_of_order`) and an earlier card in its `behind` list still has open subtasks that nobody dismissed | the three decisions below |

A silent agent (Waiting, `silent`) is not an inbox item yet.

**Leftovers.** One item per earlier card, naming the cards done before it. The decision is
`POST /v1/tickets/{ticket}/leftovers/{move|close|leave}` on that earlier card; a person only, and
`409` when nothing is left to decide. The rules are `inbox::leftovers` in core:

- `move`: a new card `Leftovers from <title>` waits for every ticket nothing else waits for (the end
  of the chain); the open subtasks move to it as the same rows with their text untouched and the
  old card's checklist is numbered again; a decision comment on the old card names them. The spec
  folder export matches a card by its tasks' `T` ids, so the new card is matched to the old phase
  and the exported `tasks.md` stays byte-identical.
- `close`: the open subtasks are marked done by the person and a decision comment on the card
  lists them. The export then differs from the original only in those checkboxes.
- `leave`: the earlier card's key is added to `OutOfOrder::dismissed` on every card that was done
  behind it. The mark lives in the ticket's existing JSON column, so there is no migration, and
  the dismissal survives restarts.

The stream refreshes the inbox: every journal event that refreshes a chain also reloads it.

## Presentation: MVI



The UI follows Model-View-Intent, the same unidirectional shape the core already has
(`apply(state, event) -> (state, effects)`):

- `State`: an immutable value per screen (data, loading, error, selection).
- `Intent`: what the person did (`SelectTicket`, `AcceptTicket`, `FilterNeedsHuman`).
- `Msg`: everything that changes state: intents, API responses, stream events.
- `reduce(state, msg) -> [state, effects]`: a pure function, unit-tested without DOM or network.
- `Effect`: a description of a side effect (`CallApi`, `Navigate`, `ShowToast`) that an effect
  runner executes.
- `View`: a React component that reads `state` and dispatches intents; it never calls `fetch`.

State management:

- One normalized client store per app (`tickets[id]`, `chains[id]`, `lastSeq`), feature selectors
  over it.
- A stream event refreshes only what it touches: the chain it belongs to and the inbox (chain
  status and "needs a human" stay computed by the daemon). A full reload happens only after a gap
  in `seq` or an event the page cannot place.
- No optimistic updates: the command answers with fresh state and the event confirms it. Rules
  stay on the server: which actions a card allows is `TicketDetail.allowed_actions` and its review
  rounds are `TicketDetail.review.rounds`, both computed by the daemon; the page keeps no table of
  statuses and no history parsing, only the words and styles of the buttons.

Navigation is state too, and it lives in the URL, so a deep link opens the right screen:

| Route | Screen |
|---|---|
| `/` | Chains, with the "needs a human" filter |
| `/chains/:chain` | Chain graph |
| `/chains/:chain/tickets/:key` | Ticket details and runner terminal |
| `/inbox` | Everything waiting on a person, across projects, grouped by kind, each item with its action inline; the sidebar badge shows the count |
| `/settings` | Projects and agent profiles |

Phase 0 serves `/`, `/chains/:chain` and `/inbox` (the daemon answers these paths with the page);
the other routes arrive with their screens.

A reducer never navigates by itself; it returns a `Navigate` effect.

Types: the daemon's OpenAPI document is pinned in `apps/web/openapi.json` by a daemon test, and
`npm run gen` turns it into `apps/web/src/gen/api.d.ts`; `cargo xtask web` fails when the two
drift. The page has no hand-written copy of an API type. Layout of `apps/web/src`: `api/` (types,
the only `fetch`), `store/` (state, msg, reduce, effects, route; `reduce.test.ts` runs under
Vitest), `views/` (React components that read state and dispatch intents).

## Desktop

The desktop app is Tauri 2 around the same web bundle the daemon serves. It starts the daemon as
a sidecar, or attaches to a running one through `daemon.json`, exactly like the CLI. Closing the
window does not stop agents. On macOS (the first target) the shell uses the system WebKit view;
Windows and Linux use WebView2 and WebKitGTK with the same code.

The shell (`apps/desktop/src-tauri`) is its own Cargo workspace, because it needs the platform web
view, which `cargo xtask verify` does not assume. It runs `toknila daemon start` and
`toknila open --json` through the bundled sidecar, so starting, attaching and the token follow
the CLI's rules, and opens one window on that address. The window gets no Tauri IPC
capabilities: the page talks to the daemon over HTTP like in a browser.

## Rules the guards enforce

| Guard | Rule |
|---|---|
| `deps toknila-core` | Core has no async runtime, storage or transport in its whole dependency tree |
| `deps toknila-app` | The application layer has no async runtime, storage or transport in its dependency tree; it reaches them through ports |
| `deps toknila` | The CLI depends directly on no storage, HTTP-serving or HTTP-client crate; it calls the daemon through `toknila-client` |
| `file-size` | A production source file (Rust or TypeScript, inline tests excluded) has at most 400 lines. Files already over it are listed in `xtask/src/guards.rs` with their size and may only shrink; the guard asks to lower or drop an entry once a file shrinks |

Each step of the plan adds the guards for what it introduces (for example, `toknila-app` without
`axum` or `rusqlite`).

## Refactoring plan

Every step is one PR that keeps behaviour: the end-to-end tests and the JSON snapshots stay green
at each step.

| Step | Change |
|---|---|
| R-01 | This page, the ADR, the guards above |
| R-02 | Core without duplicates: one cycle check for the graph and chain files, `TicketStatus::is_done`, entity constructors and defaults, one way to name enums, `Display` for status reasons; `graph.rs` and `plan.rs` split into submodules |
| R-03 | Typed domain events and a single `settle` that returns them |
| R-04 | `toknila-app`: ports, unit of work, reference resolution, comments and subtasks, agent events, recovery, `AppError`, tests over an in-memory port implementation |
| R-05 | The daemon as thin adapters: the remaining use cases move to `toknila-app`, `http/routes/*`, `server.rs` split, typed ticket actions, one error body |
| R-06 | `toknila-api-types` and `toknila-client`; the CLI on typed responses, split by command group into `cmd/*` |
| R-07 | The web page on generated TypeScript types with an MVI store, a router and reducer tests |
| R-08 | Platform data directory on macOS with migration of `~/.toknila`; Tauri 2 shell in `apps/desktop` with `toknila` as its sidecar, built by `cargo xtask desktop` (CI: macOS) |
