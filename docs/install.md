# Install and first run

Toknila runs as an app on a Mac (`Toknila.app`, from the Releases page). On Linux and Windows the
daemon serves the same window in a browser. This page starts with the app from a release, then
Linux and Windows, then building from source, then the sections for whoever cuts releases.

## Install from a release

What you need: a Mac with macOS 10.15 or later (the minimum the app declares), Apple silicon or
Intel; `git`; Claude Code installed and signed in. The GitHub command line (`gh`) is needed only if
accepted work is sent as pull requests. Nothing else is installed.

1. Download the `.dmg` for your Mac from the
   [Releases page](https://github.com/jh0per/Toknila/releases):
   `Toknila-<version>-aarch64.dmg` for Apple silicon, `Toknila-<version>-x86_64.dmg` for Intel.
   Open it and drag Toknila onto Applications. Open the app.
2. **Add a project.** Choose the folder that holds your project, a git repository. If the folder is
   missing or is not a repository yet, Toknila offers to create it or set it up. It fills in the
   main branch, which agents start from and where approved work lands, and under Details the test
   command that agents run before they hand in. Also under Details you choose where accepted work
   lands: merged into the main branch, or sent as a pull request on GitHub.
3. **Plan with the assistant.** Open Assistant under your project and describe your first epic.
   The assistant creates epics and their stories, steers the process and answers your questions.
   An epic holds stories and ends in a release.
4. **Start a story.** Press Start on a story, or S on the diagram. An agent then works on it in
   its own workspace, a separate copy of the repository, so your folder stays as it is. Only as
   many agents run at once as the project allows; other ready stories wait for a free slot.
5. **Read the hand-in.** When the agent is done it hands in: what changed, each check as PASS,
   FAIL, NOT_RUN or BLOCKED, and what is left. The test command runs, then a reviewer agent reads
   the work. Needs you lists what only you can judge; do those by hand first.
6. **Accept.** Accept merges the story into the main branch (or opens a pull request when the
   project is set to). Return sends your words to the agent, which fixes and hands in again.
7. **Release.** When all the stories of an epic are done, a Release button appears at the end of
   its diagram. Check the version (it becomes the tag, like `v0.3.0`) and press Release.

The Guide at the foot of the window's sidebar says the same with pictures of the window.

If macOS refuses to open the app, open System Settings, Privacy & Security and choose Open Anyway
for Toknila. This is needed only for builds made before the first signed and notarized release;
a signed and notarized app opens without a warning.

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

## Linux and Windows

There is no desktop app for these systems yet. The release archives hold the daemon (`toknila`)
and the command line (`tok`): `toknila-<version>-linux-x86_64.tar.gz` and
`toknila-<version>-windows-x86_64.zip` (run `tok.exe` from PowerShell). Unpack one, put `tok` and
`toknila` on your `PATH`, and make sure `git` and Claude Code are installed and signed in.

```sh
tok daemon start     # the background daemon, on 127.0.0.1
tok open             # prints the address of the window; open it in a browser
```

The browser shows the same window as the Mac app, and the command line is complete. Data lives in
`~/.toknila` on Linux and `%APPDATA%\toknila` on Windows (see Where things live below).

### macOS, plain binaries

The same archives exist for a Mac (`macos-arm64` for Apple silicon, `macos-x86_64` for Intel), for
whoever wants only the command line. They are not signed or notarised yet, so download with `curl`
(it does not set the quarantine flag) or clear the flag after a browser download. Take the version
from the Releases page.

```sh
V=<version>
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

## Build from source

Needs a stable Rust toolchain (`rust-toolchain.toml` pins the channel and components) and Node 22.

```sh
cargo xtask web             # build the web page the daemon embeds
cargo xtask verify          # fmt check, clippy -D warnings, tests, architecture guards
cargo xtask e2e-web         # browser tests of the page (Playwright)
cargo run --bin tok -- --version
cargo xtask dist aarch64-apple-darwin   # release archive in dist/
```

### The Mac app (built locally)

The app is built on the Mac that runs it, for that Mac's architecture. It is signed ad hoc (no
Developer ID, not notarised), which is enough to run it where it was built. Needs the Rust
toolchain, Node 22 and the Xcode command line tools. (The app of a release is signed with a
Developer ID and notarized on the maintainer's Mac: see Signing and notarizing the Mac app.)

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
`xattr -dr com.apple.quarantine /Applications/Toknila.app`. The sections above (First run, The
`tok` command, Backups, Updating, Removing it) apply to this app too.

### Check a build

`scripts/smoke-app.sh` mounts the `.dmg`, copies the app to a temporary folder, launches it against
a temporary data home, checks the daemon, the version, a project and a card made with the bundled
`tok`, and that no process is left after quitting. It does not touch your data home.

## First run from the command line: the demo chain

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

Releases belong to one epic each. A version is planned by one unreleased epic of the project:
creating or editing an epic with a version another one plans is refused ("0.3.4 is planned by
phase-10; pick another version or move that one"). A release run that tags `vX` on the base branch
also marks every other finished epic of the project whose story merges the tag contains (every
story Merged, none open or Released) as released with it: its stories become Released, its Finish
node reads "released with vX by <epic>", and a version it planned higher becomes X. For a tag that
already exists, say it yourself:

```sh
tok release mark phase-10 --with v0.3.3   # refused when the tag is missing or lacks the epic's merges
tok release steps phase-10                # a release is also addressed by its epic's name
tok release run phase-10                  # the version defaults to the epic's own
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

## Where things live on every system

| What | Where |
|---|---|
| Data, `daemon.json`, logs | `~/Library/Application Support/toknila` on macOS, `~/.toknila` on Linux, `%APPDATA%\toknila` on Windows; `TOKNILA_HOME` overrides. A macOS `~/.toknila` from an earlier version moves to the new place on the first start after the daemon was stopped |
| Ships (worktrees) | `~/.toknila-ships` for new ships, a path without a space; `TOKNILA_SHIPS_DIR` overrides it. Ships made before keep their place |
| Database | `toknila.db` (SQLite) in that folder |
| Stop the daemon | `tok daemon stop` |
| Start over | stop the daemon, delete the folder |

Every command takes `--json` for scripts.

New to Toknila? Open **Guide** at the foot of the window's sidebar: it walks the work from a folder to a release.

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
line archives for macOS (both architectures), Linux and Windows, waits for CI on the tag's commit,
writes `latest.json` and the release notes (the titles of the pull requests merged since the
previous tag) and publishes the release with the `.dmg` files, the archives, the `.sig` files,
`latest.json` and `SHA256SUMS.txt`. Where the Mac app comes from is the repository variable
`MAC_APP_SOURCE` (a variable, not a secret; its case does not matter, `CI` is `ci`):

- unset or `ci`, the default: CI builds the Mac app for Apple silicon and Intel and signs each
  `Toknila-<version>-<arch>.app.tar.gz` with the updater key from the repository secrets. The app
  is not notarized.
- `release`: the maintainer's Mac builds the app, signs it with the Developer ID, notarizes it and
  uploads the Mac files to a draft release (`cargo xtask mac-release`, the release's Build step;
  see Signing and notarizing the Mac app). CI runs no Mac app step and no step that receives a
  signing secret. Its `publish` job waits up to 60 minutes for the six Mac files on the draft (each
  completely uploaded; a GitHub error during the wait is retried until then), adds its own files
  and publishes. If they are not there in time it fails with one sentence: "The Mac files are not
  on the release vX.Y.Z after 60 minutes (missing: …): run the release's Build step on the Mac
  (cargo xtask mac-release vX.Y.Z), then re-run this job." A release that is already published
  is never touched (a re-run, or a manual run of a tag that is out): "The release vX.Y.Z is
  already published: this job never replaces the files of a public release …".

A release whose archives are not signed fails instead of publishing. The publishing is
`.github/scripts/assemble_release.py`; its tests run in `cargo xtask verify`. Pushes to `main` only
build; pull requests run `ci.yml` (the gate on three systems and the desktop build), not the
release builds.

`cargo xtask app` signs the update archive when `TAURI_SIGNING_PRIVATE_KEY` (and
`TAURI_SIGNING_PRIVATE_KEY_PASSWORD`) are set and skips it, saying so, when they are not. The
signing itself is done by the official Tauri signer (`tauri signer sign`, a pinned version of
`@tauri-apps/cli` run through `npx`), so the signature format is the one the app verifies.

### Where the signing key lives

The updater key pair was generated once with `tauri signer generate`. The private key and its
password are in `~/.toknila-release/` on the maintainer's Mac (`updater.key`, `updater.password`;
folder mode 700, files mode 600); `cargo xtask mac-release` signs the archives with them there.
Until the first release made with `MAC_APP_SOURCE=release` they are also in the repository's
Actions secrets `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`, which the
`ci` mode uses; delete them after it (see After the first signed release). Only the public key is
in the repository. Back up the folder somewhere private; never commit it, paste it into an issue
or print it in a log.

