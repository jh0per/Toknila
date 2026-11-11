//! `cargo xtask <task>`: repository automation that must behave the same locally and in CI.
//!
//! - `verify`: fmt check, clippy with warnings denied, tests, architecture guards.
//! - `guards`: architecture guards only (see [`guards`]).
//! - `web`: install `apps/web`, check its generated API types, run its reducer tests, type-check
//!   and build it (needs Node), so the daemon embeds it.
//!   Run it before `verify` or a release build; CI does.
//! - `e2e-web`: browser smoke test of the page against the real `tok` binary (Playwright).
//!   Runs every `apps/web/e2e/*.mjs` spec, rebuilds the bundle when it is stale.
//!   With `CI` set it first installs Chromium; locally it uses an installed one.
//! - `dist <target>`: release archive for one target triple in `dist/` (web page embedded).
//! - `app [--target <triple>]`: the macOS `Toknila.app`, `.dmg` and the signed update archive
//!   (see [`app`] and [`updates`]).
//! - `bump <X.Y.Z>`: set the version everywhere, regenerate locks and the API document.
//! - `latest-json`, `release-notes`: the release's updater manifest and notes (CI uses them).
//! - `desktop`: build the Tauri shell in `apps/desktop` with `toknila` as its sidecar, and run
//!   clippy on it (needs the platform web view; CI runs it on macOS).

use std::process::{Command, ExitCode};
use std::time::Duration;

mod app;
mod bump;
mod guards;
mod leaks;
mod updates;
mod versions;

