//! Which tool folders an agent's `PATH` should get beyond the daemon's own. An app opened from
//! Finder or the Start menu has the `PATH` of the login shell, and that shell often does not add
//! the folders where per-user tools live (`~/.cargo/bin`), so the first `cargo` of a card fails
//! and the agent then prefixes `export PATH=...` to every command, which no allow rule matches.
//!
//! This module only decides; the host says what exists on disk.

use std::path::{Path, PathBuf};

use crate::system::{Check, CheckStatus, Tool};

/// The per-user tool folders, relative to the home folder, on every platform but Windows.
const HOME_DIRS: [&str; 6] = [
    ".cargo/bin",
    ".local/bin",
    ".volta/bin",
    ".bun/bin",
    "go/bin",
    ".deno/bin",
];

/// The folders that exist, are well known for `os` (`std::env::consts::OS`) and are not yet in
/// `on_path`, in the order they should be appended. `exists` says whether a folder is on disk.
pub fn missing_tool_dirs(
    os: &str,
    home: &Path,
    on_path: &[PathBuf],
    exists: &dyn Fn(&Path) -> bool,
) -> Vec<PathBuf> {
    let mut candidates: Vec<PathBuf> = if os == "windows" {
        // Only the Rust toolchain's folder has a settled place there.
        vec![home.join(".cargo").join("bin")]
    } else {
        HOME_DIRS.iter().map(|d| home.join(d)).collect()
    };
    if os == "macos" {
        candidates.push(PathBuf::from("/opt/homebrew/bin"));
        candidates.push(PathBuf::from("/usr/local/bin"));
    }
    candidates
        .into_iter()
        .filter(|d| !on_path.contains(d) && exists(d))
        .collect()
}

/// The first word of a check command, which is the program the agent runs.
pub fn program_of(command: &str) -> Option<&str> {
    command.split_whitespace().next()
}

/// The folder a well-known program is installed to, for the hint.
fn usual_folder(program: &str) -> Option<&'static str> {
    match program {
        "cargo" | "rustc" | "rustup" => Some("~/.cargo/bin"),
        "bun" | "bunx" => Some("~/.bun/bin"),
        "deno" => Some("~/.deno/bin"),
        "node" | "npm" | "npx" | "pnpm" | "yarn" => Some("~/.volta/bin"),
        _ => None,
    }
}

/// Whether a check that failed with `code` and the output `tail` never ran its command: the
/// shell could not find the program (exit 127 on unix, 9009 on Windows, or the shell's own
/// message). That is a problem of the machine, not of the worker's change.
pub fn command_not_found(code: Option<i32>, tail: &str) -> bool {
    matches!(code, Some(127 | 9009))
        || tail.contains("command not found")
        || tail.contains("is not recognized as an internal or external command")
}

/// What a person reads when the check command `command` could not run: the program, the hint of
/// [`check_command`], and that the worker's change is untouched.
pub fn cannot_run_text(command: &str) -> String {
    let program = program_of(command).unwrap_or(command);
    let hint = check_command(command, None).hint;
    format!(
        "The check command `{command}` could not run on this machine: its first word `{program}` \
         was not found. {hint} The agent's work is untouched and was not judged: you can Accept \
         it or Return it."
    )
}

/// The verdict on the project's check command: `found` is the folder that holds its program on the
/// agents' `PATH`, if any.
pub fn check_command(command: &str, found: Option<&Path>) -> Check {
    let program = program_of(command).unwrap_or(command);
    let (status, detail, hint) = match found {
        Some(dir) => (
            CheckStatus::Ok,
            format!("`{program}` is found in {}.", dir.display()),
            "Agents run the check command as given, without asking.".to_owned(),
        ),
        None => (
            CheckStatus::Missing,
            format!(
                "`{program}`, the first word of the check command `{command}`, is not on the \
                 agents' PATH."
            ),
            match usual_folder(program) {
                Some(dir) => {
                    format!("Add {dir} to the PATH of your login shell, then restart Toknila.")
                }
                None => format!(
                    "Install `{program}`, or add the folder that holds it to the PATH of your \
                     login shell, then restart Toknila."
                ),
            },
        ),
    };
    Check {
        tool: Tool::CheckCommand,
        status,
        version: None,
        detail,
        hint,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all(_: &Path) -> bool {
        true
    }

    #[test]
    fn existing_folders_not_yet_on_the_path_are_appended_in_order() {
        let home = Path::new("/h");
        let on_path = [PathBuf::from("/h/.local/bin"), PathBuf::from("/usr/bin")];
        let got = missing_tool_dirs("linux", home, &on_path, &all);
        let want: Vec<PathBuf> = [
            ".cargo/bin",
            ".volta/bin",
            ".bun/bin",
            "go/bin",
            ".deno/bin",
        ]
        .iter()
        .map(|d| home.join(d))
        .collect();
        assert_eq!(got, want);
    }

    #[test]
    fn folders_that_do_not_exist_are_left_out() {
        let home = Path::new("/h");
        let only_cargo = |p: &Path| p == Path::new("/h/.cargo/bin");
        assert_eq!(
            missing_tool_dirs("linux", home, &[], &only_cargo),
            vec![PathBuf::from("/h/.cargo/bin")]
        );
        assert!(missing_tool_dirs("linux", home, &[], &|_| false).is_empty());
    }

    #[test]
    fn macos_also_gets_homebrew_and_usr_local() {
        let got = missing_tool_dirs("macos", Path::new("/h"), &[], &all);
        assert_eq!(got[got.len() - 2], PathBuf::from("/opt/homebrew/bin"));
        assert_eq!(got[got.len() - 1], PathBuf::from("/usr/local/bin"));
        let linux = missing_tool_dirs("linux", Path::new("/h"), &[], &all);
        assert!(!linux.contains(&PathBuf::from("/opt/homebrew/bin")));
    }

    #[test]
    fn windows_gets_the_cargo_folder_only() {
        assert_eq!(
            missing_tool_dirs("windows", Path::new("C:/Users/me"), &[], &all),
            vec![Path::new("C:/Users/me").join(".cargo").join("bin")]
        );
    }

    #[test]
    fn the_check_command_verdict_names_the_folder_to_add() {
        let ok = check_command("cargo xtask verify", Some(Path::new("/h/.cargo/bin")));
        assert_eq!(ok.status, CheckStatus::Ok);
        let bad = check_command("cargo xtask verify", None);
        assert_eq!(bad.status, CheckStatus::Missing);
        assert!(bad.hint.contains("~/.cargo/bin"), "{}", bad.hint);
        assert!(bad.detail.contains("`cargo`"));
        let other = check_command("make test", None);
        assert!(other.hint.starts_with("Install `make`"), "{}", other.hint);
    }
}
