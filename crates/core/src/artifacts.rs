//! The built-artifact rule: files a ship's commits add that are build outputs, caches or files
//! the repository ignores never go in a hand-in. The host lists the added files and asks git
//! which of them the repository's `.gitignore` ignores; this module judges the rest, pure.

/// Directory names that mark a built artifact (`__pycache__/`, `node_modules/`, ...).
const DIRS: [&str; 4] = ["__pycache__", "node_modules", "target", "dist"];
/// File names that mark one.
const NAMES: [&str; 1] = [".DS_Store"];
/// Extensions that mark one (`*.pyc`, `*.log`, `*.tmp`).
const EXTENSIONS: [&str; 3] = ["pyc", "log", "tmp"];

/// The position, among the path's components, of the one that makes it a built artifact by the
/// fixed list: a directory of [`DIRS`] above the file, or the file itself by name or extension.
fn listed_at(parts: &[&str]) -> Option<usize> {
    let last = parts.len().checked_sub(1)?;
    if let Some(at) = parts[..last].iter().position(|p| DIRS.contains(p)) {
        return Some(at);
    }
    let file = parts[last];
    let by_ext = file
        .rsplit_once('.')
        .is_some_and(|(stem, ext)| !stem.is_empty() && EXTENSIONS.contains(&ext));
    (NAMES.contains(&file) || by_ext).then_some(last)
}

/// Whether the fixed list names `path` (a `/`-separated path in the repository).
pub fn is_listed(path: &str) -> bool {
    listed_at(&components(path)).is_some()
}

fn components(path: &str) -> Vec<&str> {
    path.split('/').filter(|p| !p.is_empty()).collect()
}

/// Whether the card's prompt names the path: the file, the end of its path from a directory at
/// or above the marked one, or a directory from the root down to the marked one or below.
fn named_by(prompt: &str, parts: &[&str], marked: usize) -> bool {
    let tail = (0..=marked).any(|from| prompt.contains(&parts[from..].join("/")));
    let head = (marked..parts.len() - 1)
        .any(|to| prompt.contains(&format!("{}/", parts[..=to].join("/"))));
    tail || head
}

/// The files among `added` that a hand-in may not carry: those the fixed list names and those in
/// `ignored` (what the repository's own `.gitignore` ignores among them), except a path the
/// card's `prompt` names. In the order of `added`.
pub fn offending(added: &[String], ignored: &[String], prompt: &str) -> Vec<String> {
    added
        .iter()
        .filter(|path| {
            let parts = components(path);
            if parts.is_empty() {
                return false;
            }
            let marked = listed_at(&parts).or_else(|| {
                ignored
                    .contains(path)
                    .then(|| parts.len().saturating_sub(1))
            });
            marked.is_some_and(|m| !named_by(prompt, &parts, m))
        })
        .cloned()
        .collect()
}

/// What the agent is told when `finished` is refused for `files`: each one, and the way out.
pub fn refusal(files: &[String]) -> String {
    format!(
        "Not finished: your commits add {}: {}. these are built or ignored files; remove them \
         from git (git rm --cached) and hand in again",
        if files.len() == 1 { "1 file" } else { "files" },
        files.join(", ")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all(paths: &[&str], ignored: &[&str], prompt: &str) -> Vec<String> {
        let own = |v: &[&str]| v.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
        offending(&own(paths), &own(ignored), prompt)
    }

    #[test]
    fn each_pattern_of_the_list_is_caught() {
        for path in [
            ".github/scripts/__pycache__/wait_for_ci.cpython-315.pyc",
            "a/b.pyc",
            "apps/web/node_modules/x/index.js",
            "target/debug/app",
            "crates/x/target/a",
            "dist/app.js",
            ".DS_Store",
            "docs/.DS_Store",
            "run.log",
            "notes/out.tmp",
            "x/__pycache__/mod.py",
        ] {
            assert_eq!(all(&[path], &[], ""), vec![path.to_owned()], "{path}");
        }
    }

    #[test]
    fn ordinary_files_pass() {
        let ok = [
            "src/main.rs",
            "docs/distance.md",
            "targets.toml",
            "dist.rs",
            "src/target.rs",
            "logs.md",
            "a.pyc.md",
            "pyc",
            ".log",
        ];
        assert!(all(&ok, &[], "").is_empty());
    }

    #[test]
    fn a_file_git_says_is_ignored_is_caught_and_the_rest_are_not() {
        let got = all(&["out/a.txt", "src/b.rs"], &["out/a.txt"], "");
        assert_eq!(got, vec!["out/a.txt".to_owned()]);
    }

    #[test]
    fn a_path_the_prompt_names_passes() {
        let prompt = "Add a fixture at tests/fixtures/dist/a.txt for the packer.";
        assert!(all(&["tests/fixtures/dist/a.txt"], &[], prompt).is_empty());
        let dir = "Put the fixtures under tests/fixtures/dist/ .";
        assert!(all(&["tests/fixtures/dist/a.txt"], &[], dir).is_empty());
        let ignored = "Commit conf/local.env as the example.";
        assert!(all(&["conf/local.env"], &["conf/local.env"], ignored).is_empty());
        // Only the named path passes; another one the prompt does not name is still refused.
        let both = all(
            &["tests/fixtures/dist/a.txt", "x/__pycache__/a.pyc"],
            &[],
            prompt,
        );
        assert_eq!(both, vec!["x/__pycache__/a.pyc".to_owned()]);
    }

    #[test]
    fn a_prompt_that_only_mentions_a_pattern_names_no_path_below_another_directory() {
        let prompt = "Never commit dist/ or __pycache__/ or *.pyc files.";
        assert_eq!(
            all(&["x/__pycache__/a.pyc", "dist/b.js"], &[], prompt),
            vec!["x/__pycache__/a.pyc".to_owned()]
        );
    }

    #[test]
    fn the_refusal_names_each_file_and_says_how_to_remove_them() {
        let text = refusal(&["a/__pycache__/b.pyc".to_owned(), "c.log".to_owned()]);
        assert!(text.contains("a/__pycache__/b.pyc, c.log"), "{text}");
        assert!(
            text.contains(
                "these are built or ignored files; remove them from git (git rm --cached) and hand in again"
            ),
            "{text}"
        );
    }
}
