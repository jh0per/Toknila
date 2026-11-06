//! What the machine needs to run Toknila, judged from what the tools said.
//!
//! The host runs `git`, `claude` and `gh` (no shell, with a timeout) and hands each answer in as
//! a [`Probe`]; this module reads versions and decides what is wrong and how to fix it. It does no
//! I/O, so every verdict is testable without the tools installed.

use serde::{Deserialize, Serialize};

/// What happened when the host ran one tool.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Probe {
    /// The program is not on the path.
    Missing,
    /// It did not finish in time and was stopped.
    TimedOut,
    /// It could not be started for another reason.
    Failed(String),
    /// It ran to the end.
    Ran {
        code: Option<i32>,
        stdout: String,
        stderr: String,
    },
}

/// Which tool a check is about.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tool {
    Git,
    Claude,
    Gh,
}

/// The verdict on one tool.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckStatus {
    Ok,
    /// Not installed.
    Missing,
    /// Installed but older than needed.
    Outdated,
    /// `gh` is installed but not logged in.
    NotLoggedIn,
    /// It ran but did not behave (timeout, odd output).
    Failed,
}

/// One line of the system check.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Check {
    pub tool: Tool,
    pub status: CheckStatus,
    /// The version the tool reported, when it did.
    pub version: Option<String>,
    /// What was found, in a sentence.
    pub detail: String,
    /// What to do about it; always set, also when the check passes.
    pub hint: String,
}

/// All checks, and whether every one passed.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SystemChecks {
    pub ok: bool,
    pub checks: Vec<Check>,
}

impl SystemChecks {
    /// The checks with their overall verdict.
    pub fn new(checks: Vec<Check>) -> Self {
        Self {
            ok: checks.iter().all(|c| c.status == CheckStatus::Ok),
            checks,
        }
    }
}

/// The oldest git with `git merge-tree --write-tree`, which merging without a checkout needs.
pub const MIN_GIT: (u32, u32) = (2, 38);

/// The first `major.minor` (and the full dotted version) in `text`.
fn first_version(text: &str) -> Option<String> {
    text.split(|c: char| c.is_whitespace() || c == '(' || c == ')')
        .find(|w| {
            let mut parts = w.split('.');
            matches!((parts.next(), parts.next()), (Some(a), Some(b))
                if !a.is_empty() && !b.is_empty()
                    && a.bytes().all(|c| c.is_ascii_digit())
                    && b.bytes().next().is_some_and(|c| c.is_ascii_digit()))
        })
        .map(str::to_owned)
}

/// `(major, minor)` of the first `x.y` in `git version 2.39.5 (Apple Git-154)`.
pub fn git_version(text: &str) -> Option<(u32, u32)> {
    let v = first_version(text)?;
    let mut parts = v.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts
        .next()?
        .chars()
        .take_while(char::is_ascii_digit)
        .collect::<String>()
        .parse()
        .ok()?;
    Some((major, minor))
}

fn check(
    tool: Tool,
    status: CheckStatus,
    version: Option<String>,
    detail: String,
    hint: &str,
) -> Check {
    Check {
        tool,
        status,
        version,
        detail,
        hint: hint.to_owned(),
    }
}

/// The checks for a probe that did not produce a version: missing, hung or broken.
fn unusable(tool: Tool, name: &str, probe: &Probe, hint: &str) -> Option<Check> {
    let (status, detail) = match probe {
        Probe::Missing => (CheckStatus::Missing, format!("{name} is not installed")),
        Probe::TimedOut => (
            CheckStatus::Failed,
            format!("{name} did not answer in time"),
        ),
        Probe::Failed(e) => (CheckStatus::Failed, format!("{name} could not be run: {e}")),
        Probe::Ran { code: Some(0), .. } => return None,
        Probe::Ran { code, stderr, .. } => {
            let exit =
                code.map_or_else(|| "was stopped".to_owned(), |c| format!("exited with {c}"));
            (
                CheckStatus::Failed,
                format!("{name} {exit} {}", stderr.trim()).trim().to_owned(),
            )
        }
    };
    Some(check(tool, status, None, detail, hint))
}

fn stdout(probe: &Probe) -> &str {
    match probe {
        Probe::Ran { stdout, .. } => stdout,
        _ => "",
    }
}

const GIT_HINT: &str =
    "Install git 2.38 or newer from https://git-scm.com and make sure `git` is on the PATH.";

/// The verdict on `git --version`.
pub fn git(probe: &Probe) -> Check {
    if let Some(bad) = unusable(Tool::Git, "git", probe, GIT_HINT) {
        return bad;
    }
    let text = stdout(probe);
    let version = first_version(text);
    match (git_version(text), version) {
        (Some(v), Some(full)) if v >= MIN_GIT => check(
            Tool::Git,
            CheckStatus::Ok,
            Some(full.clone()),
            format!("git {full}"),
            GIT_HINT,
        ),
        (Some(_), Some(full)) => check(
            Tool::Git,
            CheckStatus::Outdated,
            Some(full.clone()),
            format!(
                "git {full} cannot merge without a checkout (`git merge-tree --write-tree` needs 2.38)"
            ),
            "Upgrade git to 2.38 or newer: https://git-scm.com/downloads",
        ),
        _ => check(
            Tool::Git,
            CheckStatus::Failed,
            None,
            format!("unexpected answer from git: {}", text.trim()),
            GIT_HINT,
        ),
    }
}

