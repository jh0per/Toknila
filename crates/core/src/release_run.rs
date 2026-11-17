//! The steps of a release run and the rules around them: which steps a project has, the version a
//! person may pick, the one Toknila suggests from the tags, and the tag name. Pure: the daemon
//! runs the steps with `git` and the project's commands.

use serde::{Deserialize, Serialize};

/// One step of a release run, in the order it runs.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseStep {
    /// The project's check command, on the base branch.
    Check,
    /// The project's bump command, then a commit of what it changed.
    Bump,
    /// An annotated tag on the release commit.
    Tag,
    /// The base branch and the tag to the repository's remote; a choice of the release.
    Push,
}

impl ReleaseStep {
    /// The step as a person reads it.
    pub fn label(self) -> &'static str {
        match self {
            Self::Check => "Check",
            Self::Bump => "Bump the version",
            Self::Tag => "Tag",
            Self::Push => "Push",
        }
    }
}

/// The steps a release of a project runs: the check and the bump only when the project names
/// them, the tag always, the push only when the release asks for it.
pub fn steps(check: Option<&str>, bump: Option<&str>, push: bool) -> Vec<ReleaseStep> {
    let mut steps = Vec::new();
    if check.is_some_and(|c| !c.trim().is_empty()) {
        steps.push(ReleaseStep::Check);
    }
    if bump.is_some_and(|b| !b.trim().is_empty()) {
        steps.push(ReleaseStep::Bump);
    }
    steps.push(ReleaseStep::Tag);
    if push {
        steps.push(ReleaseStep::Push);
    }
    steps
}

/// Whether a run with `steps`, of which `passed` passed, is still in its project's merge line: it
/// is until its Tag, and its Push when it has one, passed (a failed step ends the run).
pub fn in_merge_line(steps: &[ReleaseStep], passed: &[ReleaseStep]) -> bool {
    steps
        .iter()
        .filter(|s| matches!(s, ReleaseStep::Tag | ReleaseStep::Push))
        .any(|s| !passed.contains(s))
}

/// What the Check step of a release says while it waits in the machine's check line: `place` is
/// its place among those that wait (1 is next), `held` when the machine's load keeps it.
pub fn check_line_words(place: u32, held: Option<crate::LoadHold>) -> String {
    match held {
        Some(held) => held.words(),
        None => format!("waits for its turn (#{place} in the check line)"),
    }
}

/// The version as a person typed it, without a leading `v`, or why it is not one. A version is
/// numbers separated by dots, with an optional suffix of letters, digits, dots and dashes
/// (`1.4.0`, `2.0.0-rc.1`): it becomes part of a tag and goes into the bump command.
pub fn clean_version(text: &str) -> Result<String, &'static str> {
    const BAD: &str = "a version is numbers separated by dots, such as 1.4.0";
    let text = text.trim();
    let text = text.strip_prefix('v').unwrap_or(text);
    let (core, suffix) = match text.split_once('-') {
        Some((core, suffix)) => (core, Some(suffix)),
        None => (text, None),
    };
    let numbers = !core.is_empty()
        && core
            .split('.')
            .all(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()));
    let suffix_ok = suffix.is_none_or(|s| {
        !s.is_empty()
            && s.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
    });
    if numbers && suffix_ok {
        Ok(text.to_owned())
    } else {
        Err(BAD)
    }
}

/// The steps a run takes again after it was interrupted, given the steps that had passed. A
/// passed step is not redone, except the bump while the tag has not passed: its commit lives only
/// in the run's throwaway worktree, and the tag step is what moves the base branch to it.
pub fn resume_steps(all: &[ReleaseStep], passed: &[ReleaseStep]) -> Vec<ReleaseStep> {
    let tagged = passed.contains(&ReleaseStep::Tag);
    all.iter()
        .copied()
        .filter(|s| !passed.contains(s) || (*s == ReleaseStep::Bump && !tagged))
        .collect()
}

/// Whether the subject of a merge commit on the base branch is the merge of the story `key` of
/// the epic named `chain`: a local merge says `Merge ship/<chain>/<key>-<title>: KEY Title`, a
/// pull request's merge names the same branch (`Merge pull request #4 from o/ship/<chain>/...`).
pub fn merges_story(subject: &str, chain: &str, key: &str) -> bool {
    let branch = format!("ship/{}/{}", crate::slugify(chain), crate::slugify(key));
    let on_branch = subject.match_indices(&branch).any(|(at, _)| {
        subject[at + branch.len()..]
            .chars()
            .next()
            .is_none_or(|c| matches!(c, '-' | ':' | ' ' | '/'))
    });
    on_branch || subject.contains(&format!(": {key} "))
}

/// The tag a version gets: `v1.4.0`.
pub fn tag_name(version: &str) -> String {
    format!("v{version}")
}

