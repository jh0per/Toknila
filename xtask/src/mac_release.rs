//! `cargo xtask mac-release <tag> [--no-upload]`: the Mac files of a release, made on the
//! maintainer's Mac by the release's Build step (the project's artifact command is
//! `cargo xtask mac-release {tag}`). For aarch64, then x86_64, it builds and assembles
//! `Toknila.app`, signs it with the Developer ID under the hardened runtime, makes and signs the
//! dmg, has Apple notarize it, staples the app and the dmg, and packs and signs the updater archive
//! from the stapled app (`mac_sign.rs`). Then it uploads the two dmg, the two archives and, last,
//! their `.sig` files to the release on GitHub, as a draft when the tag has none yet
//! (`mac_upload.rs`).
//! CI's Release workflow, with the repository variable `MAC_APP_SOURCE=release`, adds the rest and
//! publishes. The settings are names and paths on the maintainer's computer (`mac_settings.rs`);
//! `--no-upload` stops before GitHub, to try the certificate and the notary profile.

use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::app::{self, Build};
use crate::mac_settings::Settings;
use crate::mac_sign::{self, Signing};
use crate::mac_upload::{self, Release};
use crate::runner::{Cmd, Exec, Process, check};
use crate::{versions, web};

/// The two Mac targets, in the order they are built.
pub const TARGETS: [&str; 2] = ["aarch64-apple-darwin", "x86_64-apple-darwin"];

const USAGE: &str = "usage: cargo xtask mac-release <tag> [--no-upload]";

/// The command's arguments.
#[derive(Debug, PartialEq, Eq)]
pub struct Args {
    pub tag: String,
    pub upload: bool,
}

fn parse_args(args: impl Iterator<Item = String>) -> Result<Args, String> {
    let (mut tag, mut upload) = (None, true);
    for a in args {
        match a.as_str() {
            "--no-upload" => upload = false,
            t if t.starts_with('v') && tag.is_none() => tag = Some(t.to_owned()),
            other => return Err(format!("unexpected argument {other}; {USAGE}")),
        }
    }
    let tag = tag.ok_or(USAGE)?;
    Ok(Args { tag, upload })
}

/// What can be checked in seconds, before an hour of building: the tag is the version, the tools
/// are there, both Rust targets are installed, and (when it uploads) the tag is on GitHub and its
/// release is not already published.
pub fn preflight(exec: &mut dyn Exec, root: &Path, a: &Args, version: &str) -> Result<(), String> {
    if a.tag != format!("v{version}") {
        return Err(format!(
            "the tag {} is not v{version}, the version of this checkout; run it at the tagged commit",
            a.tag
        ));
    }
    let mut tools = vec![(
        "xcrun",
        "install the Xcode command line tools: xcode-select --install",
    )];
    if a.upload {
        tools.push((
            "gh",
            "install GitHub's command line tool (brew install gh) and log in: gh auth login",
        ));
    }
    for (tool, hint) in tools {
        if exec.run(&Cmd::new(tool, root).args(["--version"])).is_err() {
            return Err(format!("{tool} was not found: {hint}"));
        }
    }
    let rustup = Cmd::new("rustup", root).args(["target", "list", "--installed"]);
    let installed = check(exec, &rustup)
        .map_err(|e| format!("cannot list the installed Rust targets: {e}"))?
        .out;
    let missing: Vec<&str> = TARGETS
        .into_iter()
        .filter(|t| !installed.lines().any(|l| l.trim() == *t))
        .collect();
    if !missing.is_empty() {
        let s = if missing.len() == 1 { "" } else { "s" };
        return Err(format!(
            "the Rust target{s} {} not installed: run rustup target add {}",
            if s.is_empty() {
                format!("{} is", missing[0])
            } else {
                format!("{} are", missing.join(" and "))
            },
            missing.join(" ")
        ));
    }
    if a.upload {
        mac_upload::tag_on_github(exec, root, &a.tag)?;
        if mac_upload::state(exec, root, &a.tag)? == Release::Published {
            return Err(mac_upload::published(&a.tag));
        }
    }
    Ok(())
}

/// The whole release after the settings are read: preflight, then for each target `build` (which
/// assembles `<out>/Toknila.app`) and the signing pipeline, then the upload. Returns the files.
pub fn release(
    exec: &mut dyn Exec,
    root: &Path,
    a: &Args,
    version: &str,
    s: &Signing,
    build: &mut dyn FnMut(&str, &Path) -> Result<(), String>,
) -> Result<Vec<PathBuf>, String> {
    preflight(exec, root, a, version)?;
    mac_sign::check_notary(exec, root, s.profile)?;
    let base = root.join("dist/mac-release");
    let _ = std::fs::remove_dir_all(&base);
    let mut files = Vec::new();
    for triple in TARGETS {
        let arch = app::arch_of(triple);
        let out = base.join(arch);
        build(triple, &out)?;
        files.extend(mac_sign::one_target(exec, s, &out, version, arch)?);
    }
    if a.upload {
        mac_upload::upload(exec, root, &a.tag, &files)?;
    } else {
        exec.say("xtask mac-release: --no-upload, so nothing went to GitHub");
    }
    Ok(files)
}

/// `cargo xtask mac-release`.
pub fn mac_release() -> Result<(), String> {
    let a = parse_args(std::env::args().skip(2))?;
    if !cfg!(target_os = "macos") {
        return Err("mac-release runs on a Mac only".to_owned());
    }
    let started = Instant::now();
    let root = app::root()?;
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let settings = Settings::load(&|n| std::env::var(n).ok(), home.as_deref(), &root)?;
    let password = settings.password()?;
    let mut exec = Process::new(settings.redact(&password));
    let version = versions::current(&root)?;
    let entitlements = mac_sign::entitlements(&root);
    let signing = Signing {
        identity: &settings.identity,
        profile: &settings.profile,
        entitlements: &entitlements,
        key_path: &settings.key_path,
        password: &password,
    };
    match settings.target_dir {
        Some(_) => exec.say("xtask mac-release: building in the CARGO_TARGET_DIR of the settings"),
        None => exec
            .say("xtask mac-release: no CARGO_TARGET_DIR in the settings: this build starts cold"),
    }
    let mut web_built = false;
    let mut build = |triple: &str, out: &Path| -> Result<(), String> {
        if !web_built {
            web()?;
            web_built = true;
        }
        let b = Build {
            triple: Some(triple),
            target_dir: settings.target_dir.as_deref(),
        };
        app::assemble(&root, b, &version, out).map(|_| ())
    };
    let files = release(&mut exec, &root, &a, &version, &signing, &mut build)?;
    let minutes = started.elapsed().as_secs() / 60;
    exec.say(&format!("xtask mac-release: PASS in {minutes} min"));
    for f in &files {
        exec.say(&format!("  {}", f.display()));
    }
    Ok(())
}

#[cfg(test)]
#[path = "mac_release_tests.rs"]
mod tests;
