//! What a run's agent is given besides its command: the environment (daemon address, the run's
//! token) and, for a profile that asks for it, generated MCP and hook config files. The files
//! live under the data home (`runs/<ticket>/<role>/`, so the worker and the reviewer of a ticket
//! never share or overwrite a file), never in the worktree and never in the user's
//! own agent settings, and reach the agent through command-line flags. The instructions file
//! (`instructions.md`) is passed the same way, as an addition to the system prompt, so the
//! worktree's own `CLAUDE.md` and `AGENTS.md` stay untouched and still apply. The token is not in any
//! file: the files name environment variables that the agent's process inherits.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use toknila_api_types::runner::agent_env;
use toknila_app::RunnerError;
use toknila_app::runner::RunBrief;
use toknila_core::hook::HOOKS;
use toknila_core::{AgentProfile, AgentRole, RunId, TicketKey, instructions};

/// Name of the MCP server in the agent's config.
const SERVER: &str = "toknila";

/// What one run needs wired.
pub(super) struct Context<'a> {
    /// `runs/<ticket id>`: the folder of the ticket's runs; generated files go in a folder of the
    /// role under it.
    pub dir: &'a Path,
    pub key: &'a TicketKey,
    pub role: AgentRole,
    pub run: RunId,
    pub token: &'a str,
    /// The daemon's address, once it listens.
    pub url: Option<&'a str>,
    /// The `toknila` binary the agent runs for `mcp` and `hook`.
    pub bin: &'a Path,
    /// The folder the agent runs in (the ship, or the project root): the one place it may read
    /// besides the design files.
    pub repo: &'a Path,
    /// The ticket's task, written into the instructions file.
    pub brief: &'a RunBrief,
}

/// Extra arguments and environment of the agent's process.
#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct Wiring {
    /// The profile's command with the first prompt filled in (`{kickoff}`).
    pub command: Vec<String>,
    /// Flags for the generated files, after the command.
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
    /// Inherited variables the process must not get (see `AgentProfile::inherited_to_drop`).
    pub env_remove: Vec<String>,
}

/// The folder name of a role's generated files.
fn role_dir(role: AgentRole) -> &'static str {
    match role {
        AgentRole::Worker => "worker",
        AgentRole::Reviewer => "reviewer",
        AgentRole::Release => "release",
        AgentRole::Planner => "planner",
    }
}

/// `${NAME}`: the agent expands it from its own environment when it reads the file.
fn reference(name: &str) -> String {
    format!("${{{name}}}")
}

/// The MCP config: one stdio server, `toknila mcp`, whose environment is taken from the agent's.
fn mcp_config(bin: &Path, role: AgentRole) -> Value {
    // A planner works on a project, not a card: it has no ticket for the variable to name, and a
    // reference to a variable that is not set would stop the agent's config from loading.
    let names: &[&str] = match role {
        AgentRole::Planner => &[agent_env::URL, agent_env::TOKEN, agent_env::ROLE],
        _ => &[
            agent_env::URL,
            agent_env::TOKEN,
            agent_env::TICKET,
            agent_env::ROLE,
        ],
    };
    let env: serde_json::Map<String, Value> = names
        .iter()
        .copied()
        .map(|name| (name.to_owned(), Value::String(reference(name))))
        .collect();
    json!({ "mcpServers": { SERVER: {
        "type": "stdio",
        "command": bin.display().to_string(),
        "args": ["mcp"],
        "env": env,
    }}})
}

/// The tools denied to every role, by the names of the tools reference. A card is done by one
/// agent, visible in its terminal, inside its own ship: so no tool that starts subagents (`Agent`,
/// `Workflow`), leaves the ship (`EnterWorktree`) or schedules work outside the card
/// (`CronCreate`, `CronDelete`, `RemoteTrigger`).
const DENIED_TOOLS: [&str; 6] = [
    "Agent",
    "Workflow",
    "EnterWorktree",
    "CronCreate",
    "CronDelete",
    "RemoteTrigger",
];

