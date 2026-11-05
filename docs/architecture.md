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
| Domain | `toknila-core` | Entities, the ticket state machine, the chain graph (with wildcard cards: proposed work outside the plan until approved), chain files, the comment template, typed domain events, the agent event mapping, threads on a card (`thread`: open, post, approve, the hand-in gate, the computed phase), entity constructors and defaults | I/O, the clock, id generation, async runtimes, storage, transport |
| Application | `toknila-app` | Every use case (`projects`, `chains` with import and export, `tickets` with typed `TicketAction`, `wildcards` (proposals), `threads`, `releases` with `ReleaseAction`, comments and subtasks, agent events), the chain unit of work (`with_chain`: change, settle, save, journal, deliver), reference resolution, startup recovery, `AppError`, and the command inputs (`NewTicket`, `NewChain`, ...) the API accepts as request bodies. Ports: `Reader` and `UnitOfWork` (which carries the command's clock and actor); a `Runner` port arrives with real runners in phase 1 | `axum`, `rusqlite`, `tokio`, HTTP status codes |
| Protocol | `toknila-api-types` | Read models (`ChainDetail::from_view`, `TicketDetail`, list items), `ErrorBody`, `EventRecord`, query and small request types; re-exports the command inputs of `toknila-app` and the agent events of core. OpenAPI is generated from them (feature `openapi`), TypeScript types from OpenAPI | Logic, transport |
| Adapters | `toknila-daemon` | `http/` (`routes/*`: one module per resource, each handler parses, runs one use case, answers; one `ErrorBody` for every failure including 401), `server/` (router, auth, lifecycle, logging), `store/` (SQLite implementation of the ports), `stream`, `runner/` (sim now, PTY in phase 1), lifecycle (`daemon.json`, signals), paths, logging, embedded web assets | Business rules |
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
  stay on the server.

Navigation is state too, and it lives in the URL, so a deep link opens the right screen:

| Route | Screen |
|---|---|
| `/` | Chains, with the "needs a human" filter |
| `/chains/:chain` | Chain graph |
| `/chains/:chain/tickets/:key` | Ticket details and runner terminal |
| `/inbox` | Everything waiting on a person, across chains |
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
