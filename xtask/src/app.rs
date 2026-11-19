//! `cargo xtask app`: the macOS bundle (`Toknila.app` and a `.dmg`) for the host architecture, for
//! developers: signed ad hoc so it runs on the Mac that built it (or with the identity
//! `APPLE_SIGNING_IDENTITY` names, which CI's `ci` mode sets), not notarized. The release's signed
//! and notarized app is `cargo xtask mac-release` (`mac_release.rs`); it builds the bundle with
//! [`assemble`] and the dmg with [`make_dmg`] from here. The desktop shell, its sidecars and the
//! staging are shared with `cargo xtask desktop`. `--target <triple>` builds for another macOS
//! architecture; the same step packs and signs the archive the in-app updater installs (see
//! `updates.rs`).

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::mac_settings::IDENTITY;
use crate::runner::{Cmd, Exec, Process, Redact, check};
use crate::{run, updates, versions, web};

/// The repository root, resolved. The manifest folder is read from the environment cargo sets
/// when it runs the task: the compiled-in path can belong to another checkout when several share
/// one target directory (cargo reuses a build of identical sources), which would make a task
/// edit the wrong tree.
pub fn root() -> Result<PathBuf, String> {
    let manifest = std::env::var_os("CARGO_MANIFEST_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")));
    manifest
        .join("..")
        .canonicalize()
        .map_err(|e| format!("repo root: {e}"))
}

/// The cargo target directory of the main workspace: `over` when given, else `CARGO_TARGET_DIR`,
/// else `target` in the repository.
fn target_dir(root: &Path, over: Option<&Path>) -> PathBuf {
    over.map(Path::to_path_buf)
        .or_else(|| std::env::var_os("CARGO_TARGET_DIR").map(PathBuf::from))
        .unwrap_or_else(|| root.join("target"))
}

/// The folder of the desktop shell (its own Cargo workspace).
pub fn shell_dir(root: &Path) -> PathBuf {
    root.join("apps/desktop/src-tauri")
}

/// Where cargo puts binaries: `<target>/<profile>`, or `<target>/<triple>/<profile>` when
/// cross-building.
fn built_dir(target_dir: &Path, triple: Option<&str>, profile: &str) -> PathBuf {
    match triple {
        Some(t) => target_dir.join(t).join(profile),
        None => target_dir.join(profile),
    }
}

/// The architecture name of a target triple (`aarch64-apple-darwin` is `aarch64`).
pub fn arch_of(triple: &str) -> &str {
    triple.split('-').next().unwrap_or(triple)
}

fn host_triple() -> Result<String, String> {
    let out = Command::new(std::env::var("RUSTC").unwrap_or_else(|_| "rustc".to_owned()))
        .arg("-vV")
        .output()
        .map_err(|e| format!("cannot run rustc: {e}"))?;
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .find_map(|l| l.strip_prefix("host: ").map(str::to_owned))
        .ok_or_else(|| "rustc -vV printed no host triple".to_owned())
}

/// Copies `toknila` and `tok` built under `profile` ("debug" or "release") to where Tauri looks
/// for sidecars: `binaries/<name>-<triple>` in the shell folder. `target` is the triple they
/// were cross-built for; `None` means this machine's. `built` is the build folder, if not the default.
pub fn stage_sidecars(
    root: &Path,
    profile: &str,
    target: Option<&str>,
    built: Option<&Path>,
) -> Result<(), String> {
    let host = match target {
        Some(t) => t.to_owned(),
        None => host_triple()?,
    };
    let exe = std::env::consts::EXE_SUFFIX;
    let binaries = shell_dir(root).join("binaries");
    std::fs::create_dir_all(&binaries).map_err(|e| format!("binaries/: {e}"))?;
    for name in ["toknila", "tok"] {
        let from =
            built_dir(&target_dir(root, built), target, profile).join(format!("{name}{exe}"));
        let to = binaries.join(format!("{name}-{host}{exe}"));
        std::fs::copy(&from, &to).map_err(|e| format!("copy {}: {e}", from.display()))?;
    }
    Ok(())
}

/// Runs cargo, with `CARGO_TARGET_DIR` set to `target_dir` when one is given.
fn cargo_in(args: &[&str], target_dir: Option<&Path>) -> Result<(), String> {
    eprintln!("$ cargo {}", args.join(" "));
    let mut cmd = Command::new(crate::cargo_bin());
    cmd.args(args);
    if let Some(t) = target_dir {
        cmd.env("CARGO_TARGET_DIR", t);
    }
    let status = cmd.status().map_err(|e| format!("cannot run cargo: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("`cargo {}` exited with {status}", args.join(" ")))
    }
}

/// One bundle's build: the target triple (`None`: this Mac's) and the build folder (`None`: the
/// default one).
#[derive(Debug, Clone, Copy, Default)]
pub struct Build<'a> {
    pub triple: Option<&'a str>,
    pub target_dir: Option<&'a Path>,
}