/// The version to suggest after the last of `tags` (newest first): its patch number plus one, or
/// `0.1.0` when no tag reads as a version.
pub fn suggest_version<S: AsRef<str>>(tags: &[S]) -> String {
    for tag in tags {
        let Ok(version) = clean_version(tag.as_ref()) else {
            continue;
        };
        if version.contains('-') {
            continue;
        }
        let mut parts: Vec<u64> = version.split('.').map(|n| n.parse().unwrap_or(0)).collect();
        while parts.len() < 3 {
            parts.push(0);
        }
        parts[2] += 1;
        return parts
            .iter()
            .map(u64::to_string)
            .collect::<Vec<_>>()
            .join(".");
    }
    "0.1.0".to_owned()
}

/// The page of a tag on GitHub (`<url>/releases/tag/v0.3.4`) when `remote_url` is a GitHub
/// remote, in any of its spellings (https, `git@github.com:o/r.git`, `ssh://`).
pub fn tag_page(remote_url: &str, tag: &str) -> Option<String> {
    let url = remote_url.trim();
    let rest = ["https://", "http://", "ssh://", "git://"]
        .iter()
        .find_map(|p| url.strip_prefix(p))
        .map(|r| r.rsplit_once('@').map_or(r, |(_, host)| host))
        .map(|r| r.replacen(':', "/", 1))
        .or_else(|| url.split_once('@').map(|(_, r)| r.replacen(':', "/", 1)))?;
    let path = rest.strip_prefix("github.com/")?;
    let path = path.trim_end_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path);
    let (owner, repo) = path.split_once('/')?;
    if owner.is_empty() || repo.is_empty() || repo.contains('/') {
        return None;
    }
    Some(format!(
        "https://github.com/{owner}/{repo}/releases/tag/{tag}"
    ))
}

/// What the Push step says when it passed: `Pushed main and v0.3.4 to origin`, then the tag's
/// page on its own line when the remote is on GitHub, then the line that says GitHub waits for CI.
pub fn pushed_text(base: &str, tag: &str, remote: &str, remote_url: &str, short: &str) -> String {
    let line = format!("Pushed {base} and {tag} to {remote}");
    let line = match tag_page(remote_url, tag) {
        Some(page) => format!("{line}\n{page}"),
        None => line,
    };
    format!("{line}\nGitHub publishes after CI passes on {short}")
}

/// The line a person reads when a push failed: git's own reason for a refused ref
/// (`rejected: non-fast-forward; pull first`), else git's last line as it is.
pub fn push_failure_line(git_output: &str) -> String {
    for line in git_output.lines() {
        if line.contains("[rejected]")
            && let Some(reason) = line.rsplit_once('(').and_then(|(_, r)| r.strip_suffix(')'))
        {
            return format!("rejected: {reason}; pull first");
        }
    }
    git_output
        .lines()
        .rev()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("git printed nothing")
        .trim()
        .to_owned()
}

