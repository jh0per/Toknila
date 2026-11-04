//! Architecture guards: rules about the shape of the code that `cargo xtask guards` checks, so
//! the layering in `docs/architecture.md` is enforced by CI rather than by review.
//!
//! - Dependency rules: a crate must not depend on what its layer may not use.
//! - Size ratchet: a production source file stays under [`MAX_LINES`]; files already over it
//!   are listed in [`OVERSIZED`] with their current size and may only shrink.

use std::path::{Path, PathBuf};

use crate::output;

/// Whether a dependency rule looks at direct dependencies only or the whole tree.
#[derive(Debug, Clone, Copy)]
enum Scope {
    Direct,
    Transitive,
}

/// A crate that must not depend on the listed packages.
#[derive(Debug)]
struct DepRule {
    package: &'static str,
    scope: Scope,
    banned: &'static [&'static str],
    why: &'static str,
}

/// Crates `toknila-core` must never depend on, directly or transitively. Core stays pure:
/// no async runtime, no storage, no transport.
pub(crate) const CORE_BANNED: &[&str] = &[
    "tokio",
    "rusqlite",
    "sqlx",
    "axum",
    "hyper",
    "reqwest",
    "portable-pty",
];

const DEP_RULES: &[DepRule] = &[
    DepRule {
        package: "toknila-core",
        scope: Scope::Transitive,
        banned: CORE_BANNED,
        why: "core is the pure domain layer",
    },
    DepRule {
        package: "toknila-app",
        scope: Scope::Transitive,
        banned: CORE_BANNED,
        why: "use cases reach storage and transport only through ports",
    },
    DepRule {
        package: "toknila",
        scope: Scope::Direct,
        banned: &["rusqlite", "axum", "utoipa", "utoipa-axum", "tower", "ureq"],
        why: "the CLI is a thin client: storage and HTTP serving belong to the daemon, HTTP calls to toknila-client",
    },
];

/// Longest production source file, in lines before its `#[cfg(test)]` block.
pub(crate) const MAX_LINES: usize = 400;

/// Files over [`MAX_LINES`] when the guard was introduced, with their size then. Each may only
/// shrink; once a file is at or under the limit its entry must go (the guard says so).
const OVERSIZED: &[(&str, usize)] = &[("crates/daemon/src/store/codec.rs", 728)];

/// Source trees the size ratchet covers, relative to the repository root.
const SOURCE_DIRS: &[&str] = &["crates", "xtask/src", "apps/web/src"];

/// Runs every guard; the first failure stops.
pub fn run(root: &Path) -> Result<(), String> {
    for rule in DEP_RULES {
        check_deps(rule)?;
    }
    check_sizes(root)?;
    Ok(())
}

fn check_deps(rule: &DepRule) -> Result<(), String> {
    let mut args = vec![
        "tree",
        "--package",
        rule.package,
        "--edges",
        "normal",
        "--prefix",
        "none",
        "--locked",
    ];
    if let Scope::Direct = rule.scope {
        args.extend(["--depth", "1"]);
    }
    let tree = output(&args)?;
    // Control: the tree must at least list the package itself, or the probe read nothing.
    let me = format!("{} ", rule.package);
    if !tree.lines().any(|l| l.starts_with(&me)) {
        return Err(format!(
            "cargo tree output did not mention {}; guard probe is blind",
            rule.package
        ));
    }
    let hits = banned_in_tree(&tree, rule.banned);
    if !hits.is_empty() {
        return Err(format!(
            "{} depends on banned crates: {} ({})",
            rule.package,
            hits.join(", "),
            rule.why
        ));
    }
    eprintln!("guard deps {}: PASS", rule.package);
    Ok(())
}

/// Names from `banned` that appear as packages in `cargo tree --prefix none` output.
pub(crate) fn banned_in_tree(tree: &str, banned: &[&str]) -> Vec<String> {
    let mut hits: Vec<String> = tree
        .lines()
        .filter_map(|line| line.split_whitespace().next())
        .filter(|name| banned.contains(name))
        .map(str::to_owned)
        .collect();
    hits.sort();
    hits.dedup();
    hits
}