/// The environment switch for auto memory (the memory page), set next to the settings key.
const AUTO_MEMORY_OFF: (&str, &str) = ("CLAUDE_CODE_DISABLE_AUTO_MEMORY", "1");

/// The settings file of every role: auto memory off (a ship is a worktree of the person's own
/// repository, whose personal notes were written for another session), the tools of [`DENIED_TOOLS`] denied,
/// Read, Grep, Glob and Bash denied on the person's private folders (`toknila_core::private`),
/// the hooks and the status line of [`hook_settings`], and the profile's allow rules, when it has
/// any for this project, are what the agent may do without asking.
fn settings(bin: &Path, hooks: bool, rules: &[String], private: &[String]) -> Value {
    let mut out = serde_json::Map::new();
    if hooks && let Value::Object(h) = hook_settings(bin) {
        out.extend(h);
    }
    out.insert("autoMemoryEnabled".to_owned(), json!(false));
    let deny: Vec<&str> = DENIED_TOOLS
        .iter()
        .copied()
        .chain(private.iter().map(String::as_str))
        .collect();
    let mut permissions = json!({ "deny": deny });
    if !rules.is_empty() {
        permissions["allow"] = json!(rules);
    }
    out.insert("permissions".to_owned(), permissions);
    Value::Object(out)
}

/// The hook settings: every subscribed hook runs `toknila hook <name>`, and the status line runs
/// `toknila statusline`, which passes on the plan limits Claude Code documents there (they feed
/// the Usage page; the line itself stays empty). Forward slashes, so the command reads the same
/// under a Unix shell and Git Bash on Windows.
fn hook_settings(bin: &Path) -> Value {
    let program = bin.display().to_string().replace('\\', "/");
    let hooks: serde_json::Map<String, Value> = HOOKS
        .iter()
        .map(|(claude, ours)| {
            let command = format!("\"{program}\" hook {ours}");
            let mut hook = json!({ "type": "command", "command": command });
            // The permission hook holds the agent's dialog until the person answers in the window.
            if *ours == "permission_request" {
                hook["timeout"] = json!(toknila_core::hook::PERMISSION_HOOK_TIMEOUT_SECS);
            }
            let entry = json!([{ "hooks": [hook] }]);
            ((*claude).to_owned(), entry)
        })
        .collect();
    let status_line = json!({ "type": "command", "command": format!("\"{program}\" statusline") });
    json!({ "hooks": hooks, "statusLine": status_line })
}

fn write_json(path: &Path, value: &Value) -> Result<(), RunnerError> {
    let text =
        serde_json::to_string_pretty(value).map_err(|e| RunnerError::Spawn(e.to_string()))?;
    std::fs::write(path, text)
        .map_err(|e| RunnerError::Spawn(format!("cannot write {}: {e}", path.display())))
}

/// The designs of a worker's card that are still in the project's library, with their files: what
/// the instructions name and what the run may read, and nothing else of the library.
fn worker_designs(cx: &Context<'_>) -> (Vec<instructions::BriefDesign>, Vec<PathBuf>) {
    let (Some(project), true) = (cx.brief.design_project, cx.role == AgentRole::Worker) else {
        return (Vec::new(), Vec::new());
    };
    let Some(home) = cx.dir.parent().and_then(Path::parent) else {
        return (Vec::new(), Vec::new());
    };
    let found = crate::designs::Designs::new(home, project).locate(&cx.brief.card_designs);
    let files = found.iter().filter_map(|d| d.path.clone()).collect();
    let briefs = found
        .into_iter()
        .map(|d| instructions::BriefDesign {
            name: d.name,
            path: d.path.map(|p| p.display().to_string()),
            region: d.region,
        })
        .collect();
    (briefs, files)
}

