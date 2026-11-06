# Agent event contract

How an agent reports on its ticket. The built-in `sim` profile starts no process and a person
sends these events with `tok sim <ticket> ...`. The `claude-code` profile sends the same events
from MCP tools (`tok mcp`, below) and, for a few clear cases, from agent hooks (`tok hook`), or
the runner does (process exit). The daemon does not care who sent them: one route, one use case.

`POST /v1/tickets/{ticket}/events` with `Authorization: Bearer <token>` (from `daemon.json`).
`{ticket}` is a ticket key (`DEMO-3`) or id. The body is one JSON object tagged by `event`.
The response is the ticket with its subtasks and comments (`TicketDetail` in `/openapi.json`);
for `propose` it is the new card.

| `event` | Fields | Effect | Allowed from |
|---|---|---|---|
| `started` | | Ready -> Running; no-op if already Running | Ready, Running |
| `ask_human` | `text`, `permission` (bool, default false) | -> Waiting with the question or permission request as reason; the text is stored as an agent report comment | Running |
| `subtask_done` | `position` (1-based) | ticks the checklist item, `done_by: "agent"` | any |
| `note` | `body` (comment template fields: `gist`, `what`, `why`, `how_to_verify`, `alternatives`, `consequences`) | stores an agent report comment | any |
| `finished` | `summary` | -> Review; summary stored as an agent report comment. Refused with `409` while the ticket has an open thread | Running |
| `crashed` | `detail` | -> Blocked (reason `crashed`, resumes to the previous status on unblock) | Running, Waiting |
| `review_verdict` | `verdict` (`approve`, `request_changes`), `comment` (required with `request_changes`) | the reviewer agent's decision, recorded as a review round. In `agent` mode `approve` merges the card and `request_changes` returns it to the worker with the comment; in `both` mode the card then waits for a person. **Only a reviewer's token may send it, and a reviewer's token may send nothing else** (`403`) | Review, while it waits for the agent (else `409`) |

The runner sends `crashed` itself when the agent's process ends while the ticket is Running or
Waiting and nobody asked it to stop: a non-zero exit code or a signal, and also a zero exit
without a `finished` event (the detail says which: `exit 101`, `killed by signal ...`,
`exited without a finished event`). A restart that lost the process blocks the ticket with
`runner_lost` instead. See the runner section of `docs/architecture.md`.
| `open_thread` | `kind` (`design`, `flow`, `architecture`, `other`), `title` | opens a thread on the ticket; nothing else changes | any |
| `thread_message` | `thread` (number on the ticket, or id), `text` | a message from the agent in the thread | any |
| `thread_version` | `thread`, `format` (`mermaid`, `markdown`, `html`), `body` | the next artifact version (at most 512 KiB, else `400`); reopens an approved thread | any |
| `propose` | `title`, `prompt`, `acceptance` (list), `after`, `before` (lists of ticket keys, titles or release versions of the chain) | a wildcard card in Proposed on the ticket's chain, found during this ticket; nothing else changes | any |

```json
{"event":"ask_human","text":"which framework?"}
{"event":"subtask_done","position":1}
{"event":"note","body":{"gist":"half way","what":"parser done"}}
{"event":"finished","summary":"did X"}
{"event":"crashed","detail":"exit 101"}
{"event":"review_verdict","verdict":"request_changes","comment":"handle the empty list"}
{"event":"propose","title":"token refresh endpoint","before":["DEMO-2"]}
{"event":"open_thread","kind":"design","title":"List screen"}
{"event":"thread_version","thread":"1","format":"mermaid","body":"flowchart TD\n  A --> B"}
{"event":"thread_message","thread":"1","text":"empty, loading, error and content states"}
```

An agent token (below) sends these with `Authorization: Bearer <run token>` and only for its own
ticket; any other ticket answers `403 {"error": "...", "code": "forbidden"}`.

A refused transition answers `409 {"error": "...", "code": "conflict"}` and changes nothing.

## MCP server (`tok mcp`)

`tok mcp` is a stdio MCP server (newline-delimited JSON-RPC 2.0: `initialize`, `ping`,
`tools/list`, `tools/call`). It reads the daemon address and the run's token from the environment
the runner gives the agent (`TOKNILA_URL`, `TOKNILA_RUN_TOKEN`, `TOKNILA_TICKET`; `TOKNILA_RUN`
names the run, `TOKNILA_ROLE` is `worker` or `reviewer`), never from the data home, and forwards each call to
`POST /v1/tickets/{ticket}/events`. There is one tool per event, named like it, taking the
event's fields: `started`, `ask_human`, `subtask_done`, `note`, `finished`, `crashed`, `propose`,
`open_thread`, `thread_message`, `thread_version`. A reviewer run (`TOKNILA_ROLE=reviewer`) is
offered one tool instead: `review_verdict`. A call the daemon refuses (a `409` such as an
open thread blocking `finished`, or a `403`) comes back as a tool result with `isError: true` and
the daemon's message; the agent reads it and adapts. No tool approves, rejects, accepts or
cancels anything: those are a person's.

