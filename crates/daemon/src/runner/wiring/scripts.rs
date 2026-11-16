//! The repository scripts a reviewer may run because the hand-in names them.

use std::path::Path;
use std::process::{Command, Stdio};

use toknila_core::{AgentRole, RepoTool};

use super::Context;

/// Whether `path` is a file of the ship `repo` that git tracks.
fn tracked(repo: &Path, path: &str) -> bool {
    repo.join(path).is_file()
        && Command::new("git")
            .args(["ls-files", "--error-unmatch", "--", path])
            .current_dir(repo)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
}

/// The exact commands of the hand-in's Checks lines that run a tracked file of the ship; none for
/// a role that is not a reviewer.
pub(super) fn of_hand_in(cx: &Context<'_>) -> Vec<String> {
    match (cx.role, cx.brief.hand_in.as_deref()) {
        (AgentRole::Reviewer, Some(hand_in)) => {
            toknila_core::hand_in_scripts(hand_in, &|p| tracked(cx.repo, p))
        }
        _ => Vec::new(),
    }
}

/// What the reviewer's instructions list: its toolchains' commands, then `scripts`.
pub(super) fn review_commands(
    role: AgentRole,
    tools: &[RepoTool],
    scripts: Vec<String>,
) -> Vec<String> {
    match role {
        AgentRole::Reviewer => {
            let mut out = toknila_core::reviewer_commands(tools);
            out.extend(scripts);
            out
        }
        _ => Vec::new(),
    }
}
