//! `tok project`.

use std::io::{BufRead, IsTerminal};

use anyhow::{Result, anyhow, bail};
use clap::Subcommand;
use toknila_api_types::{DeleteProject, NewProject, PatchProject, ProjectDeletion};
use toknila_client::Client;
use toknila_core::removal::FolderFate;
use toknila_core::wire::from_wire_name;

use super::emit;
use crate::output;

#[derive(Debug, Subcommand)]
pub enum ProjectCmd {
    /// Create a project; the key prefixes ticket keys (demo -> DEMO-1).
    New {
        key: String,
        #[arg(long)]
        name: Option<String>,
        /// Folder of the project's git repository; every ticket gets its own workspace (a copy of it).
        #[arg(long)]
        repo: Option<String>,
        /// The main branch: workspaces start from it and approved work lands on it (default: the
        /// branch the repository is on, else the remote's default branch).
        #[arg(long)]
        base: Option<String>,
        /// Agent profile tickets run with unless they name one (default: sim).
        #[arg(long)]
        profile: Option<String>,
        /// The test command, optional: agents run it before they hand in (default: detected from the
        /// repository, e.g. `cargo xtask verify`, `npm test`; an empty text names none).
        #[arg(long)]
        check_command: Option<String>,
        /// What happens when a card is approved: `local` (merge into the main branch) or `github`
        /// (open a pull request); default: `github` when the repository has a GitHub remote and
        /// `gh auth status` succeeds, else `local`.
        #[arg(long)]
        merge: Option<String>,
        /// When a card's agent wants to run a command: `auto` (default; Claude Code decides and
        /// asks only about risky actions), `ask` (you answer each prompt) or `trust` (never asked).
        #[arg(long)]
        commands: Option<String>,
    },
    /// Show a project's settings, including the commands remembered with "Allow always".
    Show { key: String },
    /// Change a project's settings.
    Set {
        key: String,
        /// When a card's agent wants to run a command: `ask` (you answer each prompt), `auto`
        /// (Claude Code decides and asks only about risky actions) or `trust` (never asked).
        #[arg(long)]
        commands: Option<String>,
        /// Forget a command remembered with "Allow always", e.g. `--forget-command "Bash(cargo test *)"`
        /// (repeat for more).
        #[arg(long)]
        forget_command: Vec<String>,
        /// Most agents working at once (1 to 64, default 2); more started cards wait in a queue.
        #[arg(long)]
        parallel: Option<u32>,
        /// How many times the loop (the check, then the reviewer agent) may return a card to its
        /// worker before it waits for you (0 to 20, default 3).
        #[arg(long)]
        review_rounds: Option<u32>,
        /// What happens when a card is approved: `local` (merge into the main branch) or `github`
        /// (open a pull request, merge it there when its checks pass).
        #[arg(long)]
        merge: Option<String>,
        /// Pull request title; {key} and {title} are the card's (default: "{key}: {title}").
        #[arg(long)]
        pr_title: Option<String>,
        /// How a pull request merges: merge, squash or rebase (default: merge).
        #[arg(long)]
        merge_method: Option<String>,
        /// Seconds a pull request may report no check before you are asked what to do (merge now,
        /// keep waiting or block); default 600.
        #[arg(long)]
        no_checks_timeout: Option<u64>,
        /// The test command, optional (e.g. `cargo xtask verify`): agents run it before they hand in,
        /// and may without asking. An empty text clears it.
        #[arg(long)]
        check_command: Option<String>,
        /// The Claude model of card agents (`sonnet`, `opus`, `haiku`, or a full model name);
        /// passed as `--model` to profiles that run Claude Code. An empty text goes back to your
        /// Claude Code default.
        #[arg(long)]
        worker_model: Option<String>,
        /// The Claude model of reviewers; like `--worker-model`.
        #[arg(long)]
        reviewer_model: Option<String>,
        /// The Claude model of the planner; like `--worker-model`.
        #[arg(long)]
        planner_model: Option<String>,
    },
    /// List projects.
    Ls,
    /// Delete a project with its chains, cards and everything attached to them. The workspaces
    /// and run files go too; the project folder and the branches made for its cards stay.
    Rm {
        key: String,
        /// Also move the repository folder to the system Trash; asks you to type the project key.
        #[arg(long)]
        trash_folder: bool,
        /// Do not ask (needed with --trash-folder when stdin is not a terminal).
        #[arg(long)]
        yes: bool,
    },
}

