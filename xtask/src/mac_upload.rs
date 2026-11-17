//! The Mac files go to the release on GitHub through `gh`: a draft is created when the tag has
//! no release yet, and a published release is never touched (its `latest.json` carries the
//! signatures of the archives it has). CI publishes the draft once it has added its own files and
//! all six Mac files are there, so the `.sig` files go last: on a draft that already has them, the
//! old ones are deleted first, and the set stays incomplete until the new signatures are up.

use std::path::{Path, PathBuf};

use crate::runner::{Cmd, Exec, check};
use crate::updates::REPO;

/// What GitHub has for the tag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Release {
    Missing,
    /// A draft, with the names of its files.
    Draft(Vec<String>),
    Published,
}

/// The last line `gh` printed on its error output, which says why.
fn why(err: &str) -> &str {
    err.lines()
        .rev()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("no answer")
}

/// Fails, in seconds and before any build, when the tag is not on GitHub: the draft is created
/// with `--verify-tag`, which would refuse it only after both builds and both notarizations.
pub fn tag_on_github(exec: &mut dyn Exec, dir: &Path, tag: &str) -> Result<(), String> {
    let path = format!("repos/{REPO}/git/ref/tags/{tag}");
    let ran = exec.run(&Cmd::new("gh", dir).args(["api", &path, "--jq", ".ref"]))?;
    if ran.ok && ran.out.trim() == format!("refs/tags/{tag}") {
        return Ok(());
    }
    if ran.err.contains("HTTP 404") {
        return Err(format!(
            "the tag {tag} is not on GitHub: push it (git push origin {tag}), then run the Build step again"
        ));
    }
    let why = if ran.ok {
        format!("it answered {:?}", ran.out.trim())
    } else {
        why(&ran.err).to_owned()
    };
    Err(format!(
        "gh could not look up the tag {tag} on GitHub: {why}"
    ))
}

/// Asks GitHub whether the tag has a release, whether it is still a draft, and its files.
pub fn state(exec: &mut dyn Exec, dir: &Path, tag: &str) -> Result<Release, String> {
    let view = Cmd::new("gh", dir).args([
        "release",
        "view",
        tag,
        "--repo",
        REPO,
        "--json",
        "isDraft,assets",
        "--jq",
        ".isDraft, .assets[].name",
    ]);
    let ran = exec.run(&view)?;
    if !ran.ok {
        if ran.err.contains("release not found") {
            return Ok(Release::Missing);
        }
        return Err(format!(
            "gh could not read the release {tag}: {}",
            why(&ran.err)
        ));
    }
    let mut lines = ran.out.lines().map(str::trim).filter(|l| !l.is_empty());
    match lines.next() {
        Some("true") => Ok(Release::Draft(lines.map(str::to_owned).collect())),
        Some("false") => Ok(Release::Published),
        other => Err(format!(
            "gh answered {:?} for the draft state of {tag}",
            other.unwrap_or("")
        )),
    }
}

/// The refusal for a release that is already published.
pub fn published(tag: &str) -> String {
    format!(
        "the release {tag} is already published; mac-release does not replace its files, because its latest.json carries the signatures of the archives it has"
    )
}

fn name_of(p: &Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Uploads `files` to the tag's release, creating it as a draft first when it does not exist:
/// the dmg and the archives first, the `.sig` files last.
pub fn upload(exec: &mut dyn Exec, dir: &Path, tag: &str, files: &[PathBuf]) -> Result<(), String> {
    let (sigs, rest): (Vec<&PathBuf>, Vec<&PathBuf>) =
        files.iter().partition(|f| name_of(f).ends_with(".sig"));
    match state(exec, dir, tag)? {
        Release::Published => return Err(published(tag)),
        Release::Draft(there) => {
            // A rerun: without this, CI could see the new archives beside the old signatures.
            for sig in sigs.iter().map(|s| name_of(s)) {
                if there.contains(&sig) {
                    let delete = Cmd::new("gh", dir).args([
                        "release",
                        "delete-asset",
                        tag,
                        &sig,
                        "--repo",
                        REPO,
                        "--yes",
                    ]);
                    check(exec, &delete)?;
                }
            }
        }
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
    for set in [rest, sigs] {
        let mut cmd =
            Cmd::new("gh", dir).args(["release", "upload", tag, "--repo", REPO, "--clobber"]);
        for f in set {
            cmd = cmd.path(f);
        }
        check(exec, &cmd)?;
    }
    Ok(())
}