fn main() -> ExitCode {
    let task = std::env::args().nth(1).unwrap_or_default();
    let result = match task.as_str() {
        "verify" => verify(),
        "guards" => guards(),
        // The leak check alone, over daemons of any age (to see it fail on a daemon you left).
        "leaks" => leaks::check(Duration::from_secs(u32::MAX.into())),
        "web" => web(),
        "e2e-web" => e2e_web(),
        "dist" => dist(std::env::args().nth(2)),
        "desktop" => desktop(),
        "app" => app::app(),
        "bump" => bump::bump(std::env::args().nth(2)),
        "latest-json" => updates::latest_json_cmd(&std::env::args().skip(2).collect::<Vec<_>>()),
        "release-notes" => std::env::args()
            .nth(2)
            .ok_or_else(|| "usage: cargo xtask release-notes <tag>".to_owned())
            .and_then(|tag| updates::release_notes(&tag))
            .map(|notes| print!("{notes}")),
        _ => {
            eprintln!(
                "usage: cargo xtask <verify|guards|web|e2e-web|desktop|app [--target <triple>]|dist <target>|bump <X.Y.Z>|latest-json|release-notes <tag>>"
            );
            return ExitCode::from(2);
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(msg) => {
            eprintln!("xtask {task}: FAIL: {msg}");
            ExitCode::FAILURE
        }
    }
}

fn verify() -> Result<(), String> {
    let began = std::time::Instant::now();
    cargo(&["fmt", "--all", "--check"])?;
    cargo(&[
        "clippy",
        "--workspace",
        "--all-targets",
        "--locked",
        "--",
        "-D",
        "warnings",
    ])?;
    cargo(&["test", "--workspace", "--locked"])?;
    guards()?;
    leaks::check(began.elapsed())?;
    eprintln!("xtask verify: PASS");
    Ok(())
}

fn guards() -> Result<(), String> {
    let root = app::root()?;
    versions::check(&root)?;
    guards::run(&root)
}

fn web() -> Result<(), String> {
    let dir = app::root()?.join("apps/web");
    // npm is a .cmd script on Windows; Command does not add the extension itself.
    let npm = if cfg!(windows) { "npm.cmd" } else { "npm" };
    run(npm, &["ci", "--no-audit", "--no-fund"], &dir)?;
    // The TypeScript types must match the pinned API document (apps/web/openapi.json).
    run(npm, &["run", "gen:check"], &dir)?;
    run(npm, &["test"], &dir)?;
    web_build(&dir)?;
    eprintln!("xtask web: PASS");
    Ok(())
}

/// The production build of `apps/web` into `dist/` (the step `web` ends with).
fn web_build(dir: &std::path::Path) -> Result<(), String> {
    let npm = if cfg!(windows) { "npm.cmd" } else { "npm" };
    run(npm, &["run", "build"], dir)?;
    if !dir.join("dist/index.html").is_file() {
        return Err("apps/web/dist/index.html was not produced".to_owned());
    }
    Ok(())
}

fn e2e_web() -> Result<(), String> {
    let root = app::root()?;
    let web = root.join("apps/web");
    let specs = e2e_specs(&web.join("e2e"))?;
    let names: Vec<&str> = specs.iter().map(|(name, _)| name.as_str()).collect();
    eprintln!("xtask e2e-web: specs: {}", names.join(" "));
    // Before the cargo builds: the daemon embeds `dist` when it is compiled.
    ensure_web_bundle(&web)?;
    cargo(&["build", "--locked", "--bin", "tok"])?;
    cargo(&[
        "build",
        "--locked",
        "-p",
        "toknila-daemon",
        "--bin",
        "fake-agent",
    ])?;
    let target = std::env::var_os("CARGO_TARGET_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| root.join("target"));
    if std::env::var_os("CI").is_some() {
        let npx = if cfg!(windows) { "npx.cmd" } else { "npx" };
        let mut args = vec!["playwright", "install", "chromium"];
        if cfg!(target_os = "linux") {
            args.push("--with-deps");
        }
        run(npx, &args, &web)?;
    }
    let exe = |name: &str| {
        let file = format!("{name}{}", std::env::consts::EXE_SUFFIX);
        target
            .join("debug")
            .join(file)
            .to_string_lossy()
            .into_owned()
    };
    for (name, needs_agent) in &specs {
        let script = format!("e2e/{name}.mjs");
        let tok = exe("tok");
        if *needs_agent {
            run("node", &[&script, &tok, &exe("fake-agent")], &web)?;
        } else {
            run("node", &[&script, &tok], &web)?;
        }
    }
    eprintln!("xtask e2e-web: PASS");
    Ok(())
}

/// The specs in `dir` as (name without extension, needs the fake agent): `smoke` first, the rest
/// alphabetical. `helpers.mjs` is a shared module, not a spec. A spec declares the fake agent
/// with `// needs: fake-agent` within its first 15 lines.
fn e2e_specs(dir: &std::path::Path) -> Result<Vec<(String, bool)>, String> {
    let entries =
        std::fs::read_dir(dir).map_err(|e| format!("cannot read {}: {e}", dir.display()))?;
    let mut specs = Vec::new();
    for entry in entries {
        let path = entry
            .map_err(|e| format!("cannot read {}: {e}", dir.display()))?
            .path();
        if path.extension().and_then(|e| e.to_str()) != Some("mjs") {
            continue;
        }
        let Some(name) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if name == "helpers" {
            continue;
        }
        let text = std::fs::read_to_string(&path)
            .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        let needs_agent = text
            .lines()
            .take(15)
            .any(|line| line.trim() == "// needs: fake-agent");
        specs.push((name.to_owned(), needs_agent));
    }
    if specs.is_empty() {
        return Err(format!("no e2e spec (*.mjs) found in {}", dir.display()));
    }
    specs.sort_by(|a, b| (a.0 != "smoke", &a.0).cmp(&(b.0 != "smoke", &b.0)));
    Ok(specs)
}

/// Latest modification time of a file under `path` (a file or a folder walked recursively).
fn newest_mtime(path: &std::path::Path) -> Result<Option<std::time::SystemTime>, String> {
    let err = |e: std::io::Error| format!("cannot read {}: {e}", path.display());
    let meta = std::fs::metadata(path).map_err(err)?;
    if !meta.is_dir() {
        return Ok(Some(meta.modified().map_err(err)?));
    }
    let mut newest = None;
    for entry in std::fs::read_dir(path).map_err(err)? {
        let found = newest_mtime(&entry.map_err(err)?.path())?;
        newest = newest.max(found);
    }
    Ok(newest)
}

/// Builds the web bundle when `apps/web/dist` is missing or older than its sources.
fn ensure_web_bundle(web: &std::path::Path) -> Result<(), String> {
    let mut sources = vec![
        web.join("src"),
        web.join("index.html"),
        web.join("package.json"),
    ];
    if let Ok(entries) = std::fs::read_dir(web) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            if name.to_string_lossy().starts_with("vite.config.") {
                sources.push(entry.path());
            }
        }
    }
    let mut newest = None;
    for source in sources.iter().filter(|p| p.exists()) {
        newest = newest.max(newest_mtime(source)?);
    }
    let built = newest_mtime(&web.join("dist/index.html")).ok().flatten();
    if built.is_some() && built >= newest {
        eprintln!("xtask e2e-web: the web bundle is current");
        return Ok(());
    }
    eprintln!("xtask e2e-web: building the web bundle (dist is older than the sources)");
    web_build(web)
}