pub(super) fn run(c: &Client, cmd: ProjectCmd, json_out: bool) -> Result<()> {
    match cmd {
        ProjectCmd::New {
            key,
            name,
            repo,
            base,
            profile,
            check_command,
            merge,
            commands,
        } => {
            let mut merge_mode = merge
                .as_deref()
                .map(|m| from_wire_name(m).ok_or_else(|| anyhow!("unknown merge mode {m}")))
                .transpose()?;
            let mut check_command = check_command;
            // What the repository suggests fills in what was not given; the check failing is
            // not an error here, the creation below says what is wrong with the folder.
            let mut detected = Vec::new();
            if let Some(path) = repo.as_deref()
                && (check_command.is_none() || merge_mode.is_none())
                && let Ok(found) = c.check_repo(path, base.as_deref())
            {
                if check_command.is_none()
                    && let Some(cmd) = found.suggested_check_command
                {
                    detected.push(format!("test command: {cmd} (detected)"));
                    check_command = Some(cmd);
                }
                if merge_mode.is_none() {
                    merge_mode = found.suggested_merge_mode;
                    if merge_mode == Some(toknila_core::MergeMode::Github) {
                        detected.push(
                            "merge: github (detected: GitHub remote, gh logged in)".to_owned(),
                        );
                    }
                }
            }
            let p = c.create_project(&NewProject {
                key: key.to_uppercase(),
                name: Some(name.unwrap_or(key)),
                repo_path: repo,
                work_branch: base,
                default_profile: profile,
                check_command,
                merge_mode,
                command_policy: commands.as_deref().map(policy).transpose()?,
                ..NewProject::default()
            })?;
            emit(json_out, &p, |p| {
                println!("project {} created", p.key);
                for line in &detected {
                    println!("{line}");
                }
            });
        }
        ProjectCmd::Show { key } => {
            let p = c.project(&key)?;
            emit(json_out, &p, |p| {
                println!("project {}: {}", p.key, p.name);
                print_commands(p);
            });
        }
        ProjectCmd::Set {
            key,
            commands,
            forget_command,
            parallel,
            review_rounds,
            merge,
            pr_title,
            merge_method,
            no_checks_timeout,
            check_command,
            worker_model,
            reviewer_model,
            planner_model,
        } => {
            let allowed_commands = if forget_command.is_empty() {
                None
            } else {
                let mut rules = c.project(&key)?.allowed_commands;
                for gone in &forget_command {
                    if !rules.iter().any(|r| r == gone) {
                        bail!("{gone:?} is not remembered by {key} (see `tok project show {key}`)");
                    }
                }
                rules.retain(|r| !forget_command.contains(r));
                Some(rules)
            };
            let patch = PatchProject {
                command_policy: commands.as_deref().map(policy).transpose()?,
                allowed_commands,
                max_parallel: parallel,
                max_review_rounds: review_rounds,
                merge_mode: merge
                    .as_deref()
                    .map(|m| from_wire_name(m).ok_or_else(|| anyhow!("unknown merge mode {m}")))
                    .transpose()?,
                pr_title,
                merge_method: merge_method
                    .as_deref()
                    .map(|m| from_wire_name(m).ok_or_else(|| anyhow!("unknown merge method {m}")))
                    .transpose()?,
                no_checks_timeout_secs: no_checks_timeout,
                check_command,
                worker_model,
                reviewer_model,
                planner_model,
                ..PatchProject::default()
            };
            let p = c.patch_project(&key, &patch)?;
            emit(json_out, &p, |p| {
                println!(
                    "project {}: up to {} agents at once; merge: {} ({}), PR title {:?}",
                    p.key,
                    p.max_parallel,
                    output::w(&p.merge.mode),
                    output::w(&p.merge.method),
                    p.merge.pr_title
                );
                if let Some(c) = &p.check_command {
                    println!("test command: {c}");
                }
                println!(
                    "review: {}, up to {} rounds of the loop",
                    output::w(&p.default_review_mode),
                    p.max_review_rounds
                );
                print_commands(p);
                for (what, model) in [
                    ("worker", &p.worker_model),
                    ("reviewer", &p.reviewer_model),
                    ("planner", &p.planner_model),
                ] {
                    if let Some(m) = model {
                        println!("{what} model: {m}");
                    }
                }
            });
        }
        ProjectCmd::Ls => {
            let projects = c.projects()?;
            emit(json_out, &projects, |ps| {
                let rows: Vec<Vec<String>> = ps
                    .iter()
                    .map(|p| {
                        vec![
                            p.key.to_string(),
                            p.name.clone(),
                            p.max_parallel.to_string(),
                        ]
                    })
                    .collect();
                output::table(&["KEY", "NAME", "AGENTS"], &rows);
            });
        }
        ProjectCmd::Rm {
            key,
            trash_folder,
            yes,
        } => {
            if trash_folder && !yes {
                let p = c.project(&key)?;
                let folder = p.repo_path.as_deref().unwrap_or("its folder (it has none)");
                let stdin = std::io::stdin();
                let terminal = stdin.is_terminal();
                confirm(&mut stdin.lock(), terminal, &p.key.to_string(), folder)?;
            }
            let out = c.delete_project(&key, &DeleteProject { trash_folder })?;
            emit(json_out, &out, deleted);
        }
    }
    Ok(())
}

