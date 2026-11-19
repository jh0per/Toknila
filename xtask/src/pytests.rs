//! The tests of the workflow scripts: every `.github/scripts/test_*.py`, run by `cargo xtask verify`
//! (and alone by `cargo xtask scripts`) with `python3 -B`, so they run on every push on the three
//! systems of CI. No bytecode is written (`-B`, and `PYTHONDONTWRITEBYTECODE` for the scripts the
//! tests start). Without Python 3 the step says so in one line and verify goes on.

use std::path::{Path, PathBuf};
use std::process::Command;

/// The test files in `dir`, sorted by name.
fn test_files(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let entries = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut out: Vec<PathBuf> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("test_") && n.ends_with(".py"))
        })
        .collect();
    out.sort();
    Ok(out)
}

/// Does `--version` output name Python 3? (Windows' Store stub prints something else, or nothing.)
fn is_python3(version_output: &str) -> bool {
    version_output.trim_start().starts_with("Python 3.")
}

/// The first of `python3` and `python` that is Python 3.
fn python() -> Option<&'static str> {
    ["python3", "python"].into_iter().find(|p| {
        Command::new(p).arg("--version").output().is_ok_and(|o| {
            o.status.success()
                && is_python3(&format!(
                    "{}{}",
                    String::from_utf8_lossy(&o.stdout),
                    String::from_utf8_lossy(&o.stderr)
                ))
        })
    })
}

/// A path without the `\\?\` prefix Windows' `canonicalize` adds: other programs (Python) cannot
/// join `..` onto a verbatim path.
fn plain(p: &Path) -> PathBuf {
    let s = p.to_string_lossy();
    if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
        return PathBuf::from(format!(r"\\{rest}"));
    }
    match s.strip_prefix(r"\\?\") {
        Some(rest) => PathBuf::from(rest),
        None => p.to_path_buf(),
    }
}

/// Runs every test of `.github/scripts`; the first failure stops.
pub fn run(root: &Path) -> Result<(), String> {
    let root = &plain(root);
    let tests = test_files(&root.join(".github/scripts"))?;
    let Some(py) = python() else {
        eprintln!(
            "xtask scripts: python3 not found; the {} tests of .github/scripts did not run",
            tests.len()
        );
        return Ok(());
    };
    for t in &tests {
        let rel = t
            .strip_prefix(root)
            .unwrap_or(t)
            .to_string_lossy()
            .replace('\\', "/");
        eprintln!("$ {py} -B {rel}");
        let status = Command::new(py)
            .arg("-B")
            .arg(t)
            .env("PYTHONDONTWRITEBYTECODE", "1")
            .current_dir(root)
            .status()
            .map_err(|e| format!("cannot run {py}: {e}"))?;
        if !status.success() {
            return Err(format!("`{py} -B {rel}` exited with {status}"));
        }
    }
    eprintln!("xtask scripts: PASS ({} test files)", tests.len());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_test_file_of_the_scripts_folder_is_found_in_order() {
        let root = crate::app::root().expect("repo root");
        let names: Vec<String> = test_files(&root.join(".github/scripts"))
            .expect("read")
            .iter()
            .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
            .collect();
        // Control: the two scripts' tests are found, and the scripts themselves are not.
        assert_eq!(
            names,
            [
                "test_assemble_release.py",
                "test_cla_workflow.py",
                "test_license.py",
                "test_links.py",
                "test_wait_for_ci.py"
            ]
        );
    }

    #[test]
    fn verbatim_prefixes_are_stripped() {
        let p = |s: &str| plain(Path::new(s));
        assert_eq!(p(r"\\?\D:\a\b"), PathBuf::from(r"D:\a\b"));
        assert_eq!(
            p(r"\\?\UNC\server\share\x"),
            PathBuf::from(r"\\server\share\x")
        );
        assert_eq!(p(r"D:\a\b"), PathBuf::from(r"D:\a\b"));
        assert_eq!(p("/usr/x"), PathBuf::from("/usr/x"));
    }

    #[test]
    fn only_python_3_counts() {
        assert!(is_python3("Python 3.12.4\n"));
        assert!(!is_python3("Python 2.7.18"));
        assert!(!is_python3(""));
        assert!(!is_python3(
            "Python was not found; run without arguments to install from the Microsoft Store"
        ));
    }
}
