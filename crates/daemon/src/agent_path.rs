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

/// The `PATH` the daemon process should have: its own folders followed by the missing tool
/// folders under `home`; `None` when that is what it already has.
pub(crate) fn process_path(
    own: Option<&OsString>,
    home: Option<&Path>,
    os: &str,
) -> Option<String> {
    let joined = std::env::join_paths(dirs_with(own, home, os)).ok()?;
    if Some(&joined) == own {
        return None;
    }
    joined.into_string().ok()
}

/// Gives the daemon process the agents' `PATH`, and returns the folders it appended.
///
/// An app opened from Finder has a bare `PATH`, and everything the daemon spawns (the check
/// command, the release steps, the merge's git calls, the agents) inherits it. Setting it once
/// for the process, before any thread exists, gives all of them the same `PATH` without passing
/// it to every `Command`; the system check then judges the check command against the `PATH` it
/// really runs with. Call it first thing in `daemon run`, before logging and the runtime start.
#[allow(unsafe_code)]
pub fn adopt_agent_path() -> Vec<PathBuf> {
    let own = std::env::var_os("PATH");
    let Some(value) = process_path(
        own.as_ref(),
        dirs::home_dir().as_deref(),
        std::env::consts::OS,
    ) else {
        return Vec::new();
    };
    let before: Vec<PathBuf> = own
        .as_ref()
        .map(|p| std::env::split_paths(p).collect())
        .unwrap_or_default();
    let added = std::env::split_paths(&value)
        .filter(|d| !before.contains(d))
        .collect();
    // SAFETY: called from `daemon run` before the logger or the tokio runtime started a thread,
    // so no other thread can read or write the environment while it changes.
    unsafe { std::env::set_var("PATH", value) };
    added
}

/// One info line naming the folders [`adopt_agent_path`] appended; call it once logging runs.
pub fn log_adopted_path(added: &[PathBuf]) {
    if !added.is_empty() {
        let folders: Vec<String> = added.iter().map(|d| d.display().to_string()).collect();
        tracing::info!(folders = %folders.join(", "), "added tool folders to the daemon's PATH");
    }
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
    fn the_process_path_gets_the_tool_folder_and_only_when_it_changes() {
        let tmp = tempfile::tempdir().expect("tmp");
        let cargo = tmp.path().join(".cargo").join("bin");
        std::fs::create_dir_all(&cargo).expect("dir");
        let own = std::env::join_paths(["/usr/bin"]).expect("join");
        let got = process_path(Some(&own), Some(tmp.path()), "linux").expect("changed");
        let dirs: Vec<PathBuf> = std::env::split_paths(&got).collect();
        assert_eq!(dirs, vec![PathBuf::from("/usr/bin"), cargo]);
        // Run again on its own result: nothing left to add.
        let again = std::ffi::OsString::from(got);
        assert_eq!(process_path(Some(&again), Some(tmp.path()), "linux"), None);
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