/// The word of `--commands` as a setting.
fn policy(text: &str) -> Result<toknila_core::CommandPolicy> {
    from_wire_name(text).ok_or_else(|| anyhow!("--commands is ask, auto or trust, not {text:?}"))
}

/// What a project does when an agent wants to run a command, and the commands it remembers.
fn print_commands(p: &toknila_core::Project) {
    let what = match p.command_policy {
        toknila_core::CommandPolicy::Ask => "you answer each prompt",
        toknila_core::CommandPolicy::Auto => {
            "Claude Code decides and asks only about risky actions"
        }
        toknila_core::CommandPolicy::Trust => "agents are never asked",
    };
    println!("commands: {} ({what})", output::w(&p.command_policy));
    for rule in &p.allowed_commands {
        println!("allowed always: {rule}");
    }
    if !p.reviewer_web_domains.is_empty() {
        println!("reviewer reads: {}", p.reviewer_web_domains.join(", "));
    }
}

/// Asks the person to type the project's key before its folder goes to the Trash. Refuses when
/// nobody can answer (stdin is not a terminal) and when the answer is not the key.
fn confirm(input: &mut impl BufRead, terminal: bool, key: &str, folder: &str) -> Result<()> {
    if !terminal {
        bail!(
            "not deleting {key}: moving {folder} to the Trash needs a confirmation and stdin is not a terminal; pass --yes"
        );
    }
    eprintln!("This deletes project {key} and moves {folder} to the Trash.");
    eprint!("Type the project key ({key}) to go on: ");
    let mut line = String::new();
    input.read_line(&mut line)?;
    if line.trim() != key {
        bail!("{:?} is not {key}; nothing was deleted", line.trim());
    }
    Ok(())
}

/// What a deletion did, for a person: what went, where the folder is, and what stayed behind.
fn deleted(d: &ProjectDeletion) {
    println!(
        "project {} deleted: {} chain(s) {:?}, {} card(s)",
        d.project.key,
        d.chains.len(),
        d.chains,
        d.tickets
    );
    for w in &d.worktrees_removed {
        println!("removed workspace {w}");
    }
    let folder = d.project.repo_path.as_deref().unwrap_or_default();
    match d.folder {
        FolderFate::Kept => println!("folder kept: {folder}"),
        FolderFate::Trashed => println!("folder moved to the Trash: {folder}"),
        FolderFate::None => println!("no folder"),
    }
    if let Some(e) = &d.folder_error {
        println!("the folder stayed where it was: {e}");
    }
    for w in &d.warnings {
        println!("warning: {w}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_trash_needs_the_key_typed_at_a_terminal() {
        let typed = |text: &str, terminal: bool| {
            confirm(&mut text.as_bytes(), terminal, "DEMO", "/w/demo").map_err(|e| e.to_string())
        };
        assert_eq!(typed("DEMO\n", true), Ok(()));
        assert_eq!(typed("  DEMO  \n", true), Ok(()));
        assert_eq!(
            typed("demo\n", true),
            Err("\"demo\" is not DEMO; nothing was deleted".to_owned())
        );
        assert_eq!(
            typed("", true),
            Err("\"\" is not DEMO; nothing was deleted".to_owned())
        );
        let e = typed("DEMO\n", false).expect_err("no terminal");
        assert!(e.contains("stdin is not a terminal; pass --yes"), "{e}");
    }
}
