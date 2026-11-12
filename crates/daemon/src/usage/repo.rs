//! Which repository a workspace folder belongs to, so the usage page can group worktrees of one
//! repository. Git is asked once per path and the answer kept for the daemon's life.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, PoisonError};

use toknila_core::usage::UsageItem;

/// The answers so far: a path to its main repository's folder, or `None` for a folder that is
/// not in a repository or does not exist.
#[derive(Default)]
pub(super) struct RepoCache {
    known: Mutex<HashMap<String, Option<PathBuf>>>,
}

impl RepoCache {
    /// Fills `repo` and `name` of each line whose folder is in a repository. Blocking: git runs
    /// for a path only the first time it is seen.
    pub(super) fn annotate(&self, items: &mut [UsageItem]) {
        let mut known = self.known.lock().unwrap_or_else(PoisonError::into_inner);
        for item in items {
            let found = known
                .entry(item.label.clone())
                .or_insert_with(|| repo_of(Path::new(&item.label)));
            if let Some(repo) = found {
                item.repo = Some(repo.display().to_string());
                item.name = repo
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .or_else(|| Some(repo.display().to_string()));
            }
        }
    }
}

/// The main repository's folder of the repository `folder` is in (or is a worktree of).
fn repo_of(folder: &Path) -> Option<PathBuf> {
    if !folder.is_dir() {
        return None;
    }
    let out = Command::new("git")
        .arg("-C")
        .arg(folder)
        .args(["rev-parse", "--git-common-dir"])
        .output()
        .ok()
        .filter(|o| o.status.success())?;
    let printed = String::from_utf8_lossy(&out.stdout).trim().to_owned();
    if printed.is_empty() {
        return None;
    }
    // Relative to the folder git was asked in (`.git` in a main checkout).
    let common = folder.join(printed);
    let common = common.canonicalize().unwrap_or(common);
    // A normal repository keeps its data in `<repo>/.git`; a bare one is its own folder.
    if common.file_name().is_some_and(|n| n == ".git") {
        common.parent().map(Path::to_owned)
    } else {
        Some(common)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::testing::{git, init_repo};
    use toknila_core::usage::UsageFigures;

    fn item(path: &Path) -> UsageItem {
        UsageItem {
            label: path.display().to_string(),
            figures: UsageFigures::default(),
            repo: None,
            name: None,
        }
    }

    #[test]
    fn worktrees_of_one_repository_share_it_and_other_folders_have_none() {
        let dir = tempfile::tempdir().expect("tmp");
        let root = dir.path().canonicalize().expect("canonical");
        let main = root.join("imagegen-backend");
        init_repo(&main, "main");
        let (a, b) = (root.join("wt/a"), root.join("other-tool/b"));
        git(
            &main,
            &[
                "worktree",
                "add",
                "--quiet",
                "-b",
                "a",
                &a.display().to_string(),
            ],
        );
        git(
            &main,
            &[
                "worktree",
                "add",
                "--quiet",
                "-b",
                "b",
                &b.display().to_string(),
            ],
        );
        let loose = root.join("loose");
        std::fs::create_dir_all(&loose).expect("mkdir");
        let missing = root.join("gone");

        let mut items = vec![
            item(&main),
            item(&a),
            item(&b),
            item(&loose),
            item(&missing),
        ];
        let cache = RepoCache::default();
        cache.annotate(&mut items);
        let seen: Vec<_> = items
            .iter()
            .map(|i| (i.repo.clone(), i.name.clone()))
            .collect();
        let want = (
            Some(main.display().to_string()),
            Some("imagegen-backend".to_owned()),
        );
        assert_eq!(
            seen,
            [want.clone(), want.clone(), want, (None, None), (None, None)]
        );
    }

    #[test]
    fn git_is_asked_once_per_path() {
        let dir = tempfile::tempdir().expect("tmp");
        let main = dir.path().join("r");
        init_repo(&main, "main");
        let cache = RepoCache::default();
        cache.annotate(&mut [item(&main)]);
        // The folder is gone, yet the first answer stands: git is not asked again.
        std::fs::remove_dir_all(&main).expect("remove");
        let mut again = [item(&main)];
        cache.annotate(&mut again);
        assert!(again[0].repo.is_some());
    }
}
