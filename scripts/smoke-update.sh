#!/usr/bin/env bash
# End-to-end test of the in-app updater, on this Mac only.
#
# Builds the app twice from temporary copies of the working tree: version A (9.0.0) and version B
# (9.0.1, signed with a throwaway test key). Serves B's `latest.json` and archive from a local HTTP
# server, launches A from a temporary folder with a temporary data home, and checks that
#   1. a tampered archive (valid manifest, changed bytes) is downloaded but NOT installed, and
#   2. the intact archive is installed, the app relaunches as B, and the takeover leaves a daemon
#      of version B.
# The real signing key is never used: the test key is generated here, and A is built with its public
# key (TAURI_CONFIG merges it over the one in tauri.conf.json for these builds only). The endpoint
# override (TOKNILA_UPDATE_ENDPOINT) and the plain-http permission exist only for this test: the
# override is safe to ship because the signature is still checked against the embedded public key,
# and plain http is refused unless the build was made with dangerousInsecureTransportProtocol.
#
# It never installs into /Applications, never touches the real data home or the real agent
# configuration, and starts no agent. Needs: Rust, Node 22, npx (network on first use, to fetch the
# pinned Tauri CLI), python3, the Xcode command line tools. Takes several minutes (two builds).
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
export PATH="$HOME/.cargo/bin:$PATH"
tauri_cli="@tauri-apps/cli@2.12.4"
ver_a="9.0.0"
ver_b="9.0.1"

case "$(uname -m)" in
  arm64) arch="aarch64" ;;
  x86_64) arch="x86_64" ;;
  *) echo "FAIL: unsupported machine $(uname -m)" >&2; exit 2 ;;
esac

tmp="$(mktemp -d "${TMPDIR:-/tmp}/toknila-update.XXXXXX")"
tmp="$(cd "$tmp" && pwd -P)"
export CARGO_TARGET_DIR="${TOKNILA_SMOKE_TARGET:-$root/target}"
home="$tmp/data home"
run="$tmp/with space"
app="$run/Toknila.app"
serve="$tmp/serve"
mkdir -p "$home" "$run" "$serve"
server_pid=""

step() { printf '\n== %s\n' "$*"; }
fail() { echo "FAIL: $*" >&2; exit 1; }

quit_app() {
  pkill -TERM -f "$app/Contents/MacOS/toknila-desktop" 2>/dev/null || true
  if [ -x "$app/Contents/MacOS/tok" ]; then
    TOKNILA_HOME="$home" "$app/Contents/MacOS/tok" daemon stop >/dev/null 2>&1 || true
  fi
  pkill -f "$app/Contents/MacOS/" 2>/dev/null || true
}

cleanup() {
  status=$?
  quit_app
  [ -z "$server_pid" ] || kill "$server_pid" 2>/dev/null || true
  if [ "$status" -eq 0 ]; then rm -rf "$tmp"; else echo "kept $tmp for inspection" >&2; fi
  exit "$status"
}
trap cleanup EXIT

step "throwaway test key (not the release key)"
npx --yes "$tauri_cli" signer generate -w "$tmp/test.key" -p "smoke-test" >/dev/null 2>&1
pub="$(tr -d '\n' < "$tmp/test.key.pub")"
test_config="{\"plugins\":{\"updater\":{\"pubkey\":\"$pub\",\"dangerousInsecureTransportProtocol\":true}}}"

port="$(python3 -I -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1",0)); print(s.getsockname()[1])')"
base="http://127.0.0.1:$port"

# A copy of the working tree (tracked and new files, no build output) at the given version.
build_version() {
  local version="$1" sign="$2" src="$tmp/src-$1"
  mkdir -p "$src"
  (cd "$root" && git ls-files -z --cached --others --exclude-standard | tar --null -T - -cf -) | tar -xf - -C "$src"
  local before; before="$(cksum "$root/Cargo.toml" "$root/apps/web/package.json")"
  (cd "$src" && cargo xtask bump "$version")
  [ "$before" = "$(cksum "$root/Cargo.toml" "$root/apps/web/package.json")" ] || fail "bump changed the real working tree"
  grep -q "^version = \"$version\"" "$src/Cargo.toml" || fail "bump did not change the copy"
  if [ "$sign" = yes ]; then
    (cd "$src" && TAURI_CONFIG="$test_config" \
      TAURI_SIGNING_PRIVATE_KEY="$(cat "$tmp/test.key")" TAURI_SIGNING_PRIVATE_KEY_PASSWORD="smoke-test" \
      TOKNILA_UPDATE_BASE_URL="$base" TOKNILA_RELEASE_NOTES="Smoke test release $version" \
      cargo xtask app)
  else
    (cd "$src" && env -u TAURI_SIGNING_PRIVATE_KEY -u TAURI_SIGNING_PRIVATE_KEY_PASSWORD \
      TAURI_CONFIG="$test_config" cargo xtask app)
  fi
}

step "build version $ver_a (the app that will be updated)"
build_version "$ver_a" no
ditto "$tmp/src-$ver_a/dist/app/Toknila.app" "$app"

step "build version $ver_b (signed with the test key)"
build_version "$ver_b" yes
b_dist="$tmp/src-$ver_b/dist/app"
archive="Toknila-$ver_b-$arch.app.tar.gz"
[ -f "$b_dist/$archive" ] && [ -f "$b_dist/$archive.sig" ] && [ -f "$b_dist/latest.json" ] \
  || fail "version $ver_b did not produce the archive, its signature and latest.json"
