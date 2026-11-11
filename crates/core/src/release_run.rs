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
    /// The base branch and the tag to `origin`; only for projects that merge by pull request.
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
/// them, the tag always, the push only when the project merges by pull request.
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

/// The bump command with the version put in (`{version}`).
pub fn bump_command(template: &str, version: &str) -> String {
    template.replace("{version}", version)
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn the_tag_and_the_bump_command_carry_the_version() {
        assert_eq!(tag_name("1.4.0"), "v1.4.0");
        assert_eq!(
            bump_command("npm version {version}", "1.4.0"),
            "npm version 1.4.0"
        );
    }
}
