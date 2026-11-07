//! `cargo xtask app`: the macOS bundle (`Toknila.app` and a `.dmg`) for the host architecture,
//! ad-hoc signed so it runs on the Mac that built it. No Developer ID, no notarisation: handing it
//! to other people is a later job. The desktop shell, its sidecars and the staging are shared
//! with `cargo xtask desktop`. `--target <triple>` builds for another macOS architecture; the
//! same step packs and signs the archive the in-app updater installs (see `updates.rs`).

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::{cargo, run, updates, versions, web};

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

/// The cargo target directory of the main workspace.
fn target_dir(root: &Path) -> PathBuf {
    std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
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
fn arch_of(triple: &str) -> &str {
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
/// were cross-built for; `None` means this machine's.
pub fn stage_sidecars(root: &Path, profile: &str, target: Option<&str>) -> Result<(), String> {
    let host = match target {
        Some(t) => t.to_owned(),
        None => host_triple()?,
    };
    let exe = std::env::consts::EXE_SUFFIX;
    let binaries = shell_dir(root).join("binaries");
    std::fs::create_dir_all(&binaries).map_err(|e| format!("binaries/: {e}"))?;
    for name in ["toknila", "tok"] {
        let from = built_dir(&target_dir(root), target, profile).join(format!("{name}{exe}"));
        let to = binaries.join(format!("{name}-{host}{exe}"));
        std::fs::copy(&from, &to).map_err(|e| format!("copy {}: {e}", from.display()))?;
    }
    Ok(())
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
    let mut build = vec!["build", "--release", "--locked"];
    build.extend(triple.iter().flat_map(|t| ["--target", t]));
    build.extend(["--bin", "toknila", "--bin", "tok"]);
    cargo(&build)?;
    stage_sidecars(&root, "release", triple)?;
    let shell = shell_dir(&root);
    let manifest = shell.join("Cargo.toml");
    let manifest = manifest.to_string_lossy();
    // `custom-protocol` makes the shell serve its own start page instead of a dev server.
    let mut build = vec!["build", "--release", "--locked"];
    build.extend(triple.iter().flat_map(|t| ["--target", t]));
    build.extend([
        "--manifest-path",
        &manifest,
        "--features",
        "custom-protocol",
    ]);
    cargo(&build)?;
    let shell_target = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| shell.join("target"));

    let out = root.join("dist/app");
    let _ = std::fs::remove_dir_all(&out);
    let bundle = out.join("Toknila.app");
    let macos = bundle.join("Contents/MacOS");
    let resources = bundle.join("Contents/Resources");
    for d in [&macos, &resources] {
        std::fs::create_dir_all(d).map_err(|e| format!("{}: {e}", d.display()))?;
    }
    let target = built_dir(&target_dir(&root), triple, "release");
    let copies = [
        (
            built_dir(&shell_target, triple, "release").join("toknila-desktop"),
            "toknila-desktop",
        ),
        (target.join("toknila"), "toknila"),
        (target.join("tok"), "tok"),
    ];
    for (from, name) in &copies {
        std::fs::copy(from, macos.join(name))
            .map_err(|e| format!("copy {}: {e}", from.display()))?;
    }
    std::fs::write(bundle.join("Contents/Info.plist"), info_plist(&version))
        .map_err(|e| format!("Info.plist: {e}"))?;
    icon(
        &shell.join("icons/icon.png"),
        &resources.join("icon.icns"),
        &out,
    )?;

    // Ad-hoc ("-"): runs here, carries no identity. Nested code first, then the bundle.
    let path = |p: &Path| p.to_string_lossy().into_owned();
    for name in ["toknila", "tok", "toknila-desktop"] {
        let bin = path(&macos.join(name));
        run("codesign", &["--force", "--sign", "-", &bin], &out)?;
    }
    let app_path = path(&bundle);
    run("codesign", &["--force", "--sign", "-", &app_path], &out)?;
    run("codesign", &["--verify", "--strict", &app_path], &out)?;

    let stage = out.join("dmg");
    std::fs::create_dir_all(&stage).map_err(|e| format!("{}: {e}", stage.display()))?;
    run(
        "ditto",
        &[&app_path, &path(&stage.join("Toknila.app"))],
        &out,
    )?;
    link_applications(&stage)?;
    let dmg = out.join(format!("Toknila-{version}-{arch}.dmg"));
    run(
        "hdiutil",
        &[
            "create",
            "-volname",
            "Toknila",
            "-srcfolder",
            &path(&stage),
            "-ov",
            "-format",
            "UDZO",
            &path(&dmg),
        ],
        &out,
    )?;
    let _ = std::fs::remove_dir_all(&stage);
    let archive = updates::package(&out, &version, &arch)?;
    eprintln!("xtask app: PASS");
    eprintln!("  app: {}", bundle.display());
    eprintln!("  dmg: {}", dmg.display());
    eprintln!("  update archive: {}", archive.display());
    Ok(())
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
/// next to it in `Contents/MacOS`.
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
  <key>NSHighResolutionCapable</key><true/>
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
    }

    #[test]
    fn plist_carries_the_version_and_the_shell_executable() {
        let p = info_plist("9.8.7");
        assert!(p.contains("<key>CFBundleShortVersionString</key><string>9.8.7</string>"));
        assert!(p.contains("<key>CFBundleExecutable</key><string>toknila-desktop</string>"));
    }
}