fn brief_of_key<'a>(
    cx: &'a Context<'_>,
    key: &'a str,
    designs: &'a [instructions::BriefDesign],
) -> instructions::RunBrief<'a> {
    instructions::RunBrief {
        designs,
        key,
        title: &cx.brief.title,
        prompt: &cx.brief.prompt,
        acceptance: &cx.brief.acceptance,
        check_command: cx.brief.check_command.as_deref(),
        setup_command: cx.brief.setup_command.as_deref(),
        earlier: &cx.brief.earlier,
        restarted: cx.brief.resume.as_ref().is_some_and(|r| r.lost),
        waiting: &cx.brief.waiting,
        returned: cx.brief.returned.as_ref(),
        design_first: cx.brief.design_first,
        has_designs: !cx.brief.designs.is_empty(),
        discussion: &cx.brief.discussion,
        epic: cx
            .brief
            .epic
            .as_ref()
            .map(|(name, goal)| (name.as_str(), goal.as_str())),
        check: cx.brief.check.as_ref(),
        threads: &cx.brief.threads,
        hand_in: cx.brief.hand_in.as_deref(),
    }
}

/// The toolchains the repository in `dir` holds, by the first word of their commands: `cargo` for
/// a `Cargo.toml`, `npm`, `pnpm` or `yarn` for a `package.json` (by its lock file).
pub(super) fn repo_tools(dir: &Path) -> Vec<String> {
    let mut tools = Vec::new();
    if dir.join("Cargo.toml").exists() {
        tools.push("cargo".to_owned());
    }
    if dir.join("package.json").exists() {
        let manager = if dir.join("pnpm-lock.yaml").exists() {
            "pnpm"
        } else if dir.join("yarn.lock").exists() {
            "yarn"
        } else {
            "npm"
        };
        tools.push(manager.to_owned());
    }
    tools
}

pub(super) use super::resumed::{briefed, resumed_argv};

/// Generates the files `profile` asks for and returns the flags and environment of the process.
pub(super) fn prepare(profile: &AgentProfile, cx: &Context<'_>) -> Result<Wiring, RunnerError> {
    let inherited = std::env::vars_os().filter_map(|(k, _)| k.into_string().ok());
    let mut wiring = prepare_with(profile, cx, inherited)?;
    // The tool folders the login shell left out (`~/.cargo/bin`), so a command runs as given.
    if let Some(path) = crate::agent_path::agent_path_value() {
        wiring.env.push(("PATH".to_owned(), path));
    }
    Ok(wiring)
}