/// Builds `toknila`, `tok` and the shell for one target and assembles `<out>/Toknila.app`, not yet
/// signed; `out` is emptied first. The web page must be built before (`cargo xtask web`).
pub fn assemble(root: &Path, b: Build, version: &str, out: &Path) -> Result<PathBuf, String> {
    let mut build = vec!["build", "--release", "--locked"];
    build.extend(b.triple.iter().flat_map(|t| ["--target", t]));
    build.extend(["--bin", "toknila", "--bin", "tok"]);
    cargo_in(&build, b.target_dir)?;
    stage_sidecars(root, "release", b.triple, b.target_dir)?;
    let shell = shell_dir(root);
    let manifest = shell.join("Cargo.toml");
    let manifest = manifest.to_string_lossy();
    // `custom-protocol` makes the shell serve its own start page instead of a dev server.
    let mut build = vec!["build", "--release", "--locked"];
    build.extend(b.triple.iter().flat_map(|t| ["--target", t]));
    build.extend([
        "--manifest-path",
        &manifest,
        "--features",
        "custom-protocol",
    ]);
    cargo_in(&build, b.target_dir)?;
    let shell_target = b
        .target_dir
        .map(Path::to_path_buf)
        .or_else(|| std::env::var_os("CARGO_TARGET_DIR").map(PathBuf::from))
        .unwrap_or_else(|| shell.join("target"));

    let _ = std::fs::remove_dir_all(out);
    let bundle = out.join("Toknila.app");
    let macos = bundle.join("Contents/MacOS");
    let resources = bundle.join("Contents/Resources");
    for d in [&macos, &resources] {
        std::fs::create_dir_all(d).map_err(|e| format!("{}: {e}", d.display()))?;
    }
    let target = built_dir(&target_dir(root, b.target_dir), b.triple, "release");
    let copies = [
        (
            built_dir(&shell_target, b.triple, "release").join("toknila-desktop"),
            "toknila-desktop",
        ),
        (target.join("toknila"), "toknila"),
        (target.join("tok"), "tok"),
    ];
    for (from, name) in &copies {
        std::fs::copy(from, macos.join(name))
            .map_err(|e| format!("copy {}: {e}", from.display()))?;
    }
    std::fs::write(bundle.join("Contents/Info.plist"), info_plist(version))
        .map_err(|e| format!("Info.plist: {e}"))?;
    icon(
        &shell.join("icons/icon.png"),
        &resources.join("icon.icns"),
        out,
    )?;
    Ok(bundle)
}

/// Builds the bundle and prints where the `.app` and the `.dmg` are.
pub fn app() -> Result<(), String> {
    if !cfg!(target_os = "macos") {
        return Err("the app bundle is macOS only for now".to_owned());
    }
    let triple = target_arg(std::env::args().skip(2))?;
    let triple = triple.as_deref();
    let arch = match triple {
        Some(t) => arch_of(t).to_owned(),
        None => arch_of(&host_triple()?).to_owned(),
    };
    let root = root()?;
    let version = versions::current(&root)?;
    // The window shows the page the daemon embeds, so the web build comes first.
    web()?;
    let out = root.join("dist/app");
    let build = Build {
        triple,
        target_dir: None,
    };
    let bundle = assemble(&root, build, &version, &out)?;

    // One identity for every build when `APPLE_SIGNING_IDENTITY` names one (macOS keeps the
    // privacy permissions it granted under it across updates); ad hoc ("-") otherwise: runs here,
    // carries no identity. Nested code first, then the bundle. Its value is never printed.
    let identity = std::env::var(IDENTITY)
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "-".to_owned());
    let mut redact = Redact::default();
    match identity.as_str() {
        "-" => println!("xtask app: signing ad hoc ({IDENTITY} not set)"),
        id => {
            redact.add(IDENTITY, id);
            println!("xtask app: signing with the identity {IDENTITY} names");
        }
    }
    let mut exec = Process::new(redact);
    let macos = bundle.join("Contents/MacOS");
    for what in [
        macos.join("toknila"),
        macos.join("tok"),
        macos.join("toknila-desktop"),
        bundle.clone(),
    ] {
        let sign = Cmd::new("codesign", &out)
            .args(["--force", "--sign", &identity])
            .path(&what);
        check(&mut exec, &sign)?;
    }
    let verify = Cmd::new("codesign", &out)
        .args(["--verify", "--strict"])
        .path(&bundle);
    check(&mut exec, &verify)?;
    let dmg = make_dmg(&mut exec, &out, &version, &arch)?;
    let archive = updates::package(&out, &version, &arch)?;
    eprintln!("xtask app: PASS");
    eprintln!("  app: {}", bundle.display());
    eprintln!("  dmg: {}", dmg.display());
    eprintln!("  update archive: {}", archive.display());
    Ok(())
}