If the key is lost or leaks: generate a new pair (`npx @tauri-apps/cli signer generate -w
~/.toknila-release/updater.key`), replace the secrets with `gh secret set` if they still exist,
put the new public key in `tauri.conf.json` and cut a release. Apps that carry the old public key
cannot verify anything signed with the new key, so every installed copy needs one manual reinstall
from the new `.dmg`; after that updates work again. Until then, a leaked key lets its holder sign
updates the old apps accept, so replace the key first.

### Signing and notarizing the Mac app

A dmg downloaded from a release opens without a warning only when its app is signed with a
Developer ID and notarized by Apple. A stable identity is also what keeps the privacy permissions
macOS grants (every entry in System Settings, Privacy & Security that works that way: Microphone,
Files and Folders, Automation, Accessibility and the rest) across updates: an ad hoc signature
(`codesign -dv` shows `Signature=adhoc`) is a new identity for every build. Both happen on the
maintainer's Mac, so no signing material has to live on GitHub. `cargo xtask mac-release vX.Y.Z`,
run by the release's Build step (the project's artifact command), does this for Apple silicon and
then Intel:

1. builds `toknila`, `tok` and the window shell and assembles `Toknila.app`;
2. signs `toknila`, `tok` and then the bundle with the Developer ID under the hardened runtime and
   with a secure timestamp (`codesign --force --options runtime --timestamp`); the bundle's main
   executable gets `apps/desktop/src-tauri/entitlements.plist`; then `codesign --verify --strict`;
