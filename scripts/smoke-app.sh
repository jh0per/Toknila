#!/bin/bash
# Smoke test of the macOS app bundle, on a copy of it and a temporary data home only.
#
#   scripts/smoke-app.sh [path/to/Toknila-<version>.dmg]    (default: dist/app/Toknila-*.dmg)
#
# Mounts the .dmg, copies Toknila.app to a temp folder (a name with a space, like a real install
# path), launches it with TOKNILA_HOME pointing into the temp folder, then checks that the daemon
# answers /healthz, that the bundled `tok` reports the same version and can create a project and
# a card, and that quitting leaves no process behind. It never installs into /Applications, never
# touches the real data home or the real agent configuration, and starts no real agent session
# (the project uses the built-in simulated profile).
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
dmg="${1:-$(ls "$root"/dist/app/Toknila-*.dmg | head -n 1)}"
[ -f "$dmg" ] || { echo "FAIL: no .dmg at '$dmg'; run: cargo xtask app" >&2; exit 2; }
want="$(sed -n '/^\[workspace.package\]/,/^\[/{s/^version = "\(.*\)"/\1/p;}' "$root/Cargo.toml")"

tmp="$(mktemp -d "${TMPDIR:-/tmp}/toknila-smoke.XXXXXX")"
tmp="$(cd "$tmp" && pwd -P)"   # the resolved path is what process listings show
mnt="$tmp/mnt"
home="$tmp/data home"
copy="$tmp/with space"
app="$copy/Toknila.app"
mkdir -p "$mnt" "$home" "$copy"
tok="$app/Contents/MacOS/tok"


# The app, the daemon and anything they started: every process whose command line mentions the
# temp folder.
leftovers() { pgrep -f "$tmp" || true; }

cleanup() {
  status=$?
  if [ -x "$tok" ]; then TOKNILA_HOME="$home" "$tok" daemon stop >/dev/null 2>&1 || true; fi
  pkill -f "$app/Contents/MacOS/" 2>/dev/null || true
  hdiutil detach "$mnt" -quiet 2>/dev/null || true
  if [ "$status" -eq 0 ]; then rm -rf "$tmp"; else echo "kept $tmp for inspection" >&2; fi
  exit "$status"
}
trap cleanup EXIT

step() { printf '\n== %s\n' "$*"; }
fail() { echo "FAIL: $*" >&2; exit 1; }

step "mount $dmg and copy the app to a temp folder"
hdiutil attach "$dmg" -nobrowse -readonly -mountpoint "$mnt" -quiet
ditto "$mnt/Toknila.app" "$app"
hdiutil detach "$mnt" -quiet
codesign --verify --strict "$app" && echo "signature verifies (ad-hoc)"
[ -x "$tok" ] || fail "no tok in $app/Contents/MacOS"
[ -x "$app/Contents/MacOS/toknila" ] || fail "no toknila (daemon sidecar) in the bundle"

step "tok --version"
got="$("$tok" --version)"
echo "$got"
case "$got" in *"$want"*) ;; *) fail "expected version $want";; esac

step "an old daemon is running (same version, another binary), then launch the app"
mkdir -p "$tmp/old"
cp "$app/Contents/MacOS/toknila" "$tmp/old/toknila"
TOKNILA_HOME="$home" "$tmp/old/toknila" daemon start
old_pid="$(TOKNILA_HOME="$home" "$tok" daemon status --json | sed -n 's/.*"pid": *\([0-9]*\).*/\1/p')"
[ -n "$old_pid" ] || fail "the old daemon did not start"
echo "old daemon pid $old_pid from $tmp/old/toknila"
echo "launch the app with TOKNILA_HOME=$home"
open -n --env "TOKNILA_HOME=$home" "$app"
# The app must stop the old daemon and start its own, from the bundle.
for _ in $(seq 1 80); do
  if TOKNILA_HOME="$home" "$tok" daemon status --json 2>/dev/null | grep -qF "\"exe\":\"$app/Contents/MacOS/toknila\""; then took=1; break; fi
  sleep 0.5
done
[ -n "${took:-}" ] || fail "the app did not take over from the old daemon"
for _ in $(seq 1 20); do kill -0 "$old_pid" 2>/dev/null || break; sleep 0.5; done
if kill -0 "$old_pid" 2>/dev/null; then fail "the old daemon (pid $old_pid) is still alive"; fi
echo "the app replaced the old daemon"
pgrep -f "$app/Contents/MacOS/toknila-desktop" >/dev/null || fail "the app is not running"

step "daemon status and /healthz"
status="$(TOKNILA_HOME="$home" "$tok" daemon status --json | tr -d '\n')"
echo "$status"
port="$(printf '%s' "$status" | sed -n 's/.*"port": *\([0-9]*\).*/\1/p')"
[ -n "$port" ] || fail "the daemon reports no port"
health="$(curl -fsS "http://127.0.0.1:$port/healthz")"
echo "$health"
case "$health" in *'"ok":true'*"\"version\":\"$want\""*) ;; *) fail "healthz does not say ok and $want";; esac
case "$status" in *"$home"*) ;; *) fail "the daemon does not use the temp home";; esac

step "create a project and a card with the bundled tok"
export TOKNILA_HOME="$home"
"$tok" project new SMK --name "Smoke"
"$tok" chain new smoke --project SMK --no-autostart
"$tok" ticket new smoke "Smoke card"
listing="$("$tok" ls --json)"
case "$listing" in *"SMK-1"*) echo "ticket SMK-1 is listed" ;; *) fail "SMK-1 is not listed: $listing";; esac

step "agent wiring points at the bundled binary"
# The daemon runs the hooks, the MCP server and the status line through its own binary.
ps -o command= -p "$(pgrep -f "$app/Contents/MacOS/toknila daemon run" | head -n 1)" | grep -F "$app/Contents/MacOS/toknila" >/dev/null \
  || fail "the daemon does not run from the bundle"
echo "daemon runs from $app/Contents/MacOS/toknila"

step "quit the app and stop its daemon; no process may be left"
pkill -TERM -f "$app/Contents/MacOS/toknila-desktop" || true
sleep 1
"$tok" daemon stop || true
for _ in $(seq 1 20); do
  [ -z "$(leftovers)" ] && break
  sleep 0.5
done
left="$(leftovers)"
if [ -n "$left" ]; then
  ps -o pid=,command= -p "$(echo "$left" | tr '\n' ',' | sed 's/,$//')" >&2 || true
  fail "processes left behind"
fi
echo "no process left"

step "PASS (app $want, temp home only)"