/// Makes `<out>/Toknila-<version>-<arch>.dmg` from `<out>/Toknila.app` with a link to
/// Applications beside it, and returns its path.
pub fn make_dmg(
    exec: &mut dyn Exec,
    out: &Path,
    version: &str,
    arch: &str,
) -> Result<PathBuf, String> {
    let stage = out.join("dmg");
    let _ = std::fs::remove_dir_all(&stage);
    std::fs::create_dir_all(&stage).map_err(|e| format!("{}: {e}", stage.display()))?;
    let ditto = Cmd::new("ditto", out)
        .path(&out.join("Toknila.app"))
        .path(&stage.join("Toknila.app"));
    check(exec, &ditto)?;
    link_applications(&stage)?;
    let dmg = out.join(format!("Toknila-{version}-{arch}.dmg"));
    let create = Cmd::new("hdiutil", out)
        .args(["create", "-volname", "Toknila", "-srcfolder"])
        .path(&stage)
        .args(["-ov", "-format", "UDZO"])
        .path(&dmg);
    // `hdiutil create` fails now and then on a busy runner ("Resource busy"): try again.
    retry(DMG_ATTEMPTS, DMG_PAUSE, "dmg", || {
        check(exec, &create).map(|_| ())
    })?;
    let _ = std::fs::remove_dir_all(&stage);
    Ok(dmg)
}

/// How many times the dmg step is tried, and the pause between tries.
const DMG_ATTEMPTS: u32 = 3;
const DMG_PAUSE: std::time::Duration = std::time::Duration::from_secs(5);

/// Runs `step` up to `attempts` times, sleeping `pause` between failures, and says in the log
/// which attempt succeeded. Returns the last error when every attempt fails.
fn retry(
    attempts: u32,
    pause: std::time::Duration,
    name: &str,
    mut step: impl FnMut() -> Result<(), String>,
) -> Result<(), String> {
    let mut last = String::new();
    for attempt in 1..=attempts {
        match step() {
            Ok(()) => {
                eprintln!("xtask app: {name} succeeded on attempt {attempt} of {attempts}");
                return Ok(());
            }
            Err(e) => {
                eprintln!("xtask app: {name} attempt {attempt} of {attempts} failed: {e}");
                last = e;
                if attempt < attempts {
                    std::thread::sleep(pause);
                }
            }
        }
    }
    Err(format!("{name} failed after {attempts} attempts: {last}"))
}

/// The value of `--target <triple>` among the arguments after the task name, if given.
fn target_arg(mut args: impl Iterator<Item = String>) -> Result<Option<String>, String> {
    match args.next().as_deref() {
        None => Ok(None),
        Some("--target") => args
            .next()
            .map(Some)
            .ok_or_else(|| "--target needs a triple".to_owned()),
        Some(other) => Err(format!(
            "unknown argument {other}; usage: cargo xtask app [--target <triple>]"
        )),
    }
}

#[cfg(unix)]
fn link_applications(stage: &Path) -> Result<(), String> {
    std::os::unix::fs::symlink("/Applications", stage.join("Applications"))
        .map_err(|e| format!("Applications link: {e}"))
}

#[cfg(not(unix))]
fn link_applications(_: &Path) -> Result<(), String> {
    Err("the app bundle is macOS only for now".to_owned())
}

/// `Info.plist` of the bundle: the shell is the main executable, the daemon and the CLI sit
/// next to it in `Contents/MacOS`. The two usage descriptions are what the microphone button
/// needs under the hardened runtime too (with the audio-input entitlement in `entitlements.plist`).
fn info_plist(version: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>Toknila</string>
  <key>CFBundleDisplayName</key><string>Toknila</string>
  <key>CFBundleIdentifier</key><string>app.toknila.desktop</string>
  <key>CFBundleExecutable</key><string>toknila-desktop</string>
  <key>CFBundleIconFile</key><string>icon</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>{version}</string>
  <key>CFBundleVersion</key><string>{version}</string>
  <key>LSMinimumSystemVersion</key><string>10.15</string>
  <key>NSHumanReadableCopyright</key><string>Copyright 2026 Mykyta Kravtsov</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSMicrophoneUsageDescription</key><string>Toknila listens when you press the microphone button, so you can speak to the assistant instead of typing.</string>
  <key>NSSpeechRecognitionUsageDescription</key><string>Toknila turns what you say into text with the system's speech recognition, only while you press the microphone button.</string>
</dict>
</plist>
"#
    )
}

