# Agent event contract

How an agent reports on its ticket. In phase 0 the built-in `sim` profile starts no process and a
person sends these events with `tok sim <ticket> ...`. In phase 1 the Claude Code profile sends the
same events from MCP tools (`ask_human`, `subtask_done`, `note`, `finished`) and from agent hooks
(Notification, Stop) or the runner (process exit). The daemon does not care who sent them.

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
{"event":"propose","title":"token refresh endpoint","before":["DEMO-2"]}
{"event":"open_thread","kind":"design","title":"List screen"}
{"event":"thread_version","thread":"1","format":"mermaid","body":"flowchart TD\n  A --> B"}
{"event":"thread_message","thread":"1","text":"empty, loading, error and content states"}
```

A refused transition answers `409 {"error": "...", "code": "conflict"}` and changes nothing.

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

## Instructions back to the agent

Everything a person tells an agent is a ticket comment first (`POST /v1/tickets/{ticket}/comments`,
kind `instruction`). The runner delivers pending instructions and marks them `delivered_at`; a
Waiting ticket goes back to Running when one is delivered. The `sim` profile "reads" instantly:
delivery happens in the same command for Running and Waiting sim tickets.

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
journaled), `approved` and `reopened` (`{"ticket","thread"}`).
