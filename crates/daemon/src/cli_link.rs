//! The `tok` command for the person: a symlink `~/.local/bin/tok` to the `tok` that ships next to
//! the daemon (inside `Toknila.app` on a Mac). Only the user's own home is written, never with
//! elevated rights, and only a link of ours is replaced.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use toknila_api_types::CliLink;

/// Name of the command.
const NAME: &str = if cfg!(windows) { "tok.exe" } else { "tok" };

/// Where the link goes under `home`.
fn link_path(home: &Path) -> PathBuf {
    home.join(".local").join("bin").join(NAME)
}

/// The `tok` next to the daemon binary `exe`, if there is one.
fn bundled(exe: &Path) -> Option<PathBuf> {
    let tok = exe.parent()?.join(NAME);
    (tok.is_file() && tok != exe).then_some(tok)
}

/// Whether `dir` is one of the folders in `path_env`.
fn on_path(dir: &Path, path_env: Option<&OsStr>) -> bool {
    path_env.is_some_and(|p| std::env::split_paths(p).any(|d| d == dir))
}

/// What there is and what is done, given the daemon binary, the person's home and `PATH`.
pub fn status(exe: &Path, home: Option<&Path>, path_env: Option<&OsStr>) -> CliLink {
    let bundled = bundled(exe);
    let link = home.map(link_path);
    let installed = match (&bundled, &link) {
        (Some(b), Some(l)) => std::fs::read_link(l).is_ok_and(|t| &t == b),
        _ => false,
    };
    let on_path = link
        .as_deref()
        .and_then(Path::parent)
        .is_some_and(|d| on_path(d, path_env));
    CliLink {
        version: toknila_core::VERSION.to_owned(),
        bundled: bundled.map(|p| p.display().to_string()),
        link: link.map(|p| p.display().to_string()),
        installed,
        on_path,
    }
}

/// Links `~/.local/bin/tok` to the bundled `tok`. A link of ours that points elsewhere (an older
/// copy of the app) is replaced; a file or a foreign link already there is refused.
pub fn install(
    exe: &Path,
    home: Option<&Path>,
    path_env: Option<&OsStr>,
) -> Result<CliLink, String> {
    let home = home.ok_or("the home folder is unknown")?;
    let target = bundled(exe).ok_or("there is no `tok` next to this daemon to link")?;
    let link = link_path(home);
    std::fs::create_dir_all(link.parent().ok_or("bad link path")?)
        .map_err(|e| format!("cannot create {}: {e}", link.display()))?;
    match std::fs::symlink_metadata(&link) {
        Ok(m) if m.file_type().is_symlink() => {
            let old = std::fs::read_link(&link).unwrap_or_default();
            if old == target {
                return Ok(status(exe, Some(home), path_env));
            }
            if !is_ours(&old) {
                return Err(format!(
                    "{} already links to {}, which is not a Toknila command; remove it first",
                    link.display(),
                    old.display()
                ));
            }
            std::fs::remove_file(&link)
                .map_err(|e| format!("cannot replace {}: {e}", link.display()))?;
        }
        Ok(_) => {
            return Err(format!(
                "{} exists and is not a link; move it away first",
                link.display()
            ));
        }
        Err(_) => {}
    }
    symlink(&target, &link).map_err(|e| format!("cannot link {}: {e}", link.display()))?;
    Ok(status(exe, Some(home), path_env))
}

/// A link target that looks like the `tok` of a Toknila app or build: a `tok` next to a `toknila`.
fn is_ours(target: &Path) -> bool {
    target.parent().is_some_and(|d| {
        d.join(if cfg!(windows) {
            "toknila.exe"
        } else {
            "toknila"
        })
        .exists()
    }) || !target.exists() && target.to_string_lossy().contains("Toknila.app")
}

#[cfg(unix)]
fn symlink(target: &Path, link: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[cfg(windows)]
fn symlink(target: &Path, link: &Path) -> std::io::Result<()> {
    std::os::windows::fs::symlink_file(target, link)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fake app folder with `toknila` and `tok`, and a fake home.
    fn setup() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let dir = tempfile::tempdir().expect("temp dir");
        let app = dir.path().join("My Apps/Toknila.app/Contents/MacOS");
        std::fs::create_dir_all(&app).expect("app dir");
        for f in ["toknila", "tok"] {
            std::fs::write(app.join(NAME.replace("tok", f)), "x").expect("binary");
        }
        let home = dir.path().join("home");
        std::fs::create_dir_all(&home).expect("home");
        (dir, app.join(NAME.replace("tok", "toknila")), home)
    }

    #[cfg(unix)]
    #[test]
    fn installs_a_link_to_the_bundled_command_and_reports_path() {
        let (_d, exe, home) = setup();
        let before = status(&exe, Some(&home), None);
        assert!(!before.installed && !before.on_path);
        assert!(
            before
                .bundled
                .as_deref()
                .is_some_and(|b| b.ends_with("MacOS/tok"))
        );

        let bin = home.join(".local/bin");
        let path = std::env::join_paths([Path::new("/usr/bin"), bin.as_path()]).expect("path");
        let done = install(&exe, Some(&home), Some(&path)).expect("install");
        assert!(done.installed && done.on_path);
        assert_eq!(
            std::fs::read_link(bin.join("tok")).expect("link"),
            exe.parent().expect("dir").join("tok")
        );
        // Twice is fine.
        assert!(install(&exe, Some(&home), None).expect("again").installed);
        // Without the folder on PATH the answer says so.
        assert!(!status(&exe, Some(&home), Some(OsStr::new("/usr/bin"))).on_path);
    }

    #[cfg(unix)]
    #[test]
    fn replaces_the_link_of_an_older_app_but_not_a_file_or_a_foreign_link() {
        let (d, exe, home) = setup();
        let bin = home.join(".local/bin");
        std::fs::create_dir_all(&bin).expect("bin");
        // An older app's link (its target is gone, but it was a Toknila.app).
        symlink(
            Path::new("/Applications/Toknila.app/Contents/MacOS/tok"),
            &bin.join("tok"),
        )
        .expect("old link");
        assert!(
            install(&exe, Some(&home), None)
                .expect("replaced")
                .installed
        );

        std::fs::remove_file(bin.join("tok")).expect("rm");
        std::fs::write(bin.join("tok"), "mine").expect("file");
        let e = install(&exe, Some(&home), None).expect_err("file stays");
        assert!(e.contains("not a link"), "{e}");
        assert_eq!(
            std::fs::read_to_string(bin.join("tok")).expect("kept"),
            "mine"
        );

        std::fs::remove_file(bin.join("tok")).expect("rm");
        let other = d.path().join("elsewhere");
        std::fs::create_dir_all(&other).expect("dir");
        std::fs::write(other.join("tok"), "x").expect("tok");
        symlink(&other.join("tok"), &bin.join("tok")).expect("foreign");
        let e = install(&exe, Some(&home), None).expect_err("foreign stays");
        assert!(e.contains("not a Toknila command"), "{e}");
    }

    #[test]
    fn nothing_to_link_without_a_sibling_tok() {
        let dir = tempfile::tempdir().expect("temp dir");
        let exe = dir.path().join("toknila");
        std::fs::write(&exe, "x").expect("exe");
        assert!(status(&exe, Some(dir.path()), None).bundled.is_none());
        let e = install(&exe, Some(dir.path()), None).expect_err("nothing");
        assert!(e.contains("no `tok`"), "{e}");
    }
}