grep -q "\"darwin-$arch\"" "$b_dist/latest.json" || fail "latest.json has no darwin-$arch entry"
grep -q "\"version\": \"$ver_b\"" "$b_dist/latest.json" || fail "latest.json does not say $ver_b"
cp "$b_dist/latest.json" "$serve/latest.json"

step "serve version $ver_b from $base"
python3 -I -m http.server "$port" --bind 127.0.0.1 --directory "$serve" > "$tmp/server.log" 2>&1 &
server_pid=$!
for _ in $(seq 1 20); do curl -fsS "$base/latest.json" >/dev/null 2>&1 && break; sleep 0.25; done
curl -fsS "$base/latest.json" >/dev/null || fail "the local server does not answer"

tok="$app/Contents/MacOS/tok"
daemon_version() {
  local status port
  status="$(TOKNILA_HOME="$home" "$tok" daemon status --json 2>/dev/null | tr -d '\n')" || return 1
  port="$(printf '%s' "$status" | sed -n 's/.*"port": *\([0-9]*\).*/\1/p')"
  [ -n "$port" ] || return 1
  curl -fsS "http://127.0.0.1:$port/healthz" 2>/dev/null | sed -n 's/.*"version":"\([^"]*\)".*/\1/p'
}
plist_version() { /usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "$app/Contents/Info.plist"; }
launch() {  # launch [no-restart]: the app restarts by itself when the update is ready, unless told not to
  local auto=(--env "TOKNILA_UPDATE_AUTO_RESTART=1")
  [ "${1:-}" = no-restart ] && auto=()
  open -n --env "TOKNILA_HOME=$home" --env "TOKNILA_UPDATE_ENDPOINT=$base/latest.json" ${auto[@]+"${auto[@]}"} "$app"
}
wait_daemon() {  # wait_daemon <version> <tries>
  for _ in $(seq 1 "$2"); do
    [ "$(daemon_version || true)" = "$1" ] && return 0
    sleep 0.5
  done
  return 1
}

step "negative: a tampered archive is downloaded but never installed"
cp "$b_dist/$archive" "$serve/$archive"
printf 'tampered' >> "$serve/$archive"
launch
wait_daemon "$ver_a" 80 || fail "app $ver_a did not start its daemon"
for _ in $(seq 1 60); do grep -q "GET /$archive" "$tmp/server.log" && break; sleep 0.5; done
grep -q "GET /$archive" "$tmp/server.log" || fail "the app never asked for the archive (log: $(cat "$tmp/server.log"))"
sleep 5
[ "$(plist_version)" = "$ver_a" ] || fail "the tampered archive was installed (bundle says $(plist_version))"
[ "$(daemon_version)" = "$ver_a" ] || fail "the daemon changed after a tampered archive"
pgrep -f "$app/Contents/MacOS/toknila-desktop" >/dev/null || fail "the app is gone after a tampered archive"
echo "the app asked for the archive, refused it, and still runs $ver_a"
quit_app
for _ in $(seq 1 20); do [ -z "$(pgrep -f "$app/Contents/MacOS/" || true)" ] && break; sleep 0.5; done

if [ -n "${TOKNILA_SMOKE_SCREENSHOT:-}" ]; then
  step "optional: the update note in the window (screenshot to $TOKNILA_SMOKE_SCREENSHOT)"
  cp "$b_dist/$archive" "$serve/$archive"
  launch no-restart
  wait_daemon "$ver_a" 80 || fail "app $ver_a did not start its daemon"
  sleep 30   # the download, then the page's next poll of the shell
  screencapture -x "$TOKNILA_SMOKE_SCREENSHOT"
  quit_app
  for _ in $(seq 1 20); do [ -z "$(pgrep -f "$app/Contents/MacOS/" || true)" ] && break; sleep 0.5; done
fi

step "positive: the intact archive installs and the app relaunches as $ver_b"
cp "$b_dist/$archive" "$serve/$archive"
cmp "$b_dist/$archive" "$serve/$archive"
launch
wait_daemon "$ver_a" 80 || fail "app $ver_a did not start its daemon"
old_pid="$(TOKNILA_HOME="$home" "$tok" daemon status --json | sed -n 's/.*"pid": *\([0-9]*\).*/\1/p')"
echo "running $ver_a, daemon pid $old_pid"
wait_daemon "$ver_b" 160 || fail "no daemon of version $ver_b after the update (kept the log in $tmp/server.log)"
echo "daemon of version $(daemon_version)"
[ "$(plist_version)" = "$ver_b" ] || fail "the bundle says $(plist_version), expected $ver_b"
codesign --verify --strict "$app" || fail "the updated app's signature does not verify"
"$tok" --version | grep -qF "$ver_b" || fail "tok in the updated app is not $ver_b"
for _ in $(seq 1 20); do kill -0 "$old_pid" 2>/dev/null || break; sleep 0.5; done
if kill -0 "$old_pid" 2>/dev/null; then fail "the old daemon (pid $old_pid) is still alive"; fi
pgrep -f "$app/Contents/MacOS/toknila-desktop" >/dev/null || fail "the relaunched app is not running"
echo "relaunched as $ver_b; the old daemon is gone; the signature verifies"

step "PASS (updated $ver_a -> $ver_b, temp folders only)"
