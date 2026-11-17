//! The Mac files go to the release on GitHub through `gh`: a draft is created when the tag has
//! no release yet, and a published release is never touched (its `latest.json` carries the
//! signatures of the archives it has). CI publishes the draft once it has added its own files.

use std::path::{Path, PathBuf};

use crate::runner::{Cmd, Exec, check};
use crate::updates::REPO;

/// What GitHub has for the tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Release {
    Missing,
    Draft,
    Published,
}

/// Asks GitHub whether the tag has a release and whether it is still a draft.
pub fn state(exec: &mut dyn Exec, dir: &Path, tag: &str) -> Result<Release, String> {
    let view = Cmd::new("gh", dir).args([
        "release", "view", tag, "--repo", REPO, "--json", "isDraft", "--jq", ".isDraft",
    ]);
    let ran = exec.run(&view)?;
    if !ran.ok {
        if ran.err.contains("release not found") {
            return Ok(Release::Missing);
        }
        let why = ran
            .err
            .lines()
            .next_back()
            .unwrap_or("no answer")
            .trim()
            .to_owned();
        return Err(format!("gh could not read the release {tag}: {why}"));
    }
    match ran.out.trim() {
        "true" => Ok(Release::Draft),
        "false" => Ok(Release::Published),
        other => Err(format!(
            "gh answered {other:?} for the draft state of {tag}"
        )),
    }
}

/// The refusal for a release that is already published.
pub fn published(tag: &str) -> String {
    format!(
        "the release {tag} is already published; mac-release does not replace its files, because its latest.json carries the signatures of the archives it has"
    )
}

/// Uploads `files` to the tag's release, creating it as a draft first when it does not exist.
pub fn upload(exec: &mut dyn Exec, dir: &Path, tag: &str, files: &[PathBuf]) -> Result<(), String> {
    match state(exec, dir, tag)? {
        Release::Published => return Err(published(tag)),
        Release::Draft => {}
        Release::Missing => {
            let title = format!("Toknila {tag}");
            let create = Cmd::new("gh", dir).args([
                "release",
                "create",
                tag,
                "--repo",
                REPO,
                "--draft",
                "--verify-tag",
                "--title",
                &title,
                "--notes",
                "The Mac files are uploaded; the Release workflow adds the rest and publishes.",
            ]);
            check(exec, &create)?;
        }
    }
    let mut cmd = Cmd::new("gh", dir).args(["release", "upload", tag, "--repo", REPO, "--clobber"]);
    for f in files {
        cmd = cmd.path(f);
    }
    check(exec, &cmd)?;
    Ok(())
}