const CLAUDE_HINT: &str = "Install Claude Code (see https://docs.claude.com/claude-code) so `claude` is on the PATH, or use the sim profile.";

/// The verdict on `claude --version`.
pub fn claude(probe: &Probe) -> Check {
    if let Some(bad) = unusable(Tool::Claude, "claude", probe, CLAUDE_HINT) {
        return bad;
    }
    match first_version(stdout(probe)) {
        Some(v) => check(
            Tool::Claude,
            CheckStatus::Ok,
            Some(v.clone()),
            format!("claude {v}"),
            CLAUDE_HINT,
        ),
        None => check(
            Tool::Claude,
            CheckStatus::Failed,
            None,
            format!("unexpected answer from claude: {}", stdout(probe).trim()),
            CLAUDE_HINT,
        ),
    }
}

const GH_HINT: &str =
    "Install the GitHub CLI from https://cli.github.com, then run `gh auth login`.";

/// The verdict on `gh --version` and, when it is installed, `gh auth status`.
pub fn gh(version: &Probe, auth: Option<&Probe>) -> Check {
    if let Some(bad) = unusable(Tool::Gh, "gh", version, GH_HINT) {
        return bad;
    }
    let v = first_version(stdout(version));
    let detail = v
        .as_ref()
        .map_or_else(|| "gh".to_owned(), |v| format!("gh {v}"));
    let logged_in = matches!(auth, Some(Probe::Ran { code: Some(0), .. }));
    if logged_in {
        return check(
            Tool::Gh,
            CheckStatus::Ok,
            v,
            format!("{detail}, logged in"),
            GH_HINT,
        );
    }
    check(
        Tool::Gh,
        CheckStatus::NotLoggedIn,
        v,
        format!("{detail}, not logged in"),
        "Run `gh auth login` and follow the prompts.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ran(code: i32, out: &str) -> Probe {
        Probe::Ran {
            code: Some(code),
            stdout: out.to_owned(),
            stderr: String::new(),
        }
    }

    #[test]
    fn git_versions_of_every_platform_parse() {
        assert_eq!(
            git_version("git version 2.39.5 (Apple Git-154)\n"),
            Some((2, 39))
        );
        assert_eq!(git_version("git version 2.43.0.windows.1"), Some((2, 43)));
        assert_eq!(git_version("git version 2.37"), Some((2, 37)));
        assert_eq!(git_version("not git"), None);
    }

    #[test]
    fn git_new_enough_passes_and_old_git_says_what_to_upgrade_to() {
        let c = git(&ran(0, "git version 2.39.5 (Apple Git-154)\n"));
        assert_eq!(
            (c.status, c.version.as_deref()),
            (CheckStatus::Ok, Some("2.39.5"))
        );
        let c = git(&ran(0, "git version 2.37.1\n"));
        assert_eq!(c.status, CheckStatus::Outdated);
        assert!(c.hint.contains("2.38"), "{}", c.hint);
    }

    #[test]
    fn a_missing_tool_is_missing_with_an_install_hint() {
        assert_eq!(git(&Probe::Missing).status, CheckStatus::Missing);
        let c = claude(&Probe::Missing);
        assert_eq!(c.status, CheckStatus::Missing);
        assert!(c.hint.contains("claude"), "{}", c.hint);
        assert_eq!(gh(&Probe::Missing, None).status, CheckStatus::Missing);
    }

    #[test]
    fn claude_reports_its_version() {
        let c = claude(&ran(0, "2.1.319 (Claude Code)\n"));
        assert_eq!(
            (c.status, c.version.as_deref()),
            (CheckStatus::Ok, Some("2.1.319"))
        );
    }

    #[test]
    fn gh_installed_but_logged_out_says_to_log_in() {
        let v = ran(
            0,
            "gh version 2.40.0 (2023-12-12)\nhttps://github.com/cli/cli\n",
        );
        let c = gh(&v, Some(&ran(1, "")));
        assert_eq!(c.status, CheckStatus::NotLoggedIn);
        assert_eq!(c.version.as_deref(), Some("2.40.0"));
        assert!(c.hint.contains("gh auth login"), "{}", c.hint);
        assert_eq!(gh(&v, Some(&ran(0, ""))).status, CheckStatus::Ok);
    }

    #[test]
    fn a_tool_that_hangs_or_breaks_fails_the_check() {
        assert_eq!(git(&Probe::TimedOut).status, CheckStatus::Failed);
        assert_eq!(
            claude(&Probe::Failed("denied".into())).status,
            CheckStatus::Failed
        );
        assert_eq!(claude(&ran(1, "")).status, CheckStatus::Failed);
    }

    #[test]
    fn every_check_has_a_hint_and_the_overall_verdict_needs_all_ok() {
        let all = SystemChecks::new(vec![
            git(&Probe::Missing),
            claude(&ran(0, "2.1.0 (Claude Code)")),
        ]);
        assert!(all.checks.iter().all(|c| !c.hint.is_empty()));
        assert!(!all.ok);
    }
}
