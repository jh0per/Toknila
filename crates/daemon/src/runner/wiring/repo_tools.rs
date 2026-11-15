//! The toolchains a repository holds, at its root and one level down, for the reviewer's rules.

use std::path::Path;

use toknila_core::RepoTool;

/// The folders one level below the root that usually hold a package of their own.
const PACKAGE_PARENTS: [&str; 3] = ["apps", "packages", "web"];

/// The package manager of the folder `dir` (by its lock file), if it holds a `package.json`.
fn node_tool(dir: &Path) -> Option<&'static str> {
    if !dir.join("package.json").is_file() {
        return None;
    }
    Some(if dir.join("pnpm-lock.yaml").exists() {
        "pnpm"
    } else if dir.join("yarn.lock").exists() {
        "yarn"
    } else {
        "npm"
    })
}

/// The relative folders the check command names (`cd docs`, `--prefix "tools"`): its words that are
/// folders of the repository.
fn named_folders(root: &Path, check_command: Option<&str>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for word in check_command.unwrap_or_default().split_whitespace() {
        let word = word.trim_matches(|c| "\"';&|()".contains(c));
        let relative = !word.is_empty()
            && !word.starts_with(['/', '-', '.'])
            && !word.contains(['\\', ':'])
            && !word.contains("..");
        if relative && root.join(word).is_dir() && !out.iter().any(|o| o == word) {
            out.push(word.to_owned());
        }
    }
    out
}

/// The folders below `parent`, by name, as relative paths.
fn children(root: &Path, parent: &str) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(root.join(parent)) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .filter_map(Result::ok)
        .filter(|e| e.path().is_dir())
        .filter_map(|e| e.file_name().into_string().ok())
        .map(|n| format!("{parent}/{n}"))
        .collect();
    names.sort();
    names
}

/// The toolchains the repository in `dir` holds: `cargo` for a root `Cargo.toml`, and `npm`, `pnpm`
/// or `yarn` (by lock file) for each `package.json` at the root, in a folder the check command
/// names, or in a folder of `apps/*`, `packages/*` and `web/*`, each with its folder.
pub(in crate::runner) fn repo_tools(dir: &Path, check_command: Option<&str>) -> Vec<RepoTool> {
    let mut tools = Vec::new();
    if dir.join("Cargo.toml").exists() {
        tools.push(RepoTool {
            tool: "cargo".to_owned(),
            dir: String::new(),
        });
    }
    let mut folders = vec![String::new()];
    folders.extend(named_folders(dir, check_command));
    for parent in PACKAGE_PARENTS {
        folders.extend(children(dir, parent));
    }
    let mut seen: Vec<String> = Vec::new();
    for folder in folders {
        if seen.contains(&folder) {
            continue;
        }
        if let Some(tool) = node_tool(&dir.join(&folder)) {
            tools.push(RepoTool {
                tool: tool.to_owned(),
                dir: folder.clone(),
            });
        }
        seen.push(folder);
    }
    tools
}
