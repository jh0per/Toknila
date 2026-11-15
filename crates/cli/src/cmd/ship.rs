//! `tok ship`: the ships of a project's cards.

use anyhow::Result;
use clap::Subcommand;
use toknila_api_types::{Workspace, WorkspaceKind};
use toknila_client::Client;

use super::{default_project, emit};

/// `tok ship` subcommands.
#[derive(Debug, Subcommand)]
pub enum ShipCmd {
    /// List the workspaces of the project's stories with each story's status; the ones of merged
    /// and cancelled stories are marked, the assistant's reading copy is named as such.
    Ls {
        /// A project (key); the only one when none is named.
        #[arg(long)]
        project: Option<String>,
    },
    /// Delete the leftover workspaces and `ship/*` branches that no live story owns: those of
    /// merged, cancelled and deleted stories. Same as Clean up in Settings, Leftover ships.
    Prune {
        /// A project (key); the only one when none is named.
        #[arg(long)]
        project: Option<String>,
        /// Only list what would go.
        #[arg(long)]
        dry_run: bool,
    },
}

/// Runs a `tok ship` command.
pub fn run(c: &Client, cmd: ShipCmd, json_out: bool) -> Result<()> {
    let (project, dry_run) = match cmd {
        ShipCmd::Ls { project } => {
            let project = default_project(c, project)?;
            let list = c.workspaces(&project)?;
            emit(json_out, &list, |l| {
                for w in &l.items {
                    println!("{}", workspace_line(w));
                }
                println!(
                    "Workspaces: {}, of which {} belong to finished stories",
                    l.count, l.finished
                );
            });
            return Ok(());
        }
        ShipCmd::Prune { project, dry_run } => (default_project(c, project)?, dry_run),
    };
    if dry_run {
        let list = c.strays(&project)?;
        emit(json_out, &list, |l| {
            for s in &l.items {
                println!("{}", line(s));
            }
            println!("{} leftover(s)", l.items.len());
        });
        return Ok(());
    }
    let done = c.clean_strays(&project)?;
    emit(json_out, &done, |d| {
        for s in &d.removed {
            println!("removed {}", line(s));
        }
        for w in &d.warnings {
            println!("warning: {w}");
        }
        println!("{} leftover(s) removed", d.removed.len());
    });
    Ok(())
}

/// One workspace as a line: the story and its status (a finished story is marked), the branch
/// and the path; the assistant's reading copy is named as such.
fn workspace_line(w: &Workspace) -> String {
    if w.kind == WorkspaceKind::Planner {
        return format!("assistant's reading copy (not a story's)  {}", w.path);
    }
    let story = w.story.as_deref().unwrap_or("?");
    let status = w
        .status
        .map_or_else(String::new, |s| format!("{s:?}").to_lowercase());
    let mark = if w.finished { "  [finished story]" } else { "" };
    let branch = w.branch.as_deref().unwrap_or("-");
    format!("{story}  {status}  {branch}  {}{mark}", w.path)
}

/// One leftover as a line: its name, the card it belonged to and the commit that merged it.
fn line(s: &toknila_core::strays::Stray) -> String {
    let mut out = s.name.clone();
    if let Some(card) = &s.card {
        out.push_str(&format!(" ({card}"));
        if let Some(c) = &s.merge_commit {
            out.push_str(&format!(", merged in {c}"));
        }
        out.push(')');
    }
    out
}

#[cfg(test)]
mod tests {
    use toknila_core::TicketStatus;

    use super::*;

    fn story(status: TicketStatus, finished: bool) -> Workspace {
        Workspace {
            kind: WorkspaceKind::Story,
            story: Some("DEMO-1".to_owned()),
            status: Some(status),
            branch: Some("ship/auth/demo-1-login".to_owned()),
            path: "/s/demo/auth/demo-1-login".to_owned(),
            finished,
        }
    }

    #[test]
    fn a_workspace_line_names_the_status_and_marks_a_finished_story() {
        assert_eq!(
            workspace_line(&story(TicketStatus::Running, false)),
            "DEMO-1  running  ship/auth/demo-1-login  /s/demo/auth/demo-1-login"
        );
        assert_eq!(
            workspace_line(&story(TicketStatus::Merged, true)),
            "DEMO-1  merged  ship/auth/demo-1-login  /s/demo/auth/demo-1-login  [finished story]"
        );
    }

    #[test]
    fn the_planner_copy_is_named_as_such() {
        let planner = Workspace {
            kind: WorkspaceKind::Planner,
            story: None,
            status: None,
            branch: None,
            path: "/s/demo/_planner".to_owned(),
            finished: false,
        };
        assert_eq!(
            workspace_line(&planner),
            "assistant's reading copy (not a story's)  /s/demo/_planner"
        );
    }
}
