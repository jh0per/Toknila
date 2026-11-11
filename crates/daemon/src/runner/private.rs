//! The deny rules that keep a run's agent out of the person's private files (see
//! `toknila_core::private`): their private home folders and the Toknila data home, except the
//! run's own folder.

use std::path::{Path, PathBuf};

/// The entries of `dir`, as sorted paths; none when it cannot be read.
fn entries(dir: &Path) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|rd| rd.filter_map(|e| e.ok().map(|e| e.path())).collect())
        .unwrap_or_default();
    out.sort();
    out
}

/// The data-home paths to deny: every entry of the data home (`run_dir` is
/// `<data home>/runs/<ticket>`) except `runs`, and every entry of `runs` except the run's own
/// folder. A run folder that is not two levels below a data home denies nothing here.
///
/// The files of `keep` (design files the card agent may read) are the one exception: a folder
/// that holds one is denied entry by entry, so that the deny rules leave exactly those files out
/// (a deny rule would win over the allow rule for the file).
pub(super) fn data_home_paths(run_dir: &Path, keep: &[PathBuf]) -> Vec<String> {
    let (Some(runs), Some(own)) = (run_dir.parent(), run_dir.file_name()) else {
        return Vec::new();
    };
    let Some(data_home) = runs.parent() else {
        return Vec::new();
    };
    let runs_name = runs.file_name();
    let mut out: Vec<PathBuf> = Vec::new();
    for p in entries(data_home)
        .into_iter()
        .filter(|p| p.file_name() != runs_name)
    {
        deny_except(&p, keep, &mut out);
    }
    out.extend(
        entries(runs)
            .into_iter()
            .filter(|p| p.file_name() != Some(own)),
    );
    out.iter().map(|p| p.display().to_string()).collect()
}

/// Adds `path` to `out`, except the files of `keep`; a folder that holds one of them is
/// replaced by its entries, recursively.
fn deny_except(path: &Path, keep: &[PathBuf], out: &mut Vec<PathBuf>) {
    if keep.iter().any(|k| k == path) {
        return;
    }
    if keep.iter().any(|k| k.starts_with(path)) {
        for e in entries(path) {
            deny_except(&e, keep, out);
        }
    } else {
        out.push(path.to_owned());
    }
}

/// The deny rules for a run whose folder is `run_dir`, leaving the files of `keep` readable.
pub(super) fn deny_rules(run_dir: &Path, keep: &[PathBuf]) -> Vec<String> {
    let Some(home) = dirs::home_dir() else {
        return Vec::new();
    };
    toknila_core::private::deny_rules(&home.display().to_string(), &data_home_paths(run_dir, keep))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_own_run_folder_stays_open_and_everything_else_in_the_data_home_is_denied() {
        let tmp = tempfile::tempdir().expect("tempdir");
        for d in ["runs/t1", "runs/t2", "tokens", "db"] {
            std::fs::create_dir_all(tmp.path().join(d)).expect("mkdir");
        }
        let paths = data_home_paths(&tmp.path().join("runs").join("t1"), &[]);
        assert_eq!(names_in(tmp.path(), &paths), ["db", "tokens", "runs/t2"]);
    }

    fn names_in(root: &Path, paths: &[String]) -> Vec<String> {
        paths
            .iter()
            .map(|p| {
                Path::new(p)
                    .strip_prefix(root)
                    .expect("inside")
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .collect()
    }

    #[test]
    fn a_kept_design_file_is_the_only_thing_left_out_of_the_designs_folder() {
        let tmp = tempfile::tempdir().expect("tempdir");
        for f in [
            "designs/p1/a.png",
            "designs/p1/b.png",
            "designs/p1/index.json",
            "designs/p2/c.png",
            "tokens/x",
        ] {
            let path = tmp.path().join(f);
            std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
            std::fs::write(path, "x").expect("write");
        }
        std::fs::create_dir_all(tmp.path().join("runs/t1")).expect("mkdir");
        let keep = [tmp.path().join("designs/p1/a.png")];
        let paths = data_home_paths(&tmp.path().join("runs").join("t1"), &keep);
        // Everything else of the library stays denied, the file itself is not.
        assert_eq!(
            names_in(tmp.path(), &paths),
            [
                "designs/p1/b.png",
                "designs/p1/index.json",
                "designs/p2",
                "tokens"
            ]
        );
    }
}
