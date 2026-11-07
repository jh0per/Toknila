# Install and first run

Toknila 0.2.0 runs on your Mac as an app (`Toknila.app`) that you build yourself, or as plain
binaries (`tok` and `toknila`) from the release archives. This page starts with the app.

## The Mac app (built locally)

The app is built on the Mac that runs it, for that Mac's architecture. It is signed ad hoc (no
Developer ID, not notarised), which is enough to run it where it was built. Needs the Rust
toolchain, Node 22 and the Xcode command line tools.

```sh
cargo xtask app
```

It builds the web page in release mode, the daemon (`toknila`) and the `tok` command in release
mode and the window shell, then assembles `dist/app/Toknila.app` and
`dist/app/Toknila-<version>-<arch>.dmg` (`aarch64` or `x86_64`) and prints both paths. It also
packs the update archive (see Updates and releases below); without the signing key it says it
skipped signing, and the build is otherwise the same. `cargo xtask app --target
x86_64-apple-darwin` builds for the other architecture. To install, open the `.dmg` and drag
Toknila onto Applications (or `cp -R` the `.app` there). Because the app was built here and never
downloaded, macOS does not ask. If you copy the `.dmg` to another Mac through a browser or AirDrop,
macOS quarantines it; right-click the app and choose Open once, or run
`xattr -dr com.apple.quarantine /Applications/Toknila.app`.

Everything the app needs is inside it: the daemon and `tok` in `Contents/MacOS`, the web page and
the price table inside the daemon binary. The hooks, the MCP server and the status line that
Toknila writes for an agent call the daemon by its absolute path inside the app (paths with spaces
are quoted). A new build takes over by itself: see Updating.

### First run

Opening the app starts the daemon and opens its window. If a daemon of the same version from this
app's own binary already runs, the app attaches to it; if one of another version or from another
copy of the app runs (after an update), the app stops it first and starts its own. On an empty
data home the window shows onboarding: it checks `git`, `claude` and `gh`, each with a hint when
something is missing, then asks for the repository of your first project and its base branch.
After that, import a chain file or add cards, and set the limits and review mode in Settings. The
app starts the daemon with the `PATH` of your login shell plus the usual folders such as
`/opt/homebrew/bin`, so it finds `claude` and `gh` although Finder starts apps with a bare `PATH`.
Closing the window leaves the daemon and its agents running; `tok daemon stop` stops them (use the
command below, or `/Applications/Toknila.app/Contents/MacOS/tok daemon stop`).

### The `tok` command

Settings has an "About" box with the action "Install the tok command". It links the app's `tok` to
`~/.local/bin/tok`. It writes only inside your home folder, never uses `sudo`, replaces only a link
left by an earlier Toknila, and says when `~/.local/bin` is not on your `PATH` (add
`export PATH="$HOME/.local/bin:$PATH"` to `~/.zshrc`). The same by hand:

```sh
mkdir -p ~/.local/bin
ln -sf /Applications/Toknila.app/Contents/MacOS/tok ~/.local/bin/tok
tok --version
```

The link points into the app, so it follows updates.

### Where things live

`~/Library/Application Support/toknila` holds the data: `toknila.db` (SQLite), `daemon.json`,
the logs and the `backups` folder. New ships (worktrees) go to `~/.toknila-ships`, a path without a
space. `TOKNILA_HOME=<folder>`, set in the environment before the app
or the daemon starts, uses another folder instead, for example
`open -n --env TOKNILA_HOME=<folder> /Applications/Toknila.app`; that is how to try the app without
touching your real data.

### Backups

Before the daemon moves the database to a newer layout (an update with a schema change), it copies
the database to `backups/toknila-v<old version>-<UTC time>.sqlite` in the data home and keeps the
newest five. That needs nothing from you; copy the data home yourself for backups of your own. To
go back, stop the daemon, move `toknila.db` (and its `-wal` and `-shm` files) away, copy a backup
to `toknila.db` and start the app. A database written by a newer Toknila is refused, not changed.

### Updating

