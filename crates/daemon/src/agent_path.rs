//! The `PATH` an agent's process gets: the daemon's own, plus the well-known per-user tool
//! folders that exist and are not on it (the decision is `toknila_core::toolpath`; this module
//! supplies the facts).

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use toknila_core::toolpath;

/// The folders of `path` followed by the missing tool folders that exist under `home`.
pub(crate) fn dirs_with(path: Option<&OsString>, home: Option<&Path>, os: &str) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = path
        .map(|p| std::env::split_paths(p).collect())
        .unwrap_or_default();
    if let Some(home) = home {
        let extra = toolpath::missing_tool_dirs(os, home, &dirs, &|d| d.is_dir());
        dirs.extend(extra);
    }
    dirs
}

/// The agents' `PATH` folders on this machine.
pub(crate) fn agent_dirs() -> Vec<PathBuf> {
    dirs_with(
        std::env::var_os("PATH").as_ref(),
        dirs::home_dir().as_deref(),
        std::env::consts::OS,
    )
}

/// The `PATH` value to set for an agent, when it differs from the daemon's.
pub(crate) fn agent_path_value() -> Option<String> {
    let own = std::env::var_os("PATH");
    let joined = std::env::join_paths(agent_dirs()).ok()?;
    if Some(&joined) == own.as_ref() {
        return None;
    }
    joined.into_string().ok()
}

/// The folder among `dirs` that holds the program `program`.
pub(crate) fn find_in(program: &str, dirs: &[PathBuf]) -> Option<PathBuf> {
    let names: Vec<String> = if cfg!(windows) {
        ["", ".exe", ".cmd", ".bat"]
            .iter()
            .map(|e| format!("{program}{e}"))
            .collect()
    } else {
        vec![program.to_owned()]
    };
    dirs.iter()
        .find(|d| names.iter().any(|n| d.join(n).is_file()))
        .cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tool_folder_that_exists_is_appended_after_the_daemons_path() {
        let tmp = tempfile::tempdir().expect("tmp");
        let cargo = tmp.path().join(".cargo").join("bin");
        std::fs::create_dir_all(&cargo).expect("dir");
        let path = std::env::join_paths(["/usr/bin"]).expect("join");
        let got = dirs_with(Some(&path), Some(tmp.path()), "linux");
        assert_eq!(got, vec![PathBuf::from("/usr/bin"), cargo.clone()]);
        // Already there: not added twice.
        let path = std::env::join_paths([PathBuf::from("/usr/bin"), cargo]).expect("join");
        assert_eq!(dirs_with(Some(&path), Some(tmp.path()), "linux").len(), 2);
    }

    #[test]
    fn a_program_is_found_in_the_folder_that_holds_it() {
        let tmp = tempfile::tempdir().expect("tmp");
        let name = if cfg!(windows) { "cargo.exe" } else { "cargo" };
        std::fs::write(tmp.path().join(name), "").expect("file");
        let dirs = vec![PathBuf::from("/nonexistent"), tmp.path().to_owned()];
        assert_eq!(find_in("cargo", &dirs), Some(tmp.path().to_owned()));
        assert_eq!(find_in("nope", &dirs), None);
    }
}