## Hooks (`tok hook <name>`)

Claude Code hooks are a second source, conservative on purpose. The runner subscribes to five;
`tok hook <name>` reads the hook's JSON from stdin, and the mapping lives in core
(`toknila_core::hook`):

| Hook | `<name>` | Activity | Event |
|---|---|---|---|
| `Notification` with `notification_type: "permission_prompt"` | `notification` | yes | `ask_human` with `permission: true`, text = the notification's `message` |
| `Notification` of any other type (`idle_prompt`, ...) | `notification` | yes | none |
| `Stop` | `stop` | yes | none: it fires after every reply, so it is **not** `finished` |
| `SubagentStop` | `subagent_stop` | yes | none |
| `UserPromptSubmit` | `user_prompt_submit` | yes | none |
| `PostToolUse` | `post_tool_use` | yes | none |

Every hook is an activity signal: `POST /v1/tickets/{ticket}/activity` with `{"signal":
"hook:stop"}` (`204`, no journal entry, nothing about the ticket changes). The daemon keeps the
last signal of each ticket's agent in memory (`AgentTokens::last_activity`: its time and what it
was); any other call made with the run's token counts as activity `api`. The silence detector
consumes this (see "Silence and nudges" in `architecture.md`). `post_tool_use` and
`user_prompt_submit` also move a Waiting ticket back to Running; the other hooks change nothing. A hook never gets in the agent's way: failures are one line on stderr
and exit code 0, and outside a Toknila run (no `TOKNILA_*` environment) it does nothing. The
transition `ask_human` makes (Running -> Waiting) is refused with `409` in any other status; the
hook reports that on stderr and carries on.

## Waiting and back

`ask_human` (a question, a permission prompt) moves Running to Waiting. Any later agent event
except `ask_human` and `crashed`, or a `post_tool_use` / `user_prompt_submit` hook, moves it back to
Running (machine event `agent_active`, no journal entry of its own beyond the status change). The
same holds for Waiting (silent), which the silence detector sets after its nudges went unanswered.

## Agent tokens

Each run has its own token, scoped to its ticket and to its role (worker or reviewer). The runner creates it when the run starts,
puts it in the agent's environment only (`TOKNILA_RUN_TOKEN`) and revokes it when the process
ends (before the end is journaled); a revoked or made-up token is `401`. The person's token
(`daemon.json`) is never put in any runner environment or generated file: the generated MCP
config and hook settings name `${TOKNILA_RUN_TOKEN}` and the agent expands it from its own
environment.

An agent token may call exactly two routes, both `POST`, both for its own ticket:
`/v1/tickets/{ticket}/events` (every agent event, including the agent's thread posts and
`propose`) and `/v1/tickets/{ticket}/activity`. Everything else answers `403` with code
`forbidden`: approve and reject of wildcards, approve and request changes on threads, accept,
return, cancel, runner input, comments, edits, the stream, release events, shutdown. The rule is
an allowlist (`AGENT_ROUTES` in `crates/daemon/src/server/auth.rs`), so a route added later is
refused to agents until it is listed on purpose; a test walks every route of the API. Events for
a ticket the token does not own answer `403` and change nothing. Within the events route the role
decides: a worker's token may send everything but `review_verdict`, a reviewer's only
`review_verdict`; `accept` and `return` stay a person's.

Residual risk: an agent runs as the same OS user as the daemon and could read the person's token
from `daemon.json` (mode 600, same user) and use it. The scoping stops a cooperating or confused
agent and keeps person-only actions out of the agent's reach by construction; it is not a sandbox
against a hostile process.

## Wildcard cards

Work nobody planned becomes a wildcard card, never a silent change to the plan. An agent proposes
one with `propose`; a person with `POST /v1/chains/{chain}/wildcards` (body: the same fields plus
an optional `from` ticket; `tok propose`). The card is a ticket in `proposed` with the reason
`{"status":"proposed"}` ("awaiting approval") and a mark `wildcard`:
`{"from", "proposed_by", "after", "before", "approved"}`. Until a person decides it has no edges:
it never starts, changes no other ticket's readiness, does not count in chain progress, and is
listed under "needs a human".

Only a person decides, through the ticket actions: `POST /v1/tickets/{ticket}/actions/approve`,
or `.../actions/reject` with `{"text": "<reason>"}` (`tok approve`, `tok reject`). No agent event
approves or rejects. Approval moves the card to Backlog and turns `after` and `before` into
edges; then the chain settles as usual (Ready, autostart). A cycle, an edge to a card that is
still proposed, or a `before` node that has already started refuses the approval with `409` and
a message naming the problem; the card stays proposed. Rejection cancels the card with the
reason. The mark stays on the ticket either way (`approved: true` after approval).

