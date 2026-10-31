# Toknila

Toknila ("token killa") is a local orchestrator for a fleet of CLI coding agents (Claude Code,
Codex, OpenCode and others). You plan a project as **chains of tickets** with subtasks,
dependencies and release milestones; each ticket runs on its own **ship** (a git worktree with an
agent runner), and the ticket's status moves by itself from the agent's events: running, waiting
for a human, blocked, ready for review.

> Status: phase 0 (skeleton). Nothing runs agents yet. Phase 0 proves the model with a simulated
> agent profile driven from the CLI.

## Layout

| Path | What |
|---|---|
| `crates/core` | Domain model and pure rules: ticket state machine, chain graph. No I/O, no async runtime, no database. |
| `crates/daemon` | Daemon: SQLite store (single writer), HTTP API and WebSocket event stream on `127.0.0.1`. |
| `crates/cli` | `toknila` binary and its `tok` alias, a thin client of the daemon API. |
| `apps/web` | Web UI served by the daemon (read-only page in phase 0). |
| `apps/desktop` | Tauri 2 shell (later). |
| `xtask` | Repository tasks; `cargo xtask verify` is the one gate for local runs and CI. |

## Build and check

Requires a stable Rust toolchain (`rust-toolchain.toml` pins the channel and components).

```sh
cargo xtask verify          # fmt check, clippy -D warnings, tests, architecture guards
cargo run --bin tok -- --version
```

CI runs the same `cargo xtask verify` on Ubuntu, Windows and macOS.

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
