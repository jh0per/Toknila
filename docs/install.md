# Install and first run

Toknila 0.1.0 is phase 0: the model, the daemon, the CLI and a read-only web page, driven by a
simulated agent (`sim` profile). No real agent runs yet.

## macOS

Pick the archive for your Mac: `macos-arm64` for Apple silicon (M1 and later), `macos-x86_64`
for Intel. The binaries are not signed or notarised yet, so download with `curl` (it does not set
the quarantine flag) or clear the flag after a browser download.

```sh
V=0.1.0
ARCH=arm64            # or x86_64 on an Intel Mac
curl -LO https://github.com/jh0per/Toknila/releases/download/v$V/toknila-$V-macos-$ARCH.tar.gz
tar -xzf toknila-$V-macos-$ARCH.tar.gz
cd toknila-$V-macos-$ARCH
xattr -dr com.apple.quarantine . 2>/dev/null || true   # only needed after a browser download
sudo install -m 755 tok toknila /usr/local/bin/        # or keep them here and use ./tok
tok --version
```

If macOS still says the file "cannot be opened", open System Settings → Privacy & Security and
choose "Open Anyway" for `tok`, or run the `xattr` line above.

## Linux and Windows

Same archive layout: `toknila-<version>-linux-x86_64.tar.gz` and
`toknila-<version>-windows-x86_64.zip` (run `tok.exe` from PowerShell).

## First run: the demo chain

```sh
tok daemon start                      # background daemon on 127.0.0.1 (`tok daemon status` shows the data folder)
tok project new DEMO
tok chain import demo.toml --project DEMO
tok chain show demo                   # A runs; B and D wait for A; C waits for release R1
tok open                              # prints the web page address; open it in a browser
```

Keep the page open and move the chain from the terminal; the page updates live.

```sh
tok sim DEMO-1 subtask 1 done         # the simulated agent ticks a checklist item
tok sim DEMO-1 ask "which way?"       # the agent asks: DEMO-1 is Waiting, the chain needs a human
tok comment DEMO-1 "left"             # your answer; the agent reads it and runs again
tok sim DEMO-1 finish "A is done"     # hand-in: Review
tok accept DEMO-1                     # Merged; B and D start by themselves
tok sim DEMO-2 finish "B" && tok accept DEMO-2
tok sim DEMO-3 crash "out of memory"  # Blocked
tok unblock DEMO-3 && tok sim DEMO-3 finish "D" && tok accept DEMO-3
tok release start R1                  # manual milestone: cut; C starts
tok sim R1 finish                     # the release agent is done: A, B, D are Released
tok ls --needs-human                  # what waits on you
tok watch --since 0                   # the whole journal, then live (Ctrl-C to stop)
```

Toknila's own next phase is a chain too: `tok project new TOK && tok chain import phase-1.toml
--project TOK && tok chain show phase-1`.

## Where things live

| What | Where |
|---|---|
| Data, `daemon.json`, logs | `~/Library/Application Support/toknila` on macOS, `~/.toknila` on Linux, `%APPDATA%\toknila` on Windows; `TOKNILA_HOME` overrides. A macOS `~/.toknila` from an earlier version moves to the new place on the first start after the daemon was stopped |
| Database | `toknila.db` (SQLite) in that folder |
| Stop the daemon | `tok daemon stop` |
| Start over | stop the daemon, delete the folder |

Every command takes `--json` for scripts.