An app installed from a release updates itself: see Updates and releases below. A build made
with `cargo xtask app` updates by hand: run it again and replace `/Applications/Toknila.app` with
the new one (drag from the new `.dmg` and choose Replace). Either way the new app, once opened,
stops the old daemon (it waits up to 10 seconds for the old one to exit) and starts its own, so
there is no step to do by hand. Agents the old daemon was running end with it. If the old daemon
does not stop in time, the window says so and names it; run `tok daemon stop` and open the app
again. The data home is kept; the new daemon migrates it on its first start, after the automatic
backup above.

### Removing it

```sh
tok daemon stop                                  # first, so nothing keeps running
rm -rf /Applications/Toknila.app
rm -f ~/.local/bin/tok                           # only if you installed the command
rm -rf ~/Library/Application\ Support/toknila    # only if you also want your data gone
```

Your project repositories are yours. The ships Toknila made in them are worktrees; remove one with
the worktree command of your version control tool, run in its repository.

### Check a build

`scripts/smoke-app.sh` mounts the `.dmg`, copies the app to a temporary folder, launches it against
a temporary data home, checks the daemon, the version, a project and a card made with the bundled
`tok`, and that no process is left after quitting. It does not touch your data home.

## Updates and releases

### How the app updates

The app looks for a newer version when it starts and every 6 hours after, at
`https://github.com/jh0per/Toknila/releases/latest/download/latest.json`, and downloads it in the
background. It installs only an archive whose signature verifies against the public key built into
the app (`plugins.updater.pubkey` in `apps/desktop/src-tauri/tauri.conf.json`), so a changed
address or a swapped file cannot make it install something that was not signed with the release
key.

When an update is downloaded, a quiet note appears in the corner of the window: "Toknila X.Y.Z is
ready", with a short release note, **Restart to update** and **Later**. Later hides that version's
note until the app is opened again or a newer one arrives. If agents are running, Restart first
says "N agents are running; restarting stops them" and offers **Restart anyway** or **Later**. The
app never restarts on its own while agents run. After the restart the new app replaces the old
daemon (see Updating). Settings, About shows the current version, the time of the last check and a
**Check for updates** button. Nothing is installed without a restart, and nothing is sent
anywhere but a request for `latest.json` and the archive.

An update replaces `Toknila.app` where it is. If that folder needs an administrator (for example
`/Applications` owned by another user), macOS asks for the password.

### Cutting a release

Versions are `X.Y.Z`; a new one is cut after each batch of merged changes.

```sh
cargo xtask bump 0.2.1          # sets the version everywhere, refreshes the lock files and openapi.json
git switch -c release-0.2.1 && git commit -am "Release 0.2.1"
# open a pull request, wait for green, merge it; then on main:
git tag v0.2.1 && git push origin v0.2.1
```

The tag starts the Release workflow. It checks that the tag equals the version, builds the command
line archives for macOS (both architectures), Linux and Windows, builds the Mac app for Apple
silicon and Intel, signs each `Toknila-<version>-<arch>.app.tar.gz` with the updater key from the
repository secrets, writes `latest.json` and the release notes (the titles of the pull requests
merged since the previous tag) and publishes the release with the `.dmg` files, the archives,
the `.sig` files, `latest.json` and `SHA256SUMS.txt`. A release whose archives are not signed
fails instead of publishing. Pushes to `main` only build; pull requests run `ci.yml` (the gate on
three systems and the desktop build), not the release builds.

`cargo xtask app` signs the update archive when `TAURI_SIGNING_PRIVATE_KEY` (and
`TAURI_SIGNING_PRIVATE_KEY_PASSWORD`) are set and skips it, saying so, when they are not. The
signing itself is done by the official Tauri signer (`tauri signer sign`, a pinned version of
`@tauri-apps/cli` run through `npx`), so the signature format is the one the app verifies.

### Where the signing key lives

The updater key pair was generated once with `tauri signer generate`. The private key and its
password are in `~/.toknila-release/` on the maintainer's Mac (`updater.key`, `updater.password`;
folder mode 700, files mode 600) and in the repository's Actions secrets
`TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`. Only the public key is in
the repository. Back up the folder somewhere private; never commit it, paste it into an issue or
print it in a log.

