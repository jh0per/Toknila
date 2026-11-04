# apps/desktop

The Tauri 2 shell: a window around the web page the daemon serves.

On start it runs the bundled `toknila` binary (`daemon start`, then `open --json`), so it starts
the daemon or attaches to the running one exactly like the CLI, and opens a window on the page
address with its token. Closing the window leaves the daemon and its agents running. If the daemon
cannot start, the window shows the reason from `static/index.html`.

```sh
cargo xtask desktop   # builds the web page and toknila, copies the sidecar, clippy + build of the shell
```

`src-tauri` is a separate Cargo workspace: it needs the platform web view (WebKit on macOS, the
first target; WebView2 on Windows; `libwebkit2gtk-4.1-dev` and `libgtk-3-dev` on Linux), which the
main workspace does not assume. CI builds it on macOS. Bundles (`.app`, `.dmg`, installers) come
with the release work of phase 2; `cargo tauri build` produces them from this folder.