fn check_sizes(root: &Path) -> Result<(), String> {
    let mut files = Vec::new();
    for dir in SOURCE_DIRS {
        collect_sources(&root.join(dir), &mut files)?;
    }
    let mut measured = Vec::with_capacity(files.len());
    for path in &files {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let rel = path
            .strip_prefix(root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");
        measured.push((rel, production_lines(&text)));
    }
    // Control: the walk must have found the files we know are big, or it read nothing.
    if !measured
        .iter()
        .any(|(p, _)| p == "crates/core/src/machine.rs")
    {
        return Err("size guard did not find crates/core/src/machine.rs; probe is blind".into());
    }
    let problems = size_problems(&measured, MAX_LINES, OVERSIZED);
    if !problems.is_empty() {
        return Err(format!("file size guard:\n  {}", problems.join("\n  ")));
    }
    eprintln!(
        "guard file-size (<= {MAX_LINES} lines, {} files): PASS",
        measured.len()
    );
    Ok(())
}

fn collect_sources(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for entry in entries {
        let path = entry.map_err(|e| e.to_string())?.path();
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        if path.is_dir() {
            if !matches!(name, "target" | "node_modules" | "dist" | "tests") {
                collect_sources(&path, out)?;
            }
        } else if is_production_source(name) {
            out.push(path);
        }
    }
    Ok(())
}

/// Source files the ratchet measures: Rust and TypeScript, not test modules.
pub(crate) fn is_production_source(file_name: &str) -> bool {
    let source = [".rs", ".ts", ".tsx"]
        .iter()
        .any(|ext| file_name.ends_with(ext));
    let test = file_name == "tests.rs"
        || file_name.ends_with("_tests.rs")
        || file_name.ends_with(".test.ts")
        || file_name.ends_with(".test.tsx");
    source && !test && !file_name.ends_with(".d.ts")
}

/// Lines before the first top-level `#[cfg(test)]`: inline test modules do not count.
pub(crate) fn production_lines(text: &str) -> usize {
    text.lines()
        .take_while(|l| !l.starts_with("#[cfg(test)]"))
        .count()
}

/// Everything wrong with the measured sizes against the limit and the allowlist.
pub(crate) fn size_problems(
    measured: &[(String, usize)],
    limit: usize,
    oversized: &[(&str, usize)],
) -> Vec<String> {
    let mut out = Vec::new();
    for (path, lines) in measured {
        let cap = oversized
            .iter()
            .find(|(p, _)| p == path)
            .map_or(limit, |(_, cap)| *cap);
        if *lines > cap {
            out.push(format!(
                "{path}: {lines} lines, limit {cap}; split it by responsibility"
            ));
        }
    }
    for (path, cap) in oversized {
        match measured.iter().find(|(p, _)| p == path) {
            None => out.push(format!(
                "{path}: listed in OVERSIZED but gone; remove the entry"
            )),
            Some((_, lines)) if *lines <= limit => out.push(format!(
                "{path}: {lines} lines is within {limit}; remove its OVERSIZED entry"
            )),
            Some((_, lines)) if *lines < *cap => out.push(format!(
                "{path}: shrank to {lines} lines; lower its OVERSIZED cap from {cap}"
            )),
            Some(_) => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    // Control for the guard: a tree that does contain a banned crate must be caught,
    // including when it shows up more than once.
    #[test]
    fn banned_crate_is_detected() {
        let tree = "toknila-core v0.0.1 (/repo/crates/core)\ntokio v1.40.0\nserde v1.0.0\ntokio v1.40.0 (*)\n";
        assert_eq!(banned_in_tree(tree, CORE_BANNED), vec!["tokio".to_owned()]);
    }

    #[test]
    fn similar_names_are_not_false_positives() {
        let tree = "toknila-core v0.0.1\ntokio-util-free v0.1.0\nserde v1.0.0\n";
        assert!(banned_in_tree(tree, CORE_BANNED).is_empty());
    }

    #[test]
    fn inline_tests_do_not_count() {
        let text =
            "fn a() {}\nfn b() {}\n#[cfg(test)]\nmod tests {\n    #[test]\n    fn t() {}\n}\n";
        assert_eq!(production_lines(text), 2);
    }

    #[test]
    fn test_files_are_skipped() {
        assert!(is_production_source("graph.rs"));
        assert!(is_production_source("App.tsx"));
        assert!(!is_production_source("tests.rs"));
        assert!(!is_production_source("roundtrip_tests.rs"));
        assert!(!is_production_source("vite-env.d.ts"));
        assert!(!is_production_source("reduce.test.ts"));
        assert!(is_production_source("reduce.ts"));
        assert!(!is_production_source("README.md"));
    }

    fn sizes(rows: &[(&str, usize)]) -> Vec<(String, usize)> {
        rows.iter().map(|(p, n)| ((*p).to_owned(), *n)).collect()
    }

    #[test]
    fn a_new_big_file_fails() {
        let problems = size_problems(&sizes(&[("a.rs", 401)]), 400, &[]);
        assert_eq!(
            problems,
            vec!["a.rs: 401 lines, limit 400; split it by responsibility"]
        );
    }

    #[test]
    fn a_listed_file_may_not_grow() {
        let problems = size_problems(&sizes(&[("big.rs", 501)]), 400, &[("big.rs", 500)]);
        assert_eq!(
            problems,
            vec!["big.rs: 501 lines, limit 500; split it by responsibility"]
        );
    }

    #[test]
    fn the_ratchet_tightens() {
        let shrank = size_problems(&sizes(&[("big.rs", 450)]), 400, &[("big.rs", 500)]);
        assert_eq!(
            shrank,
            vec!["big.rs: shrank to 450 lines; lower its OVERSIZED cap from 500"]
        );
        let fixed = size_problems(&sizes(&[("big.rs", 300)]), 400, &[("big.rs", 500)]);
        assert_eq!(
            fixed,
            vec!["big.rs: 300 lines is within 400; remove its OVERSIZED entry"]
        );
        let gone = size_problems(&[], 400, &[("big.rs", 500)]);
        assert_eq!(
            gone,
            vec!["big.rs: listed in OVERSIZED but gone; remove the entry"]
        );
    }

    #[test]
    fn files_within_their_caps_pass() {
        let rows = sizes(&[("a.rs", 400), ("big.rs", 500)]);
        assert!(size_problems(&rows, 400, &[("big.rs", 500)]).is_empty());
    }
}