If the key is lost or leaks: generate a new pair (`npx @tauri-apps/cli signer generate -w
~/.toknila-release/updater.key`), replace both secrets with `gh secret set`, put the new public
key in `tauri.conf.json` and cut a release. Apps that carry the old public key cannot verify
anything signed with the new key, so every installed copy needs one manual reinstall from the new
`.dmg`; after that updates work again. Until then, a leaked key lets its holder sign updates the
old apps accept, so replace the secrets first.

### Testing the updater

`scripts/smoke-update.sh` builds version 9.0.0 and 9.0.1 from temporary copies of the working
tree, signs 9.0.1 with a throwaway test key, serves it from a local server and runs 9.0.0 from a
temporary folder with a temporary data home. It checks that a tampered archive is downloaded but
not installed, and that the intact one is installed, the app relaunches as 9.0.1 and the new
daemon replaces the old one. It never uses the release key and never touches `/Applications` or
your data home. Two environment variables exist for it: `TOKNILA_UPDATE_ENDPOINT` replaces the
release address (harmless in a release build: the signature is still checked against the built-in
key, and plain `http` is refused unless the build allows it, which only the test builds do) and
`TOKNILA_UPDATE_AUTO_RESTART=1` restarts once an update is ready and no agent runs.

## macOS, plain binaries

Pick the archive for your Mac: `macos-arm64` for Apple silicon (M1 and later), `macos-x86_64`
for Intel. The binaries are not signed or notarised yet, so download with `curl` (it does not set
the quarantine flag) or clear the flag after a browser download.

```sh
V=0.2.0
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

Work nobody planned does not change the chain by itself. While DEMO-1 runs (before
`tok accept DEMO-1`), its agent can propose a wildcard card; it waits for you:

```sh
tok sim DEMO-1 propose "Token refresh endpoint" --after DEMO-1   # DEMO-5 proposed
tok ls --needs-human                  # DEMO-5 awaits approval; chain progress does not count it
tok approve DEMO-5                    # it joins the plan and starts once DEMO-1 is accepted
tok reject DEMO-5 "not now"           # or turn it down instead: Cancelled with the reason
```

`--before KEY` makes planned work wait for the card; approval is refused if that work has already
started. A person proposes a card with `tok propose demo "<title>" --after DEMO-1 --from DEMO-1`.

Who reviews a finished card is a setting: a person (`human`, the default), a reviewer agent
(`agent`), or the agent first and then you with its remarks in view (`both`). Set it on a project,
a chain or one ticket; the ticket's wins, then the chain's, then the project's:

```sh
tok review-mode both --project DEMO   # every card of the project, unless a chain or ticket says
tok review-mode agent --chain demo    # one chain
tok review-mode human --ticket DEMO-1 # one card
tok sim DEMO-1 finish "A is done"     # hand-in; the simulated reviewer waits for its verdict
tok sim DEMO-1 review request-changes "handle the empty list"   # agent mode: back to the agent
tok accept DEMO-1 "good"              # or, in human and both mode, your verdict; remarks optional
tok show DEMO-1                       # the review rounds so far and whose turn it is
```

The reviewer agent of profile `x` is `x-reviewer` (`claude-code-reviewer` for `claude-code`);
`tok review-mode --reviewer <profile>` picks another. It runs next to the worker, not instead of
it: `tok logs DEMO-1 --reviewer` reads its terminal.

Toknila's own next phase is a chain too: `tok project new TOK && tok chain import phase-1.toml
--project TOK && tok chain show phase-1`.

Spec-driven projects (`specs/<n>-<name>/tasks.md`) import and export the same way; see
[specs.md](specs.md).

## Where things live

| What | Where |
|---|---|
| Data, `daemon.json`, logs | `~/Library/Application Support/toknila` on macOS, `~/.toknila` on Linux, `%APPDATA%\toknila` on Windows; `TOKNILA_HOME` overrides. A macOS `~/.toknila` from an earlier version moves to the new place on the first start after the daemon was stopped |
| Ships (worktrees) | `~/.toknila-ships` for new ships, a path without a space; `TOKNILA_SHIPS_DIR` overrides it. Ships made before keep their place |
| Database | `toknila.db` (SQLite) in that folder |
| Stop the daemon | `tok daemon stop` |
| Start over | stop the daemon, delete the folder |

Every command takes `--json` for scripts.
