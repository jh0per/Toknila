# [decision] Each project has a main agent, the planner: it plans, the owner starts and accepts

**What:** A project can run one planner: a Claude Code session in the project's repository that
reads the code and the whole plan and edits the plan, and does nothing else. Planner, card agents
and owner divide the work: the planner plans; card agents do the cards; the owner starts cards and
accepts work.

The planner may read the repository and the plan (chains, cards, dependencies, statuses, threads,
review results, pull request state); create chains and cards; edit a card that has not started
(title, prompt, acceptance, checklist); add, remove and reorder dependencies; and open threads on
cards and write in them. It may not start, pause, resume, cancel, accept, return, reopen or merge
anything, approve a thread, or edit a file.

It is a normal runner role (`AgentRole::Planner`) with a built-in profile `claude-code-planner`
(`--permission-mode dontAsk`; allowed: Read, Grep, Glob, read-only git, the `toknila` MCP server;
no hooks). The daemon starts it through the existing PTY runner and profile machinery in the
project's `repo_path`, with a run token whose grant has the role `planner`. Its tools are MCP tools
of `tok mcp` in that role: `show_plan`, `new_chain`, `add_card`, `edit_card`, `add_dependency`,
`remove_dependency`, `move_card`, `open_thread`, `thread_message`, `thread_version`. Each tool calls
a `/v1/planner/...` route, which runs a use case of `toknila_app::planner`, which calls the same
use cases as the window and the CLI (`chains`, `tickets`, `threads`). The owner starts and stops it
(window or `tok planner start|stop|logs`) and types to it in its terminal.

**Why:** Planning is the part of the work that most benefits from an agent that has read the code
and can see the whole plan, and the part where a wrong move is cheapest to undo. Starting a card
spends money and changes the repository; accepting changes the base branch. Keeping those with the
owner is the point, so the rule is a property of the token, not of the agent's good behaviour.

**How it is kept to its own project and away from the owner's calls:**

- The planner token reaches only the routes in `PLANNER_ROUTES` (`server/auth.rs`); every other
  route, including every route added later, is `403`. No path carries a project: the token's grant
  names it, and `toknila_app::planner` checks every chain, card and thread it resolves against
  it (`403` for another project's, `404` for one that does not exist).
- Starting a card is also an effect of editing: a chain with autostart starts a Ready card by
  itself. So the planner's chains are created with autostart off (`planner::NEW_CHAIN_AUTOSTART`),
  and it may not add a card, add or remove a dependency or move a card in a chain that autostarts
  (`planner::ensure_graph_change`, core). Text edits and threads stay open everywhere.
- Only a card that has not started is rewritten (`planner::ensure_card_editable`): its agent has
  its prompt already.
- Start, stop, input and size of the planner are a person's actions (`person_only`), and its routes
  are person-only by the token check as well.
- The planner's writes run as `Actor::Agent`, so its thread messages say so and core already
  refuses an agent's approval of a thread.

**How to verify:** core (`planner.rs`, `instructions/planner.rs`, `profile/tests.rs`), use cases
over the in-memory ports (`app/src/tests/planner.rs`), the router (`http/tests/planner.rs`: a
planner token on every route of the API, the other project's cards, a card that is never started),
the wiring (`runner/wiring/tests/planner.rs`), `crates/cli/tests/planner.rs` (a real daemon and
the fake agent as the planner profile: it plans with its token, is refused start/accept/events/stop,
and `tok mcp` in the planner role does the same through tools), and a Playwright run for the window.

**Alternatives:**

- A card-like entity "planner card" per project that holds the session: rejected, it would put a
  fake card in every list, chain and count, and the machine would need exceptions to ignore it.
- Key the runner by a new `RunTarget` enum (card or project): rejected for now, it touches every
  signature of the runner port, the logs and the streams; the planner is keyed by its project's id
  read as a card id (`planner::run_key`; ULIDs do not collide) and labelled `KEY-0`, which no card
  can have.
- Give the planner the existing card routes with a scope check in each handler: rejected, a new
  handler that forgets the check would be open; a separate list of routes with the project taken
  from the token cannot be widened by accident, and a test calls every route of the API with a
  planner token.
- Let the planner edit chains it did not create when they autostart: rejected until the owner says
  otherwise (see risks).
- Hooks and nudges for the planner: rejected, it is interactive with the owner and hands nothing
  in; a silent planner is not a stuck card.

**Consequences / risks:**

- No migration and no journal entries for the planner's start and end; its plan edits are ordinary
  journal events, so the diagram updates live. The window learns that the planner ended from the
  terminal stream.
- A chain made in the window autostarts by default, so the planner can only restructure chains
  with autostart off. The owner changes it after creation (`tok chain set <chain> --autostart
  on|off`, `PATCH /chains/{chain}`, the toggle in the chain's header and context menu); turning it
  on starts the ready cards, which is the owner's act, and the journal records it as
  `chain.updated`.
- A chain's name cannot be changed and its review settings stay with the owner, so "edit a chain"
  means creating one.
- The planner has no profile setting per project: `start` takes a profile name and defaults to
  `claude-code-planner`.
- `git diff *` and `git log *` accept options that write files (`--output`); the reviewer profile
  has the same rules. The deny-by-default mode keeps everything else out.
- P2-01 changes the run wiring for every role (auto memory off, the subagent tool denied, ships on
  a path without whitespace, a stale-base check). The planner is a normal role and gets what
  applies to it.

**Where the planner reads the code (P2-04):** the planner's folder is not always the working copy.
On every start the daemon makes a best-effort `git fetch` (20 s at most). In pull-request mode,
where merges happen on GitHub and the working copy goes stale, the planner runs in a detached
checkout of the fetched `origin/<base>` at `<ships root>/<project>/_planner`, moved to the fresh
base on every start (a planner that is still live is left alone) and removed with the project. In
local mode it runs in the working copy, and when the base is behind its upstream its instructions
say so in one line. If the checkout cannot be made, it falls back to the working copy.
