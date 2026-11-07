//! `cargo xtask bump X.Y.Z`: sets the product version in every place `versions.rs` checks,
//! regenerates what depends on it (both lock files, the pinned API document) and prints what to
//! do next to cut the release.

use std::path::Path;

use crate::{cargo, versions};

/// `X.Y.Z` with numeric parts and nothing else.
pub fn valid(version: &str) -> bool {
    let parts: Vec<&str> = version.split('.').collect();
    parts.len() == 3
        && parts.iter().all(|p| {
            !p.is_empty()
                && p.bytes().all(|b| b.is_ascii_digit())
                && (p.len() == 1 || !p.starts_with('0'))
        })
}

/// Replaces the value of the first `key` line after the `section` header line.
fn set_toml(text: &str, section: &str, key: &str, new: &str) -> Option<String> {
    let mut in_section = false;
    let mut done = false;
    let mut out = Vec::new();
    for line in text.lines() {
        if !done && in_section && line.starts_with(key) {
            out.push(format!("{key}\"{new}\""));
            done = true;
            continue;
        }
        if line.starts_with('[') {
            in_section = line.trim() == section;
        }
        out.push(line.to_owned());
    }
    done.then(|| join(&out, text))
}

/// Replaces the `n`th (from 0) line of the form `"version": "x"` of a JSON file.
fn set_json(text: &str, n: usize, new: &str) -> Option<String> {
    let mut seen = 0;
    let mut done = false;
    let mut out = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim_start();
        if !done && trimmed.starts_with("\"version\": \"") {
            if seen == n {
                let indent = &line[..line.len() - trimmed.len()];
                let comma = if trimmed.trim_end().ends_with(',') {
                    ","
                } else {
                    ""
                };
                out.push(format!("{indent}\"version\": \"{new}\"{comma}"));
                done = true;
                continue;
            }
            seen += 1;
        }
        out.push(line.to_owned());
    }
    done.then(|| join(&out, text))
}

fn join(lines: &[String], original: &str) -> String {
    let mut s = lines.join("\n");
    if original.ends_with('\n') {
        s.push('\n');
    }
    s
}

fn edit(root: &Path, rel: &str, f: impl Fn(&str) -> Option<String>) -> Result<(), String> {
    let path = root.join(rel);
    let text = std::fs::read_to_string(&path).map_err(|e| format!("cannot read {rel}: {e}"))?;
    let new = f(&text).ok_or_else(|| format!("no version line to change in {rel}"))?;
    std::fs::write(&path, new).map_err(|e| format!("cannot write {rel}: {e}"))
}

/// Runs the bump.
pub fn bump(version: Option<String>) -> Result<(), String> {
    let version = version.ok_or("usage: cargo xtask bump X.Y.Z")?;
    if !valid(&version) {
        return Err(format!("`{version}` is not a version like 0.2.1"));
    }
    let root = crate::app::root()?;
    let old = versions::current(&root)?;
    set_everywhere(&root, &version)?;
    // The lock files carry the workspace's own packages; the API document carries the daemon's.
    cargo(&["update", "--workspace", "--offline"])?;
    let shell = crate::app::shell_dir(&root).join("Cargo.toml");
    cargo(&[
        "update",
        "--workspace",
        "--offline",
        "--manifest-path",
        &shell.to_string_lossy(),
    ])?;
    // The daemon's own test rewrites its pinned API document when asked to.
    let status =
        std::process::Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
            .args(["test", "--locked", "-p", "toknila-daemon", "openapi"])
            .env("TOKNILA_UPDATE_SNAPSHOTS", "1")
            .current_dir(&root)
            .status()
            .map_err(|e| format!("cannot run cargo: {e}"))?;
    if !status.success() {
        return Err("regenerating apps/web/openapi.json failed".to_owned());
    }
    versions::check(&root)?;
    eprintln!("xtask bump: PASS ({old} -> {version})");
    eprintln!("next:");
    eprintln!("  git switch -c release-{version} && git commit -am \"Release {version}\"");
    eprintln!("  open a pull request and merge it once CI is green");
    eprintln!("  then, on main: git tag v{version} && git push origin v{version}");
    Ok(())
}

fn set_everywhere(root: &Path, v: &str) -> Result<(), String> {
    edit(root, "Cargo.toml", |t| {
        set_toml(t, "[workspace.package]", "version = ", v)
    })?;
    edit(root, "apps/desktop/src-tauri/Cargo.toml", |t| {
        set_toml(t, "[package]", "version = ", v)
    })?;
    edit(root, "apps/desktop/src-tauri/tauri.conf.json", |t| {
        set_json(t, 0, v)
    })?;
    edit(root, "apps/web/package.json", |t| set_json(t, 0, v))?;
    edit(root, "apps/web/package-lock.json", |t| {
        set_json(t, 0, v).and_then(|t| set_json(&t, 1, v))
    })?;
    edit(root, "apps/web/openapi.json", |t| set_json(t, 0, v))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_are_three_plain_numbers() {
        for ok in ["0.2.1", "10.0.12", "1.2.3"] {
            assert!(valid(ok), "{ok}");
        }
        for bad in [
            "",
            "1.2",
            "1.2.3.4",
            "v1.2.3",
            "1.2.x",
            "01.2.3",
            "1..3",
            "1.2.3-rc1",
        ] {
            assert!(!valid(bad), "{bad}");
        }
    }

    #[test]
    fn toml_version_changes_in_its_section_only() {
        let t = "[package]\nname = \"x\"\nversion = \"0.2.0\"\n[dependencies]\nversion = \"9\"\n";
        let n = set_toml(t, "[package]", "version = ", "0.2.1").expect("changed");
        assert_eq!(
            n,
            "[package]\nname = \"x\"\nversion = \"0.2.1\"\n[dependencies]\nversion = \"9\"\n"
        );
        assert_eq!(
            set_toml("[a]\nx = 1\n", "[package]", "version = ", "1.0.0"),
            None
        );
    }

    #[test]
    fn json_version_changes_the_chosen_occurrence() {
        let t = "{\n  \"version\": \"0.2.0\",\n  \"packages\": {\n    \"\": {\n      \"version\": \"0.2.0\",\n      \"x\": 1\n    }\n  }\n}\n";
        let first = set_json(t, 0, "0.2.1").expect("changed");
        assert!(
            first.contains("  \"version\": \"0.2.1\",\n  \"packages\""),
            "{first}"
        );
        assert!(first.contains("      \"version\": \"0.2.0\","), "{first}");
        let second = set_json(&first, 1, "0.2.1").expect("changed");
        assert!(second.contains("      \"version\": \"0.2.1\","), "{second}");
        assert!(second.ends_with("}\n"));
        assert_eq!(set_json(t, 5, "1.0.0"), None);
    }
}
