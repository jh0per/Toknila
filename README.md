# Toknila

Toknila ("token killa") is a local orchestrator for a fleet of CLI coding agents (Claude Code,
Codex, OpenCode and others). You plan a project as **chains of tickets** with subtasks,
dependencies and release milestones; each ticket runs on its own **ship** (a git worktree with an
agent runner), and the ticket's status moves by itself from the agent's events: running, waiting
for a human, blocked, ready for review.

> Status: v0.2.0, the first version to live in day to day: the daemon, the CLI, the web page and a
> Mac app, with real agents next to the simulated one.
>
> Install and first run: [docs/install.md](docs/install.md) (`cargo xtask app` builds the Mac
> app). Binaries for macOS (Apple silicon and
> Intel), Linux and Windows are on the [releases page](https://github.com/jh0per/Toknila/releases).

## Layout

| Path | What |
|---|---|
| `crates/core` | Domain model and pure rules: ticket state machine, chain graph. No I/O, no async runtime, no database. |
| `crates/app` | Use cases over ports: the chain unit of work, reference resolution, agent events, recovery. No I/O; the daemon implements the ports. |
| `crates/api-types` | Request and response bodies of the daemon API; the OpenAPI document is generated from them. |
| `crates/client` | Typed client of the daemon API, used by the CLI. |
| `crates/daemon` | Daemon: SQLite store (single writer), HTTP API and WebSocket event stream on `127.0.0.1`. |
| `crates/cli` | `toknila` binary and its `tok` alias, a thin client of the daemon API. |
| `apps/web` | Web UI served by the daemon (read-only page in phase 0). |
| `apps/desktop` | Tauri 2 shell: the window of the Mac app. |
| `xtask` | Repository tasks; `cargo xtask verify` is the one gate for local runs and CI. |
| `docs` | [Architecture](docs/architecture.md), decision records, install guide, agent event contract. |

## Build and check

Requires a stable Rust toolchain (`rust-toolchain.toml` pins the channel and components).

```sh
cargo xtask web             # build the web page the daemon embeds (needs Node 22)
cargo xtask verify          # fmt check, clippy -D warnings, tests, architecture guards
cargo xtask e2e-web         # browser smoke test of the page (Playwright)
cargo run --bin tok -- --version
cargo xtask dist aarch64-apple-darwin   # release archive in dist/
cargo xtask app             # macOS: Toknila.app and a .dmg for this Mac, ad-hoc signed (dist/app)
```

CI runs the same tasks on Ubuntu, Windows and macOS. Pushing a `v*` tag builds the release.

## Roadmap

0. **Skeleton**: monorepo, daemon with API, SQLite schema, chain/ticket/subtask/comment/release
   model, status transitions and Ready autostart, CLI. Gate: a demo chain reaches Released using
   only CLI commands and survives `kill -9` of the daemon.
1. **MVP**: Claude Code profile, PTY runner on a worktree from `develop`, nudge loop, MCP server and
   hooks, generated `CLAUDE.md`, web UI with three panels and xterm.js, review modes, auto-merge.
2. **v1**: depot and `CONTEXT.md`, live chain editing, release milestones, ADRs from decision
   comments, token counter, Codex and OpenCode profiles, Tauri builds.

## License

[Elastic License 2.0](LICENSE) (source-available).