3. makes the dmg and signs it;
4. submits the dmg to Apple's notary service and waits (`xcrun notarytool submit --wait`); on
   anything but Accepted it prints the submission's log and stops, and nothing is uploaded;
5. staples the ticket to the app and to the dmg, validates the dmg (`xcrun stapler validate`) and
   asks Gatekeeper about the app (`spctl -a -vv` must say "Notarized Developer ID");
6. packs `Toknila-<version>-<arch>.app.tar.gz` from the stapled app and signs it with the updater
   key.

Then it uploads the two dmg and the two archives, and last their `.sig` files, to the release on
GitHub, creating the release as a draft when the tag has none. On a draft that already has
signatures (a second run) it deletes them first, so CI, which publishes only once all six files are
there, never takes new archives with old signatures. A published release is refused: its
`latest.json` carries the signatures of the archives it has. Before any build it checks, in
seconds, that the tag is on GitHub and its release is not published. With `--no-upload` it stops
before GitHub (and does not ask it anything), which is how to try the certificate and the notary
profile.

There is one notarization per architecture: Apple tickets the dmg and the code inside it, so the
app outside the dmg is stapled too and the updater archive carries a stapled app. The app inside
the dmg is not stapled itself; Gatekeeper finds its ticket online when it is first opened.

`cargo xtask app` stays the developer's build: ad hoc, nothing to set up, not notarized.

#### What the app is allowed (entitlements)

