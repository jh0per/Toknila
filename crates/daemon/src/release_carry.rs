//! Which finished epics a tag contains: the git half of "a tag carries every finished epic it
//! holds". An epic is in the tag when the merge commit of each of its stories is an ancestor of the
//! tagged commit; the merge commits are found by their subjects (`toknila_core::release_run`).

use toknila_app::releases::FinishedEpic;
use toknila_core::ChainId;
use toknila_core::release_run::{merges_story, tag_name};

use crate::git::Git;

/// The commit of an existing tag (`v0.3.3`, or `0.3.3` for it), or the sentence that says there
/// is none.
pub fn tag_commit(repo: &str, tag: &str) -> Result<(String, String), String> {
    let git = Git::new("git");
    let mut names = vec![tag.to_owned()];
    if !tag.starts_with('v') {
        names.push(tag_name(tag));
    }
    for name in &names {
        let spec = format!("refs/tags/{name}^{{commit}}");
        if let Ok(commit) = git.run(repo, &["rev-parse", "--verify", "--quiet", &spec])
            && !commit.is_empty()
        {
            return Ok((name.clone(), commit));
        }
    }
    Err(format!("the tag {tag} does not exist in the repository"))
}

/// The epics among `epics` whose stories were all merged into the history of `commit`.
pub fn contained(repo: &str, commit: &str, epics: &[FinishedEpic]) -> Result<Vec<ChainId>, String> {
    if epics.is_empty() {
        return Ok(Vec::new());
    }
    let log = Git::new("git")
        .run(repo, &["log", "--merges", "--format=%s", commit])
        .map_err(|e| format!("cannot read the history of {commit}: {e}"))?;
    let subjects: Vec<&str> = log.lines().collect();
    Ok(epics
        .iter()
        .filter(|e| {
            !e.keys.is_empty()
                && e.keys
                    .iter()
                    .all(|key| subjects.iter().any(|s| merges_story(s, &e.name, key)))
        })
        .map(|e| e.chain)
        .collect())
}