/// [`prepare`] for a daemon whose environment has the variables called `inherited`.
pub(super) fn prepare_with(
    profile: &AgentProfile,
    cx: &Context<'_>,
    inherited: impl IntoIterator<Item = String>,
) -> Result<Wiring, RunnerError> {
    // The project's setting for commands changes a worker's permission mode and allow rules.
    let profile = &profile.under_policy(cx.brief.command_policy, &cx.brief.allowed_commands);
    // The domains the project lets a reviewer read go into the reviewer's allow rules.
    let profile = &profile.with_reviewer_domains(&cx.brief.web_domains);
    // The tests and checks the hand-in names: the check command's toolchain and the repository's.
    let profile =
        &profile.with_reviewer_checks(cx.brief.check_command.as_deref(), &cx.brief.repo_tools);
    // A planner run is labelled by its project's key; there is no card.
    let key = match cx.role {
        AgentRole::Planner => cx.key.project().to_string(),
        _ => cx.key.to_string(),
    };
    let (design_briefs, worker_files) = worker_designs(cx);
    let brief = brief_of_key(cx, &key, &design_briefs);
    let command = resumed_argv(profile, cx.brief)
        .unwrap_or_else(|| profile.argv(&instructions::kickoff(cx.role, &brief)));
    let mut wiring = Wiring {
        command,
        env_remove: profile.inherited_to_drop(inherited),
        ..Wiring::default()
    };
    let dir = cx.dir.join(role_dir(cx.role));
    // Reads are scoped to the repository the run works in (the ship, or the project root for a
    // planner); a profile's bare `Read`, `Grep` or `Glob` rule never reaches the settings file.
    let mut rules = toknila_core::private::scope_reads(
        profile.permissions.rules(cx.brief.check_command.as_deref()),
        &cx.repo.display().to_string(),
    );
    // The folder under its real name too (`/var` is `/private/var` on macOS): the agent may
    // report either spelling of the same place.
    if let Ok(real) = std::fs::canonicalize(cx.repo) {
        let real = real.display().to_string();
        let rule = toknila_core::private::read_allow_tree_rule(
            real.strip_prefix(r"\\?\").unwrap_or(&real),
        );
        if !rules.contains(&rule) {
            rules.push(rule);
        }
    }
    // A planner reads the project's design files and a card's agent those of its card, which sit
    // outside the repository: one explicit rule per file, nothing wider.
    let design_files: &[PathBuf] = match cx.role {
        AgentRole::Planner => &cx.brief.designs,
        _ => &worker_files,
    };
    rules.extend(
        design_files
            .iter()
            .map(|f| toknila_core::private::read_allow_rule(&f.display().to_string())),
    );
    if let Some(url) = cx.url {
        wiring.env.push((agent_env::URL.to_owned(), url.to_owned()));
    }
    wiring
        .env
        .push((agent_env::TOKEN.to_owned(), cx.token.to_owned()));
    if cx.role != AgentRole::Planner {
        wiring
            .env
            .push((agent_env::TICKET.to_owned(), cx.key.to_string()));
    }
    wiring.env.extend([
        (agent_env::RUN.to_owned(), cx.run.to_string()),
        (
            agent_env::ROLE.to_owned(),
            match cx.role {
                AgentRole::Reviewer => "reviewer",
                AgentRole::Planner => "planner",
                _ => "worker",
            }
            .to_owned(),
        ),
    ]);
    // Set here, after the scrub of inherited variables, so the owner's shell cannot turn it on.
    if profile.hooks.settings_flag.is_some() {
        wiring
            .env
            .push((AUTO_MEMORY_OFF.0.to_owned(), AUTO_MEMORY_OFF.1.to_owned()));
    }
    let files = [
        (
            profile.mcp.enabled,
            profile.mcp.config_flag.as_deref(),
            "mcp.json",
            mcp_config(cx.bin, cx.role),
        ),
        (
            true,
            profile.hooks.settings_flag.as_deref(),
            "settings.json",
            settings(
                cx.bin,
                profile.hooks.enabled,
                &rules,
                &super::private::deny_rules(cx.dir, design_files),
            ),
        ),
    ];
    for (enabled, flag, name, content) in files {
        let (true, Some(flag)) = (enabled, flag) else {
            continue;
        };
        std::fs::create_dir_all(&dir)
            .map_err(|e| RunnerError::Spawn(format!("cannot create {}: {e}", dir.display())))?;
        let path: PathBuf = dir.join(name);
        write_json(&path, &content)?;
        wiring.args.push(flag.to_owned());
        wiring.args.push(path.display().to_string());
    }
    wiring
        .args
        .extend(profile.model_args(cx.brief.model.as_deref()));
    if let Some(flag) = profile.instructions_flag.as_deref() {
        let text = match cx.role {
            AgentRole::Planner => {
                let facts = cx.brief.project.as_ref().ok_or_else(|| {
                    RunnerError::Spawn("a planner run needs the project's facts".to_owned())
                })?;
                instructions::generate_planner(facts)
            }
            AgentRole::Reviewer => instructions::generate_review(&brief),
            _ => instructions::generate(&brief),
        };
        std::fs::create_dir_all(&dir)
            .map_err(|e| RunnerError::Spawn(format!("cannot create {}: {e}", dir.display())))?;
        let path = dir.join("instructions.md");
        std::fs::write(&path, text)
            .map_err(|e| RunnerError::Spawn(format!("cannot write {}: {e}", path.display())))?;
        wiring.args.push(flag.to_owned());
        wiring.args.push(path.display().to_string());
    }
    Ok(wiring)
}

#[cfg(test)]
mod tests;