The hardened runtime blocks the microphone unless the app has the audio-input entitlement, so
`entitlements.plist` grants exactly one: `com.apple.security.device.audio-input`, for the
microphone button (WebKit's speech recognition records through the app). The Info.plist keys
`NSMicrophoneUsageDescription` and `NSSpeechRecognitionUsageDescription` say why it listens;
speech recognition has no entitlement of its own. Left out, because nothing needs them: JIT
(`com.apple.security.cs.allow-jit`, `allow-unsigned-executable-memory`; the window runs JavaScript
in WebKit's own process, which carries its own JIT entitlement), `disable-library-validation` (no
plug-ins signed by others), DYLD variables, camera, location, contacts, calendars, photos, and
Apple events (`com.apple.security.automation.apple-events`; nothing in Toknila sends them, so an
agent that drives another app with `osascript` is refused that). `get-task-allow` is never there:
notarization refuses it. The helpers `toknila` and `tok` run under the hardened runtime with no
entitlement; what the daemon starts (`git`, `gh`, the agents) runs under its own signature.

Sources: Apple's documentation pages Hardened Runtime, Configuring the hardened runtime, the
`audio-input` and `allow-jit` entitlements, `NSMicrophoneUsageDescription`,
`NSSpeechRecognitionUsageDescription`, Notarizing macOS software before distribution, Customizing
the notarization workflow, Resolving common notarization issues and Placing content in a bundle;
WebKit bug 184485 (the JIT entitlement of WebKit's web content process).

#### Settings

Names and paths only; none of it is in the repository. `cargo xtask mac-release` reads each one
from its environment first, then from `~/.toknila-release/mac-release.env` (`NAME=value` lines,
`#` comments; `TOKNILA_RELEASE_DIR` names another folder):

| Setting | What it names | Default |
|---|---|---|
| `APPLE_SIGNING_IDENTITY` | the certificate, `Developer ID Application: <name> (<team id>)` | required |
| `TOKNILA_NOTARY_PROFILE` | the keychain profile saved with `xcrun notarytool store-credentials` | required |
| `TAURI_SIGNING_PRIVATE_KEY_PATH` | the updater key | `updater.key` in the folder |
| `TOKNILA_UPDATER_PASSWORD_PATH` | the file with the key's password | `updater.password` in the folder |
| `CARGO_TARGET_DIR` | a build folder outside the repository, kept between releases | none: each release builds cold |

No value is printed. The command lines it shows say `<APPLE_SIGNING_IDENTITY>` and the like, and
every line the tools print passes a filter that puts the setting's name in place of the
identity, its team ID, the profile, the key's path and the password. Pick a profile name that is
not a common word (`toknila-notary`), or the filter hides that word everywhere. The password
reaches the signer only through its environment, never as an argument.

The build cache: a release runs in a new worktree each time, so without `CARGO_TARGET_DIR` both
architectures build from nothing. A cold `cargo xtask app` (one architecture, web page included)
took about 6 minutes on an Apple silicon Mac (345 s, 2026-11-17); a release builds two
architectures, so expect about 12 minutes before the notarization. With `CARGO_TARGET_DIR` set to a folder such as
`~/Library/Caches/toknila-release-target`, later releases rebuild only what changed.

#### Once, on the Mac that releases

1. Keychain Access → Certificate Assistant → Request a Certificate From a Certificate Authority:
   your email address, "Saved to disk".
2. developer.apple.com → Certificates, Identifiers & Profiles → Certificates → **+**, choose
   "Developer ID Application", upload the request, download the certificate and double-click it.
   It goes into the login keychain next to its private key; `security find-identity -v -p
   codesigning` lists `Developer ID Application: <name> (<team id>)`.
3. account.apple.com → Sign-In and Security → App-Specific Passwords: make one named
   "notarytool". Then run `xcrun notarytool store-credentials toknila-notary --apple-id <your
   Apple ID> --team-id <team id>` and paste that password when asked; it stays in the keychain.
4. `rustup target add aarch64-apple-darwin x86_64-apple-darwin` (the command names a missing one
   otherwise).
5. Write `~/.toknila-release/mac-release.env` (mode 600):

   ```sh
   APPLE_SIGNING_IDENTITY=Developer ID Application: <name> (<team id>)
   TOKNILA_NOTARY_PROFILE=toknila-notary
   CARGO_TARGET_DIR=/Users/<you>/Library/Caches/toknila-release-target
   ```

6. `gh auth status`: logged in, with a token that may write releases.
7. Try it in a checkout of the current tag: `cargo xtask mac-release v<version> --no-upload`. The
   first signature makes macOS show a window: "codesign wants to access key … in your keychain".
   Enter the login password and press **Always Allow**; otherwise it asks at every signature, and
   the release's Build step runs where nobody answers. That is why this try comes before the first
   real release. It ends with "xtask mac-release: PASS in N min", and nothing goes to GitHub.
8. In Toknila, the project's Settings: artifact command `cargo xtask mac-release {tag}`.
9. On GitHub, the repository's Settings → Secrets and variables → Actions → Variables → New
   repository variable: `MAC_APP_SOURCE`, value `release`.

#### At each release

1. Run the release from Toknila as usual. Keep the Mac awake and online during Build:
   notarization takes minutes.
2. Build ends with "xtask mac-release: PASS" and the six files. The GitHub release stays a draft
   until the Release workflow publishes it, once CI passed and the Mac files are there (it waits up
   to 60 minutes).
3. Check a download: on the release page, download the dmg in a browser. `xcrun stapler validate
   ~/Downloads/Toknila-<version>-aarch64.dmg` says "The validate action worked!"; open the dmg;
   `spctl -a -vv /Volumes/Toknila/Toknila.app` says "accepted" and "source=Notarized Developer
   ID"; the app opens without a warning.
4. If Build failed: fix the cause, press Build again in the Release dialog, then re-run the failed
   `publish` job on GitHub if it already gave up.

What a failure says: a keychain window that was denied or not answered, "codesign could not use
the certificate's private key: the keychain window that asks for it was denied or not answered";
no such certificate, "codesign found no certificate named by APPLE_SIGNING_IDENTITY in the
keychain"; a refused notarization, "Apple's notary service answered Invalid for Toknila-….dmg
(submission …); its log is above"; a missing Rust target, "… run rustup target add …"; a missing
setting names the setting; a tag that was not pushed, "the tag vX.Y.Z is not on GitHub: push it
(git push origin vX.Y.Z), then run the Build step again"; a published release, "the release
vX.Y.Z is already published".

#### Going back to CI's build

Set the repository variable `MAC_APP_SOURCE` to `ci` (or delete it): the next release builds the
Mac app in CI again, with the Actions secrets, which must still exist. In Toknila, clear the
project's artifact command (`cargo xtask mac-release {tag}`), or the next release's Build step
uploads a draft again. If a release was started in `release` mode and not published, its draft is
left on GitHub: delete it by hand first (the repository's Releases page, the draft, Delete),
keeping the tag; otherwise `publish` fails with "A draft release vX.Y.Z exists (the Mac's upload)
but MAC_APP_SOURCE is not release". Then run the Release workflow again for that tag (Actions →
Release → Run workflow, with the tag): a re-run of `publish` alone has no Mac files, because
`build` skipped them.

#### After the first signed release

Delete the Actions secrets the `ci` mode used: `gh secret delete TAURI_SIGNING_PRIVATE_KEY`,
`gh secret delete TAURI_SIGNING_PRIVATE_KEY_PASSWORD`, and `APPLE_CERTIFICATE`,
`APPLE_CERTIFICATE_PASSWORD` and `APPLE_SIGNING_IDENTITY` if they were ever set. A later change
removes the `ci` path and these names from the workflow. The first update from an ad hoc build to
the signed app asks for the microphone once more, because the identity changed; later updates keep
it. Keep the certificate: every later build must carry the same identity.

#### Signing in CI, until the switch (`MAC_APP_SOURCE` unset or `ci`)

The Release workflow can sign CI's (not notarized) build with a certificate from three secrets:
`APPLE_CERTIFICATE` (the `.p12`, base64), `APPLE_CERTIFICATE_PASSWORD` and
`APPLE_SIGNING_IDENTITY`. It imports the certificate into a temporary keychain, `cargo xtask app`
signs `dist/app/Toknila.app` with it, the keychain is deleted, and the step "Check the app
signature" fails the release when the secrets were given and the app is still ad hoc. Without the
secrets the app is signed ad hoc and both say so.

On macOS the microphone button also needs Dictation or Siri turned on in System Settings →
Keyboard → Dictation; otherwise the button says so instead of listening.

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
