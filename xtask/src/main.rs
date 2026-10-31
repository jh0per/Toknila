//! `cargo xtask <task>`: repository automation that must behave the same locally and in CI.
//!
//! - `verify`: fmt check, clippy with warnings denied, tests, architecture guards.
//! - `guards`: architecture guards only.

use std::process::{Command, ExitCode};

/// Crates `toknila-core` must never depend on, directly or transitively. Core stays pure:
/// no async runtime, no storage, no transport.
const CORE_BANNED: &[&str] = &[
    "tokio",
    "rusqlite",
    "sqlx",
    "axum",
    "hyper",
    "reqwest",
    "portable-pty",
];

fn main() -> ExitCode {
    let task = std::env::args().nth(1).unwrap_or_default();
    let result = match task.as_str() {
        "verify" => verify(),
        "guards" => guards(),
        _ => {
            eprintln!("usage: cargo xtask <verify|guards>");
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
    eprintln!("xtask verify: PASS");
    Ok(())
}

fn guards() -> Result<(), String> {
    let tree = output(&[
        "tree",
        "--package",
        "toknila-core",
        "--edges",
        "normal",
        "--prefix",
        "none",
        "--locked",
    ])?;
    let hits = banned_in_tree(&tree, CORE_BANNED);
    if !hits.is_empty() {
        return Err(format!(
            "toknila-core depends on banned crates: {}",
            hits.join(", ")
        ));
    }
    // Control: the tree must at least list core itself, or the probe read nothing.
    if !tree.lines().any(|l| l.starts_with("toknila-core ")) {
        return Err("cargo tree output did not mention toknila-core; guard probe is blind".into());
    }
    eprintln!("guard core-is-pure: PASS");
    Ok(())
}

/// Names from `banned` that appear as packages in `cargo tree --prefix none` output.
fn banned_in_tree(tree: &str, banned: &[&str]) -> Vec<String> {
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
}