/// The bump command with the version put in (`{version}`).
pub fn bump_command(template: &str, version: &str) -> String {
    template.replace("{version}", version)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_merge_subject_names_the_story_it_brought_in() {
        let local = "Merge ship/phase-9/toknila-14-add-login: TOKNILA-14 Add login";
        assert!(merges_story(local, "phase-9", "TOKNILA-14"));
        assert!(!merges_story(local, "phase-9", "TOKNILA-1"), "not a prefix");
        assert!(!merges_story(local, "phase-9", "TOKNILA-144"));
        assert!(!merges_story(local, "phase-9", "TOKNILA-15"), "other story");
        let pr = "Merge pull request #4 from me/ship/phase-9/toknila-14-add-login";
        assert!(merges_story(pr, "phase-9", "TOKNILA-14"));
        let renamed = "Merge ship/old-name/toknila-14-add-login: TOKNILA-14 Add login";
        assert!(merges_story(renamed, "phase-9", "TOKNILA-14"), "by key");
    }

    #[test]
    fn a_run_holds_the_merge_line_until_its_tag_and_push_passed() {
        use ReleaseStep::*;
        assert!(in_merge_line(&[Check, Tag], &[]));
        assert!(in_merge_line(&[Check, Tag], &[Check]));
        assert!(!in_merge_line(&[Check, Tag], &[Check, Tag]));
        assert!(
            in_merge_line(&[Tag, Push], &[Tag]),
            "the push is still to go"
        );
        assert!(!in_merge_line(&[Tag, Push], &[Tag, Push]));
    }

    #[test]
    fn a_waiting_release_check_says_its_place_in_the_check_line() {
        assert_eq!(
            check_line_words(2, None),
            "waits for its turn (#2 in the check line)"
        );
        let held = crate::LoadHold {
            load: 41,
            cores: 16,
        };
        assert_eq!(
            check_line_words(1, Some(held)),
            "waits for the machine: load 41 on 16 cores"
        );
    }

    #[test]
    fn the_steps_follow_what_the_project_has() {
        use ReleaseStep::*;
        assert_eq!(steps(None, None, false), [Tag]);
        assert_eq!(steps(Some("make test"), None, false), [Check, Tag]);
        assert_eq!(
            steps(Some("make test"), Some("./bump {version}"), true),
            [Check, Bump, Tag, Push]
        );
        assert_eq!(steps(Some("  "), Some(""), true), [Tag, Push]);
    }

    #[test]
    fn push_is_a_step_of_a_local_mode_release_when_asked_for() {
        use ReleaseStep::*;
        // The merge mode is not an input: only the choice of the release is.
        assert_eq!(
            steps(Some("make"), Some("./bump"), true),
            [Check, Bump, Tag, Push]
        );
        assert_eq!(
            steps(Some("make"), Some("./bump"), false),
            [Check, Bump, Tag]
        );
    }

    #[test]
    fn the_tag_page_is_a_link_for_every_spelling_of_a_github_remote() {
        let page = Some("https://github.com/acme/toknila/releases/tag/v0.3.4".to_owned());
        for url in [
            "https://github.com/acme/toknila.git",
            "https://github.com/acme/toknila",
            "git@github.com:acme/toknila.git",
            "ssh://git@github.com/acme/toknila.git",
            "https://user:token@github.com/acme/toknila.git",
        ] {
            assert_eq!(tag_page(url, "v0.3.4"), page, "{url}");
        }
        for url in [
            "/tmp/remote.git",
            "https://gitlab.com/acme/toknila.git",
            "https://github.com/acme",
        ] {
            assert_eq!(tag_page(url, "v0.3.4"), None, "{url}");
        }
    }

    #[test]
    fn the_push_says_what_it_pushed_and_what_git_refused() {
        assert_eq!(
            pushed_text(
                "main",
                "v0.3.4",
                "origin",
                "git@github.com:acme/toknila.git",
                "a1b2c3d"
            ),
            "Pushed main and v0.3.4 to origin\nhttps://github.com/acme/toknila/releases/tag/v0.3.4\nGitHub publishes after CI passes on a1b2c3d"
        );
        assert_eq!(
            pushed_text("main", "v0.3.4", "origin", "/srv/r.git", "a1b2c3d"),
            "Pushed main and v0.3.4 to origin\nGitHub publishes after CI passes on a1b2c3d"
        );
        let refused = "To /srv/r.git\n!\trefs/heads/main:refs/heads/main\t[rejected] (non-fast-forward)\nDone\nerror: failed to push some refs";
        assert_eq!(
            push_failure_line(refused),
            "rejected: non-fast-forward; pull first"
        );
        assert_eq!(
            push_failure_line("fatal: unable to access 'x': Could not resolve host\n"),
            "fatal: unable to access 'x': Could not resolve host"
        );
    }

    #[test]
    fn a_version_is_numbers_with_an_optional_suffix() {
        assert_eq!(clean_version(" v1.4.0 "), Ok("1.4.0".to_owned()));
        assert_eq!(clean_version("2.0.0-rc.1"), Ok("2.0.0-rc.1".to_owned()));
        for bad in ["", "v", "1..2", "1.x", "1.0; rm -rf", "1.0-", "../1", "1 0"] {
            assert!(clean_version(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn the_suggestion_is_the_next_patch_of_the_newest_tag_that_reads_as_a_version() {
        assert_eq!(suggest_version::<&str>(&[]), "0.1.0");
        assert_eq!(suggest_version(&["v1.4.2", "v1.4.1"]), "1.4.3");
        assert_eq!(suggest_version(&["nightly", "2.0", "v1.9.9"]), "2.0.1");
        assert_eq!(suggest_version(&["v3.0.0-rc.1", "v2.9.9"]), "2.9.10");
        assert_eq!(suggest_version(&["latest"]), "0.1.0");
    }

    #[test]
    fn an_interrupted_run_redoes_what_did_not_pass_and_never_the_tag() {
        use ReleaseStep::*;
        let all = [Check, Bump, Tag, Push];
        assert_eq!(resume_steps(&all, &[]), all);
        // The check passed; the bump's commit was only in the worktree: bump again.
        assert_eq!(resume_steps(&all, &[Check, Bump]), [Bump, Tag, Push]);
        // Tagged: the tag is on the base branch and is not redone.
        assert_eq!(resume_steps(&all, &[Check, Bump, Tag]), [Push]);
        assert_eq!(resume_steps(&all, &all), Vec::<ReleaseStep>::new());
    }

    #[test]
    fn the_tag_and_the_bump_command_carry_the_version() {
        assert_eq!(tag_name("1.4.0"), "v1.4.0");
        assert_eq!(
            bump_command("npm version {version}", "1.4.0"),
            "npm version 1.4.0"
        );
    }
}