## Threads on a card

Before an agent implements a card, the owner and the agent settle its design in threads (a screen
with its empty, loading, error and content states, a flowchart, a process, an architecture
diagram). A thread belongs to one ticket, has a number on it (`#1`, `#2`), a kind, a title and a
state, `open` or `approved`. It holds messages (`author`: person or agent; `kind`: `message` or
`changes_requested`) and artifact versions (`number` from 1, `format`, `body`; immutable).

A person works through `GET|POST /v1/tickets/{ticket}/threads` (list with messages and versions
and the ticket's `phase`; open with `{"kind","title"}`) and, with `{thread}` a number or an id,
`POST .../threads/{thread}/messages` (`{"text"}`), `.../versions` (`{"format","body"}`),
`.../approve` and `.../changes` (`{"text"}`, request changes). The answer of each is the thread
with its messages and versions. CLI: `tok thread ls|open|post|approve|changes`. The agent opens
and posts with the events above (`tok sim <ticket> thread open|post`); no event approves, and
core refuses approval and "request changes" from anyone but a person.

Approval pins the latest version (`approved_version`; absent for a thread with no artifact). A new
version, from anyone, reopens an approved thread and clears the pin; "request changes" keeps an
open thread open or reopens an approved one. While any thread is open, `finished` is refused:
`409 {"error": "cannot hand in DEMO-1: 1 open: #1 \"List screen\" (Design)", "code": "conflict"}`
and the ticket stays Running. The ticket's `phase` in `TicketDetail` is `design` while any thread
is open and `implement` otherwise; it is computed, never stored.

## Review

A card in Review waits for a verdict from a person, from the reviewer agent, or from both in turn,
by review mode (`docs/architecture.md`, "Review"). A person decides with the ticket actions
`accept` (optional `text`: remarks) and `return` (`text` required), which also stop a reviewer that
is still looking. Both are recorded as rounds on the ticket (`review.rounds`) with the agent's
`review_verdict` rounds. `PATCH /v1/projects/{p}` and `PATCH /v1/chains/{c}` take
`{"review_mode": "human|agent|both", "reviewer": "<profile>"}` (either may be left out; a blank
`reviewer` clears it); `PATCH /v1/tickets/{t}` takes the same two fields. `TicketDetail` carries
`review` (`mode`, `reviewer`, `awaiting`, `rounds`) and `allowed_actions`.

## Instructions back to the agent

Everything a person tells an agent is a ticket comment first (`POST /v1/tickets/{ticket}/comments`,
kind `instruction`). The runner delivers pending instructions and marks them `delivered_at`
(journal `comment.delivered`); a Waiting ticket goes back to Running when one is delivered. A
nudge to a silent agent is such an instruction, authored by `system`, so the card shows it and
the journal records its delivery. The
`sim` profile "reads" instantly: delivery happens in the same command for Running and Waiting sim
tickets. A process agent gets it typed into its terminal, or, while it works and its Stop hook is
wired, at its next stop: `POST /v1/tickets/{ticket}/instructions/claim` (agent token) answers
`{"count": n, "reason": "<instructions>"}` and marks them delivered, once. See "Agent access" in
`architecture.md`.

## Release agent

`POST /v1/releases/{release}/events` with `{"event":"finished"}` marks a cut milestone released and
moves its tickets from Merged to Released. `{release}` is a version or id. Cutting is
`POST /v1/releases/{release}/actions/start` (manual milestones) or automatic (auto milestones).

## Stream

Every change is journaled; `GET /v1/stream?since=<seq>` (WebSocket, `?token=` accepted) sends the
journal after `since`, then live events. Each message is an `EventRecord`:
`{"seq", "at", "actor", "entity", "entity_id", "kind", "payload"}`, e.g. entity `ticket`, kind
`status_changed`, payload `{"key","from","to","reason","ticket"}`. Wildcard cards add the ticket
kinds `proposed` (`{"ticket"}`), `approved` (`{"key","ticket"}`) and `rejected`
(`{"key","reason","ticket"}`), each next to the `status_changed` it causes; the edges an
approval adds come as chain `edge_added`. Threads use the entity `thread` (id: the thread's) with
the kinds `opened` (`{"ticket","thread"}`), `message_posted` (`{"ticket","message"}`),
`version_posted` (`{"ticket","thread","number","format","author","bytes"}`; the body is not
journaled), `approved` and `reopened` (`{"ticket","thread"}`). A change of review settings is
`project`/`chain` kind `updated` (`{"project"}` / `{"chain"}`).
