//! The files the in-app updater reads: `Toknila-<version>-<arch>.app.tar.gz`, its `.sig` and
//! `latest.json`, plus the release notes taken from merged pull request titles.
//!
//! Signing uses the official Tauri signer (`tauri signer sign`), run through a pinned version of
//! `@tauri-apps/cli`, not code of our own: the minisign envelope Tauri verifies is its format, and
//! one maintained implementation cannot drift from the verifier in the app. The cost is that Node
//! is needed, which `cargo xtask web` needs anyway. Without a key the step is skipped and says so.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::run;

/// The Tauri CLI that signs. Pinned, so a release never depends on whatever is newest.
const TAURI_CLI: &str = "@tauri-apps/cli@2.12.4";

/// The environment variables the signer reads (names fixed by Tauri).
const KEY_ENV: &str = "TAURI_SIGNING_PRIVATE_KEY";
const KEY_PATH_ENV: &str = "TAURI_SIGNING_PRIVATE_KEY_PATH";

/// Where the release files are published, for the `url` fields of `latest.json`.
pub fn default_base_url(version: &str) -> String {
    format!("https://github.com/jh0per/Toknila/releases/download/v{version}")
}

/// The updater's platform key for an architecture name (`aarch64`, `x86_64`).
pub fn platform_key(arch: &str) -> Option<&'static str> {
    match arch {
        "aarch64" => Some("darwin-aarch64"),
        "x86_64" => Some("darwin-x86_64"),
        _ => None,
    }
}

/// One platform's entry of `latest.json`.
pub struct Platform {
    /// `darwin-aarch64` and the like.
    pub key: String,
    /// The archive's file name, relative to the base URL.
    pub file: String,
    /// The content of the `.sig` file.
    pub signature: String,
}

