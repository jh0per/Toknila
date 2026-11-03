# Agent event contract

How an agent reports on its ticket. In phase 0 the built-in `sim` profile starts no process and a
person sends these events with `tok sim <ticket> ...`. In phase 1 the Claude Code profile sends the
same events from MCP tools (`ask_human`, `subtask_done`, `note`, `finished`) and from agent hooks
(Notification, Stop) or the runner (process exit). The daemon does not care who sent them.

`POST /v1/tickets/{ticket}/events` with `Authorization: Bearer <token>` (from `daemon.json`).
`{ticket}` is a ticket key (`DEMO-3`) or id. The body is one JSON object tagged by `event`.
The response is the ticket with its subtasks and comments (`TicketDetail` in `/openapi.json`).

| `event` | Fields | Effect | Allowed from |
|---|---|---|---|
| `started` | | Ready -> Running; no-op if already Running | Ready, Running |
| `ask_human` | `text`, `permission` (bool, default false) | -> Waiting with the question or permission request as reason; the text is stored as an agent report comment | Running |
| `subtask_done` | `position` (1-based) | ticks the checklist item, `done_by: "agent"` | any |
| `note` | `body` (comment template fields: `gist`, `what`, `why`, `how_to_verify`, `alternatives`, `consequences`) | stores an agent report comment | any |
| `finished` | `summary` | -> Review; summary stored as an agent report comment | Running |
| `crashed` | `detail` | -> Blocked (reason `crashed`, resumes to the previous status on unblock) | Running, Waiting |

```json
{"event":"ask_human","text":"which framework?"}
{"event":"subtask_done","position":1}
{"event":"note","body":{"gist":"half way","what":"parser done"}}
{"event":"finished","summary":"did X"}
{"event":"crashed","detail":"exit 101"}
```

A refused transition answers `409 {"error": "...", "code": "conflict"}` and changes nothing.

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
`status_changed`, payload `{"key","from","to","reason","ticket"}`.
