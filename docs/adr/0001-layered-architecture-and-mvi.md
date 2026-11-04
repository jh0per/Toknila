# [decision] Layered architecture with an application layer, and MVI for the UI

**What:** Split the backend into domain (`toknila-core`), application (`toknila-app`: use cases
and ports), protocol (`toknila-api-types`) and adapters (`toknila-daemon`), with a typed client
(`toknila-client`) for the CLI. Build the UI as MVI screens in React/TypeScript, shipped in the
browser and in a Tauri 2 desktop shell (macOS first, then Windows and Linux). The target shape
is in [architecture.md](../architecture.md).

**Why:** After phase 0 the core is pure and well tested, but nothing sits between HTTP and the
core: domain rules, defaults and entity construction live in `api/handlers.rs` and
`api/commands.rs`, startup recovery depends on the HTTP error type, and the CLI and the web page
read untyped JSON with hand-copied types. Phase 1 adds MCP, hooks and a PTY runner as new
sources of the same events; without an application layer each would copy those rules again.

**How to verify:** `cargo xtask guards` checks the dependency rules and the file-size ratchet;
each refactoring step R-02 to R-08 adds the guards for what it introduces.

**Alternatives:**

- Keep use cases as a module inside the daemon: rejected, a module boundary cannot be guarded,
  and the daemon already shows the drift (recovery imports the HTTP layer).
- MVVM with two-way binding in the UI: rejected, state changes come from people, agents and
  autostart at once, and two-way binding hides who changed what. A store with `state`,
  `dispatch` and selectors gives the ViewModel ergonomics without that cost.
- Compose Multiplatform (Kotlin) for the desktop UI: workable, since the daemon speaks HTTP, but
  it ships a JVM, has no ready terminal widget for the phase 1 runner view, and would need a
  second code base for the page the daemon serves.
- SwiftUI: the most native on macOS, but Windows and Linux would need a separate UI.

**Consequences / risks:** Three new crates and a code generation step for TypeScript types.
The refactoring runs before phase 1 in eight behaviour-preserving PRs; until they land, the code
and `architecture.md` differ in the places the plan lists.
