# apps/web

The read-only page the daemon serves on `127.0.0.1` (Vite + TypeScript + React), laid out like an
IDE. Left sidebar: projects, the chains of the selected project, the inbox (tickets that need a
human, with a count), running agents, a settings placeholder. Main area: the chain canvas (cards
linked left to right in dependency order, release milestones as their own nodes; `src/layout.ts`
is the pure position function) above a panel for the selected card with tabs Details, Events
(that ticket's journal in words, live, the raw event folded), Chat (its comments, questions and
answers as bubbles, with the answer box inline), Terminal, Threads and PR. Statuses are grouped
for display in `src/store/groups.ts` (to do, in progress, blocked, done; cancelled apart). It
updates live over the WebSocket stream (`/v1/stream`) and reconnects by itself, also across a
daemon restart, which keeps its address. Styling is neutral on purpose (CSS variables, light and
dark); a later ticket restyles it.

It follows MVI (`docs/architecture.md`): `src/store/reduce.ts` is a pure
`reduce(state, msg) -> [state, effects]` with Vitest tests next to it, `src/store/effects.ts` runs
the effects (the only network code), and `src/views/` only renders state and dispatches intents.
Routes live in the URL: `/projects/<key>`, `/projects/<key>/chains/<name>`,
`/projects/<key>/chains/<name>/cards/<ticket key>`, `/inbox`, `/usage` (tokens and estimated cost: totals, a per-day chart, breakdowns), and the older `/` and
`/chains/<name>`, which render in the same layout. The bottom panel's tab is state, not URL.

API types are generated: `openapi.json` is the daemon's API document (pinned by a daemon test;
refresh it with `TOKNILA_UPDATE_SNAPSHOTS=1 cargo test -p toknila-daemon openapi`), and
`npm run gen` writes `src/gen/api.d.ts` from it.

```sh
cargo xtask web       # npm ci, generated-types check, Vitest, type-check, build into dist/
cargo xtask e2e-web   # Playwright smoke test against the real tok binary
tok open              # prints http://127.0.0.1:<port>/?token=<token>
```

Without a build in `dist/` the daemon still compiles and serves a note saying how to build the
page. For development, `TOKNILA_PORT=<port from daemon.json> npm run dev` proxies `/v1` to a running
daemon; open the dev address with `?token=<token>`.

Not built yet: editing from the UI, wildcard cards, threads, the terminal, agent limits.
