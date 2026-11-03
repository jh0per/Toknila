# Architecture

Toknila is a Rust workspace with a pure domain core, a long-running daemon that owns state, and
thin clients (CLI, web page, desktop shell) that talk to the daemon over HTTP and WebSocket on
`127.0.0.1`. This page is the target shape of the code and the rules that keep it there. Where
the code does not match it yet, the refactoring plan below says which step fixes it.

Decision record: [ADR 0001](adr/0001-layered-architecture-and-mvi.md).

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
| Domain | `toknila-core` | Entities, the ticket state machine, the chain graph, chain files, the comment template, typed domain events, the agent event mapping, entity constructors and defaults | I/O, the clock, id generation, async runtimes, storage, transport |
| Application | `toknila-app` (planned) | Use cases (`create_ticket`, `apply_ticket_action`, `import_chain`, `handle_agent_event`, ...), the chain unit of work, reference resolution, read models, startup recovery, ports (`Clock`, `IdGen`, `ChainStore`, `Journal`, `Runner`), `AppError` | `axum`, `rusqlite`, HTTP status codes |
| Protocol | `toknila-api-types` (planned) | Request and response DTOs, `ErrorBody`, `EventRecord`, typed ticket and release actions. OpenAPI is generated from them, TypeScript types from OpenAPI | Logic |
| Adapters | `toknila-daemon` | `http/` (thin handlers), `store/` (SQLite implementation of the ports), `stream`, `runner/` (sim now, PTY in phase 1), lifecycle (`daemon.json`, signals), paths, logging, embedded web assets | Business rules |
| Client | `toknila-client` (planned) | Typed HTTP and WebSocket client of the API | Output formatting |
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
- Stream events are applied incrementally (a `status_changed` event carries the whole ticket).
  A full reload happens only after a gap in `seq` or an unknown event kind.
- No optimistic updates: the command answers with fresh state and the event confirms it. Rules
  stay on the server.

Navigation is state too, and it lives in the URL, so a deep link opens the right screen:

| Route | Screen |
|---|---|
| `/` | Chains, with the "needs a human" filter |
| `/chains/:chain` | Chain graph |
| `/chains/:chain/tickets/:key` | Ticket details and runner terminal |
| `/inbox` | Everything waiting on a person, across chains |
| `/settings` | Projects and agent profiles |

A reducer never navigates by itself; it returns a `Navigate` effect.

## Desktop

The desktop app is Tauri 2 around the same web bundle the daemon serves. It starts the daemon as
a sidecar, or attaches to a running one through `daemon.json`, exactly like the CLI. Closing the
window does not stop agents. On macOS (the first target) the shell uses the system WebKit view;
Windows and Linux use WebView2 and WebKitGTK with the same code.

## Rules the guards enforce

| Guard | Rule |
|---|---|
| `deps toknila-core` | Core has no async runtime, storage or transport in its whole dependency tree |
| `deps toknila` | The CLI depends directly on no storage or HTTP-serving crate |
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
| R-04 | `toknila-app`: ports, unit of work, reference resolution, use cases, recovery, `AppError`; agent events and instruction delivery as core rules behind a `Runner` port |
| R-05 | The daemon as thin adapters: `http/routes/*`, `server.rs` split, typed ticket actions, one error body |
| R-06 | `toknila-api-types` and `toknila-client`; the CLI on typed responses, split by command group |
| R-07 | The web page on generated TypeScript types with an MVI store, a router and reducer tests |
| R-08 | Platform data directories on macOS with migration of `~/.toknila`; Tauri 2 skeleton with the daemon as a sidecar |