/// Makes an `.icns` from a square PNG with the tools macOS ships.
fn icon(png: &Path, icns: &Path, work: &Path) -> Result<(), String> {
    let set = work.join("icon.iconset");
    std::fs::create_dir_all(&set).map_err(|e| format!("{}: {e}", set.display()))?;
    let (png, set_s) = (png.to_string_lossy(), set.to_string_lossy());
    for (px, name) in [
        (16, "icon_16x16"),
        (32, "icon_16x16@2x"),
        (32, "icon_32x32"),
        (64, "icon_32x32@2x"),
        (128, "icon_128x128"),
        (256, "icon_128x128@2x"),
        (256, "icon_256x256"),
        (512, "icon_256x256@2x"),
        (512, "icon_512x512"),
    ] {
        let to = set
            .join(format!("{name}.png"))
            .to_string_lossy()
            .into_owned();
        run(
            "sips",
            &["-z", &px.to_string(), &px.to_string(), &png, "--out", &to],
            work,
        )?;
    }
    run(
        "iconutil",
        &["-c", "icns", &set_s, "-o", &icns.to_string_lossy()],
        work,
    )?;
    let _ = std::fs::remove_dir_all(&set);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_target_argument_is_read() {
        let args = |v: &[&str]| {
            v.iter()
                .map(|s| (*s).to_owned())
                .collect::<Vec<_>>()
                .into_iter()
        };
        assert_eq!(target_arg(args(&[])), Ok(None));
        assert_eq!(
            target_arg(args(&["--target", "x86_64-apple-darwin"])),
            Ok(Some("x86_64-apple-darwin".to_owned()))
        );
        assert!(target_arg(args(&["--target"])).is_err());
        assert!(target_arg(args(&["--bogus"])).is_err());
        assert_eq!(arch_of("aarch64-apple-darwin"), "aarch64");
        assert_eq!(
            built_dir(Path::new("t"), Some("x86_64-apple-darwin"), "release"),
            Path::new("t/x86_64-apple-darwin/release")
        );
        assert_eq!(
            target_dir(Path::new("repo"), Some(Path::new("/cache"))),
            Path::new("/cache")
        );
    }

    #[test]
    fn a_busy_first_attempt_is_retried_until_it_succeeds() {
        let mut calls = 0;
        let r = retry(3, std::time::Duration::ZERO, "dmg", || {
            calls += 1;
            if calls == 1 {
                Err("hdiutil: create failed - Resource busy".to_owned())
            } else {
                Ok(())
            }
        });
        assert_eq!(r, Ok(()));
        assert_eq!(calls, 2);
    }

    #[test]
    fn the_retry_gives_up_after_the_last_attempt() {
        let mut calls = 0;
        let r = retry(3, std::time::Duration::ZERO, "dmg", || {
            calls += 1;
            Err("busy".to_owned())
        });
        assert_eq!(calls, 3);
        assert_eq!(r, Err("dmg failed after 3 attempts: busy".to_owned()));
    }

    #[test]
    fn plist_carries_the_version_and_the_shell_executable() {
        let p = info_plist("9.8.7");
        assert!(p.contains("<key>CFBundleShortVersionString</key><string>9.8.7</string>"));
        assert!(p.contains("<key>CFBundleExecutable</key><string>toknila-desktop</string>"));
    }

    #[test]
    fn plist_carries_the_copyright_line() {
        let p = info_plist("1.0.0");
        assert!(p.contains(
            "<key>NSHumanReadableCopyright</key><string>Copyright 2026 Mykyta Kravtsov</string>"
        ));
    }

    #[test]
    fn plist_says_why_it_listens() {
        let p = info_plist("1.0.0");
        assert!(p.contains("<key>NSMicrophoneUsageDescription</key><string>Toknila listens"));
        assert!(p.contains("<key>NSSpeechRecognitionUsageDescription</key><string>Toknila turns"));
    }

    /// Notarization needs ASCII XML entitlements without `get-task-allow`; the hardened runtime
    /// needs `audio-input` for the microphone. Nothing else is granted (see docs/install.md).
    #[test]
    fn the_entitlements_grant_audio_input_and_nothing_else() {
        let root = root().expect("repo root");
        let text = std::fs::read_to_string(crate::mac_sign::entitlements(&root)).expect("read");
        assert!(text.is_ascii());
        let keys: Vec<&str> = text
            .split("<key>")
            .skip(1)
            .filter_map(|k| k.split("</key>").next())
            .collect();
        assert_eq!(keys, ["com.apple.security.device.audio-input"]);
        let after = text.split("</key>").nth(1).expect("a value");
        assert!(after.trim_start().starts_with("<true/>"), "{after}");
        assert!(!text.contains("get-task-allow") && !text.contains("<!--"));
    }
}
