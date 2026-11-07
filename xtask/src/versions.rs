//! One version everywhere: the workspace, the web package, the desktop shell, the API document
//! and the lock files must all say the same thing, or `cargo xtask guards` fails.

use std::path::Path;

/// Every place that carries the product version: its label and the version found there.
pub(crate) fn found(root: &Path) -> Result<Vec<(String, String)>, String> {
    let read = |rel: &str| {
        std::fs::read_to_string(root.join(rel)).map_err(|e| format!("cannot read {rel}: {e}"))
    };
    let mut out = Vec::new();
    let mut add = |label: &str, v: Option<String>| {
        v.map(|v| out.push((label.to_owned(), v)))
            .ok_or_else(|| format!("no version found in {label}"))
    };
    add(
        "Cargo.toml [workspace.package]",
        after_line(&read("Cargo.toml")?, "[workspace.package]", "version = "),
    )?;
    add(
        "apps/desktop/src-tauri/Cargo.toml",
        after_line(
            &read("apps/desktop/src-tauri/Cargo.toml")?,
            "[package]",
            "version = ",
        ),
    )?;
    add(
        "apps/desktop/src-tauri/tauri.conf.json",
        json_version(&read("apps/desktop/src-tauri/tauri.conf.json")?, 0),
    )?;
    add(
        "apps/web/package.json",
        json_version(&read("apps/web/package.json")?, 0),
    )?;
    let lock = read("apps/web/package-lock.json")?;
    add("apps/web/package-lock.json", json_version(&lock, 0))?;
    add(
        "apps/web/package-lock.json packages[\"\"]",
        json_version(&lock, 1),
    )?;
    add(
        "apps/web/openapi.json",
        json_version(&read("apps/web/openapi.json")?, 0),
    )?;
    for lock in ["Cargo.lock", "apps/desktop/src-tauri/Cargo.lock"] {
        for (name, v) in own_packages(&read(lock)?) {
            add(&format!("{lock} {name}"), Some(v))?;
        }
    }
    Ok(out)
}

/// The value of the first `key` line after the `section` header line.
fn after_line(text: &str, section: &str, key: &str) -> Option<String> {
    let mut lines = text.lines().skip_while(|l| l.trim() != section);
    lines.next()?;
    lines
        .take_while(|l| !l.starts_with('['))
        .find_map(|l| l.strip_prefix(key))
        .map(|v| v.trim().trim_matches('"').to_owned())
}

/// The `n`th (from 0) line of the form `"version": "x"` in a JSON file.
fn json_version(text: &str, n: usize) -> Option<String> {
    text.lines()
        .filter_map(|l| l.trim().strip_prefix("\"version\": \""))
        .nth(n)
        .and_then(|v| v.split('"').next())
        .map(str::to_owned)
}

/// Name and version of the workspace's own packages in a `Cargo.lock`.
fn own_packages(lock: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut name = None::<String>;
    for line in lock.lines() {
        if let Some(n) = line.strip_prefix("name = ") {
            name = Some(n.trim_matches('"').to_owned());
        } else if let Some(v) = line.strip_prefix("version = ")
            && let Some(n) = name.take()
            && (n.starts_with("toknila") || n == "xtask")
        {
            out.push((n, v.trim_matches('"').to_owned()));
        }
    }
    out
}

/// The product version (the workspace's).
pub fn current(root: &Path) -> Result<String, String> {
    found(root)?
        .into_iter()
        .next()
        .map(|(_, v)| v)
        .ok_or_else(|| "no version found".to_owned())
}

/// Fails with one line per place that disagrees with the workspace version.
pub fn check(root: &Path) -> Result<(), String> {
    disagreements(&found(root)?)
}

fn disagreements(found: &[(String, String)]) -> Result<(), String> {
    let Some((_, want)) = found.first() else {
        return Err("no versions found".to_owned());
    };
    let bad: Vec<String> = found
        .iter()
        .filter(|(_, v)| v != want)
        .map(|(label, v)| format!("{label} says {v}, the workspace says {want}"))
        .collect();
    if bad.is_empty() {
        Ok(())
    } else {
        Err(format!("versions disagree:\n  {}", bad.join("\n  ")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_repository_agrees_with_itself() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let all = found(&root).expect("versions");
        // Control: the probe sees the places it is meant to see.
        assert!(all.len() >= 12, "{all:?}");
        assert!(all.iter().any(|(l, _)| l == "apps/web/openapi.json"));
        assert!(all.iter().any(|(l, _)| l.ends_with("toknila-desktop")));
        disagreements(&all).expect("one version");
    }

    #[test]
    fn a_stray_version_is_named() {
        let rows = vec![
            ("a".to_owned(), "0.2.0".to_owned()),
            ("b".to_owned(), "0.1.0".to_owned()),
        ];
        let e = disagreements(&rows).expect_err("must disagree");
        assert!(e.contains("b says 0.1.0, the workspace says 0.2.0"), "{e}");
    }

    #[test]
    fn parsers_read_what_they_should() {
        let toml = "[workspace]\nx = 1\n[workspace.package]\nversion = \"1.2.3\"\n";
        assert_eq!(
            after_line(toml, "[workspace.package]", "version = ").as_deref(),
            Some("1.2.3")
        );
        let json = "{\n  \"version\": \"1.0.0\",\n  \"p\": {\n    \"version\": \"1.0.1\"\n  }\n}";
        assert_eq!(json_version(json, 1).as_deref(), Some("1.0.1"));
        let lock = "[[package]]\nname = \"toknila-x\"\nversion = \"0.3.0\"\n[[package]]\nname = \"serde\"\nversion = \"1.0.0\"\n";
        assert_eq!(
            own_packages(lock),
            vec![("toknila-x".to_owned(), "0.3.0".to_owned())]
        );
    }
}