fn json_string(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// The `latest.json` document the updater plugin reads (static JSON format).
pub fn latest_json(
    version: &str,
    notes: &str,
    pub_date: &str,
    base_url: &str,
    platforms: &[Platform],
) -> String {
    let base = base_url.trim_end_matches('/');
    let entries: Vec<String> = platforms
        .iter()
        .map(|p| {
            format!(
                "    {}: {{\n      \"signature\": {},\n      \"url\": {}\n    }}",
                json_string(&p.key),
                json_string(p.signature.trim()),
                json_string(&format!("{base}/{}", p.file)),
            )
        })
        .collect();
    format!(
        "{{\n  \"version\": {},\n  \"notes\": {},\n  \"pub_date\": {},\n  \"platforms\": {{\n{}\n  }}\n}}\n",
        json_string(version),
        json_string(notes),
        json_string(pub_date),
        entries.join(",\n")
    )
}

/// An RFC 3339 UTC time from Unix seconds.
pub fn rfc3339(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

fn now_rfc3339() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default();
    rfc3339(secs)
}

/// The merged pull request titles in the output of
/// `git log --first-parent --format=%s%x1f%b%x1e`: the first body line of a GitHub merge commit,
/// or the subject of a commit made directly or by squashing.
pub fn notes_from_log(log: &str) -> Vec<String> {
    log.split('\x1e')
        .filter_map(|record| {
            let (subject, body) = record.trim().split_once('\x1f')?;
            let subject = subject.trim();
            if subject.starts_with("Merge pull request") {
                body.lines().map(str::trim).find(|l| !l.is_empty())
            } else if subject.is_empty() || subject.starts_with("Merge ") {
                None
            } else {
                Some(subject)
            }
        })
        .map(str::to_owned)
        .collect()
}

/// `cargo xtask release-notes <tag>`: a Markdown list of what was merged since the tag before it.
pub fn release_notes(tag: &str) -> Result<String, String> {
    let git = |args: &[&str]| -> Result<String, String> {
        let out = Command::new("git")
            .args(args)
            .output()
            .map_err(|e| format!("cannot run git: {e}"))?;
        if out.status.success() {
            Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
        } else {
            Err(String::from_utf8_lossy(&out.stderr).trim().to_owned())
        }
    };
    let parent = format!("{tag}^");
    let prev = git(&["describe", "--tags", "--abbrev=0", "--match", "v*", &parent]).ok();
    let range = match &prev {
        Some(p) if !p.is_empty() => format!("{p}..{tag}"),
        _ => tag.to_owned(),
    };
    let log = git(&["log", "--first-parent", "--format=%s%x1f%b%x1e", &range])?;
    let titles = notes_from_log(&log);
    if titles.is_empty() {
        return Ok(format!("Toknila {}\n", tag.trim_start_matches('v')));
    }
    Ok(titles.iter().map(|t| format!("- {t}\n")).collect())
}

/// Signs `file` with the official signer and returns the signature, or `None` (with a line on
/// stderr saying so) when no key is configured.
pub fn sign(file: &Path, dir: &Path) -> Result<Option<String>, String> {
    let set = |name: &str| std::env::var_os(name).is_some_and(|v| !v.is_empty());
    if !set(KEY_ENV) && !set(KEY_PATH_ENV) {
        eprintln!(
            "xtask: no {KEY_ENV}: update archive NOT signed (local build; the updater cannot use it)"
        );
        return Ok(None);
    }
    let file_s = file.to_string_lossy();
    run("npx", &["--yes", TAURI_CLI, "signer", "sign", &file_s], dir)?;
    let sig = PathBuf::from(format!("{file_s}.sig"));
    std::fs::read_to_string(&sig)
        .map(Some)
        .map_err(|e| format!("{}: {e}", sig.display()))
}

/// Packs `Toknila.app` from `out` into `Toknila-<version>-<arch>.app.tar.gz`, signs it and writes
/// `latest.json` for that one architecture. Returns the archive path.
pub fn package(out: &Path, version: &str, arch: &str) -> Result<PathBuf, String> {
    let archive = out.join(format!("Toknila-{version}-{arch}.app.tar.gz"));
    // The updater unpacks the archive and drops its first path component, so `Toknila.app` must
    // be the top folder. COPYFILE_DISABLE keeps bsdtar from adding `._` metadata files.
    eprintln!("$ tar -czf {} Toknila.app", archive.display());
    let status = Command::new("tar")
        .env("COPYFILE_DISABLE", "1")
        .args(["-czf"])
        .arg(&archive)
        .arg("Toknila.app")
        .current_dir(out)
        .status()
        .map_err(|e| format!("cannot run tar: {e}"))?;
    if !status.success() {
        return Err(format!("tar exited with {status}"));
    }
    let Some(signature) = sign(&archive, out)? else {
        return Ok(archive);
    };
    let key = platform_key(arch).ok_or_else(|| format!("unsupported architecture {arch}"))?;
    let base =
        std::env::var("TOKNILA_UPDATE_BASE_URL").unwrap_or_else(|_| default_base_url(version));
    let notes =
        std::env::var("TOKNILA_RELEASE_NOTES").unwrap_or_else(|_| format!("Toknila {version}"));
    let json = latest_json(
        version,
        &notes,
        &now_rfc3339(),
        &base,
        &[Platform {
            key: key.to_owned(),
            file: file_name(&archive),
            signature,
        }],
    );
    std::fs::write(out.join("latest.json"), json).map_err(|e| format!("latest.json: {e}"))?;
    Ok(archive)
}

fn file_name(p: &Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// `cargo xtask latest-json <dir> <version> <notes file> [base url]`: one `latest.json` over every
/// `Toknila-<version>-<arch>.app.tar.gz` (with its `.sig`) found in `dir`, written into `dir`.
pub fn latest_json_cmd(args: &[String]) -> Result<(), String> {
    let [dir, version, notes, rest @ ..] = args else {
        return Err(
            "usage: cargo xtask latest-json <dir> <version> <notes file> [base url]".into(),
        );
    };
    let dir = Path::new(dir);
    let notes = std::fs::read_to_string(notes).map_err(|e| format!("{notes}: {e}"))?;
    let base = rest
        .first()
        .cloned()
        .unwrap_or_else(|| default_base_url(version));
    let mut platforms = Vec::new();
    for arch in ["aarch64", "x86_64"] {
        let file = format!("Toknila-{version}-{arch}.app.tar.gz");
        let sig = dir.join(format!("{file}.sig"));
        if !dir.join(&file).is_file() {
            continue;
        }
        let signature = std::fs::read_to_string(&sig)
            .map_err(|e| format!("{} has no signature ({}): {e}", file, sig.display()))?;
        let key = platform_key(arch).ok_or("unsupported architecture")?;
        platforms.push(Platform {
            key: key.to_owned(),
            file,
            signature,
        });
    }
    if platforms.is_empty() {
        return Err(format!(
            "no Toknila-{version}-<arch>.app.tar.gz in {}",
            dir.display()
        ));
    }
    let json = latest_json(version, notes.trim(), &now_rfc3339(), &base, &platforms);
    std::fs::write(dir.join("latest.json"), json).map_err(|e| format!("latest.json: {e}"))?;
    eprintln!("xtask latest-json: PASS ({} platform(s))", platforms.len());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn two() -> Vec<Platform> {
        vec![
            Platform {
                key: "darwin-aarch64".into(),
                file: "Toknila-0.2.1-aarch64.app.tar.gz".into(),
                signature: "SIG-A\n".into(),
            },
            Platform {
                key: "darwin-x86_64".into(),
                file: "Toknila-0.2.1-x86_64.app.tar.gz".into(),
                signature: "SIG-X".into(),
            },
        ]
    }

    #[test]
    fn latest_json_has_what_the_updater_reads() {
        let json = latest_json(
            "0.2.1",
            "Quieter \"cards\"\nFaster start",
            "2026-11-07T10:00:00Z",
            "https://example.test/dl/v0.2.1/",
            &two(),
        );
        let want = r#"{
  "version": "0.2.1",
  "notes": "Quieter \"cards\"\nFaster start",
  "pub_date": "2026-11-07T10:00:00Z",
  "platforms": {
    "darwin-aarch64": {
      "signature": "SIG-A",
      "url": "https://example.test/dl/v0.2.1/Toknila-0.2.1-aarch64.app.tar.gz"
    },
    "darwin-x86_64": {
      "signature": "SIG-X",
      "url": "https://example.test/dl/v0.2.1/Toknila-0.2.1-x86_64.app.tar.gz"
    }
  }
}
"#;
        assert_eq!(json, want);
    }

    #[test]
    fn times_are_rfc3339() {
        assert_eq!(rfc3339(0), "1970-01-01T00:00:00Z");
        assert_eq!(rfc3339(951_782_400 + 3661), "2000-02-29T01:01:01Z");
        assert_eq!(rfc3339(1_794_045_296), "2026-11-07T09:54:56Z");
    }

    #[test]
    fn notes_are_the_merged_titles() {
        let log = "Merge pull request #42 from jh0per/p1-28\x1fP1-28: reviewer without a verdict\n\x1e\
                   Merge pull request #41 from jh0per/p1-27\x1f\nP1-27: threads\n\x1e\
                   Merge branch 'main' into x\x1f\x1e\
                   Fix a typo (#40)\x1f\x1e\
                   P1-20: direct\x1f\x1e";
        assert_eq!(
            notes_from_log(log),
            vec![
                "P1-28: reviewer without a verdict",
                "P1-27: threads",
                "Fix a typo (#40)",
                "P1-20: direct"
            ]
        );
        assert!(notes_from_log("").is_empty());
    }

    #[test]
    fn platform_keys_follow_the_updater_names() {
        assert_eq!(platform_key("aarch64"), Some("darwin-aarch64"));
        assert_eq!(platform_key("x86_64"), Some("darwin-x86_64"));
        assert_eq!(platform_key("riscv"), None);
    }

    #[test]
    fn json_strings_escape_control_characters() {
        assert_eq!(json_string("a\tb\u{1}\\"), "\"a\\tb\\u0001\\\\\"");
    }
}
