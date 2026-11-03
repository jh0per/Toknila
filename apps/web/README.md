# apps/web

The read-only page the daemon serves on `127.0.0.1` (Vite + TypeScript + React). It shows every
chain in dependency order: tickets with status colour, reason and time in status, milestones,
where the chain stopped, a "needs a human" filter, and live updates over the WebSocket stream
(`/v1/stream`). It reconnects by itself, also across a daemon restart, which keeps its address.

```sh
cargo xtask web       # npm ci + type-check + build into dist/; the daemon embeds dist/ at build time
cargo xtask e2e-web   # Playwright smoke test against the real tok binary
tok open              # prints http://127.0.0.1:<port>/?token=<token>
```

Without a build in `dist/` the daemon still compiles and serves a note saying how to build the
page. For development, `TOKNILA_PORT=<port from daemon.json> npm run dev` proxies `/v1` to a running
daemon; open the dev address with `?token=<token>`.

Phase 1 grows it into the three-panel UI (chain graph, runner terminal, ticket details).