/// Archive name part for a target triple.
fn platform(target: &str) -> Option<&'static str> {
    Some(match target {
        "aarch64-apple-darwin" => "macos-arm64",
        "x86_64-apple-darwin" => "macos-x86_64",
        "x86_64-unknown-linux-gnu" => "linux-x86_64",
        "aarch64-unknown-linux-gnu" => "linux-arm64",
        "x86_64-pc-windows-msvc" => "windows-x86_64",
        _ => return None,
    })
}

fn dist(target: Option<String>) -> Result<(), String> {
    let target = target.ok_or("usage: cargo xtask dist <target triple>")?;
    let platform = platform(&target).ok_or_else(|| format!("unsupported target {target}"))?;
    let root = app::root()?;
    let version = output(&["pkgid", "-p", "toknila"])?
        .trim()
        .rsplit(['@', '#'])
        .next()
        .unwrap_or_default()
        .to_owned();
    web()?;
    cargo(&[
        "build",
        "--release",
        "--locked",
        "--target",
        &target,
        "--bin",
        "tok",
        "--bin",
        "toknila",
    ])?;
    let windows = target.contains("windows");
    let exe = if windows { ".exe" } else { "" };
    let name = format!("toknila-{version}-{platform}");
    let out = root.join("dist");
    let stage = out.join(&name);
    let _ = std::fs::remove_dir_all(&stage);
    std::fs::create_dir_all(&stage).map_err(|e| format!("{}: {e}", stage.display()))?;
    let bin = root.join("target").join(&target).join("release");
    let copy = |from: std::path::PathBuf, to: &str| {
        std::fs::copy(&from, stage.join(to))
            .map(|_| ())
            .map_err(|e| format!("copy {}: {e}", from.display()))
    };
    for b in ["tok", "toknila"] {
        copy(bin.join(format!("{b}{exe}")), &format!("{b}{exe}"))?;
    }
    copy(root.join("LICENSE"), "LICENSE")?;
    copy(root.join("README.md"), "README.md")?;
    copy(root.join("docs/install.md"), "INSTALL.md")?;
    copy(root.join("examples/demo.toml"), "demo.toml")?;
    copy(root.join("plans/phase-1.toml"), "phase-1.toml")?;
    // bsdtar (macOS, Windows) and GNU tar (Linux) both take these flags; -a picks zip by name.
    let archive = if windows {
        format!("{name}.zip")
    } else {
        format!("{name}.tar.gz")
    };
    let flags = if windows { "-a -cf" } else { "-czf" };
    let mut args: Vec<&str> = flags.split(' ').collect();
    args.extend([archive.as_str(), name.as_str()]);
    run("tar", &args, &out)?;
    eprintln!("xtask dist: PASS: dist/{archive}");
    Ok(())
}

fn desktop() -> Result<(), String> {
    let root = app::root()?;
    // The window shows the page the daemon embeds.
    web()?;
    cargo(&["build", "--locked", "--bin", "toknila", "--bin", "tok"])?;
    app::stage_sidecars(&root, "debug", None)?;
    let manifest = app::shell_dir(&root).join("Cargo.toml");
    let manifest = manifest.to_string_lossy();
    cargo(&["fmt", "--manifest-path", &manifest, "--check"])?;
    cargo(&[
        "clippy",
        "--locked",
        "--manifest-path",
        &manifest,
        "--",
        "-D",
        "warnings",
    ])?;
    cargo(&["build", "--locked", "--manifest-path", &manifest])?;
    cargo(&["test", "--locked", "--manifest-path", &manifest])?;
    eprintln!("xtask desktop: PASS");
    Ok(())
}

fn run(program: &str, args: &[&str], dir: &std::path::Path) -> Result<(), String> {
    eprintln!("$ {program} {}", args.join(" "));
    let status = Command::new(program)
        .args(args)
        .current_dir(dir)
        .status()
        .map_err(|e| format!("cannot run {program}: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "`{program} {}` exited with {status}",
            args.join(" ")
        ))
    }
}

fn cargo_bin() -> String {
    std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned())
}

fn cargo(args: &[&str]) -> Result<(), String> {
    eprintln!("$ cargo {}", args.join(" "));
    let status = Command::new(cargo_bin())
        .args(args)
        .status()
        .map_err(|e| format!("cannot run cargo: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("`cargo {}` exited with {status}", args.join(" ")))
    }
}

fn output(args: &[&str]) -> Result<String, String> {
    let out = Command::new(cargo_bin())
        .args(args)
        .output()
        .map_err(|e| format!("cannot run cargo: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "`cargo {}` exited with {}: {}",
            args.join(" "),
            out.status,
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    String::from_utf8(out.stdout).map_err(|e| format!("non-utf8 cargo output: {e}"))
}
