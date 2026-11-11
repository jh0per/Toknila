//! What changed on a ship between a Return and the hand-in that answers it. Pure data: the host
//! reads git, core caps and words it.

use serde::{Deserialize, Serialize};

/// Most commit subjects a [`SinceReturn`] keeps.
pub const MAX_COMMITS: usize = 15;
/// Most changed files a [`SinceReturn`] keeps.
pub const MAX_FILES: usize = 30;

/// The work since the Return, derived by the daemon from the returned HEAD to the new one and
/// kept with the hand-in.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SinceReturn {
    /// The round that was returned.
    pub round: u32,
    /// The commit that was handed in and returned.
    pub from: String,
    /// The commit that is handed in now.
    pub to: String,
    /// Subjects of the commits made since, newest first, cut at [`MAX_COMMITS`].
    pub commits: Vec<String>,
    /// How many more commits there were than `commits` shows.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub more_commits: u32,
    /// Files changed since, cut at [`MAX_FILES`].
    pub files: Vec<String>,
    /// How many more files changed than `files` shows.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub more_files: u32,
}

fn is_zero(n: &u32) -> bool {
    *n == 0
}

fn cut(mut all: Vec<String>, max: usize) -> (Vec<String>, u32) {
    let more = u32::try_from(all.len().saturating_sub(max)).unwrap_or(u32::MAX);
    all.truncate(max);
    (all, more)
}

impl SinceReturn {
    /// The block for a hand-in at `to` after round `round` was returned at `from`; the lists are
    /// capped.
    pub fn new(round: u32, from: &str, to: &str, commits: Vec<String>, files: Vec<String>) -> Self {
        let (commits, more_commits) = cut(commits, MAX_COMMITS);
        let (files, more_files) = cut(files, MAX_FILES);
        Self {
            round,
            from: from.to_owned(),
            to: to.to_owned(),
            commits,
            more_commits,
            files,
            more_files,
        }
    }

    /// The block as plain text: a heading, the commits, the files.
    pub fn text(&self) -> String {
        let short = |c: &str| c.get(..7).unwrap_or(c).to_owned();
        let mut out = format!(
            "Since the Return (round {}, {}..{}):\n",
            self.round,
            short(&self.from),
            short(&self.to)
        );
        if self.commits.is_empty() {
            out.push_str("  no commits\n");
        }
        for c in &self.commits {
            out.push_str(&format!("  - {c}\n"));
        }
        if self.more_commits > 0 {
            out.push_str(&format!("  ... and {} more commits\n", self.more_commits));
        }
        if !self.files.is_empty() {
            out.push_str(&format!("Changed files: {}", self.files.join(", ")));
            if self.more_files > 0 {
                out.push_str(&format!(" and {} more", self.more_files));
            }
            out.push('\n');
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_lists_are_capped_and_the_rest_is_counted() {
        let commits: Vec<String> = (0..20).map(|i| format!("c{i}")).collect();
        let files: Vec<String> = (0..35).map(|i| format!("f{i}.rs")).collect();
        let s = SinceReturn::new(2, "aaaaaaaaaa", "bbbbbbbbbb", commits, files);
        assert_eq!((s.commits.len(), s.more_commits), (15, 5));
        assert_eq!((s.files.len(), s.more_files), (30, 5));
        let text = s.text();
        for want in [
            "round 2",
            "aaaaaaa..bbbbbbb",
            "- c0",
            "and 5 more commits",
            "and 5 more",
        ] {
            assert!(text.contains(want), "{want}: {text}");
        }
        assert!(!text.contains("c15"), "{text}");
    }
}
