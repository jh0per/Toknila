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
pub(super) fn data_home_paths(run_dir: &Path) -> Vec<String> {
    let (Some(runs), Some(own)) = (run_dir.parent(), run_dir.file_name()) else {
        return Vec::new();
    };
    let Some(data_home) = runs.parent() else {
        return Vec::new();
    };
    let runs_name = runs.file_name();
    let mut out: Vec<PathBuf> = entries(data_home)
        .into_iter()
        .filter(|p| p.file_name() != runs_name)
        .collect();
    out.extend(
        entries(runs)
            .into_iter()
            .filter(|p| p.file_name() != Some(own)),
    );
    out.iter().map(|p| p.display().to_string()).collect()
}

/// The deny rules for a run whose folder is `run_dir`.
pub(super) fn deny_rules(run_dir: &Path) -> Vec<String> {
    let Some(home) = dirs::home_dir() else {
        return Vec::new();
    };
    toknila_core::private::deny_rules(&home.display().to_string(), &data_home_paths(run_dir))
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
        let paths = data_home_paths(&tmp.path().join("runs").join("t1"));
        let names: Vec<String> = paths
            .iter()
            .map(|p| {
                Path::new(p)
                    .strip_prefix(tmp.path())
                    .expect("inside")
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .collect();
        assert_eq!(names, ["db", "tokens", "runs/t2"]);
    }
}
