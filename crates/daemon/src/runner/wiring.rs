//! What a run's agent is given besides its command: the environment (daemon address, the run's
//! token) and, for a profile that asks for it, generated MCP and hook config files. The files
//! live under the data home (`runs/<ticket>/`), never in the worktree and never in the user's
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
use toknila_core::instructions;
use toknila_core::{AgentProfile, AgentRole, RunId, TicketKey};

/// Name of the MCP server in the agent's config.
const SERVER: &str = "toknila";

/// What one run needs wired.
pub(super) struct Context<'a> {
    /// `runs/<ticket id>`: where generated files go.
    pub dir: &'a Path,
    pub key: &'a TicketKey,
    pub role: AgentRole,
    pub run: RunId,
    pub token: &'a str,
    /// The daemon's address, once it listens.
    pub url: Option<&'a str>,
    /// The `toknila` binary the agent runs for `mcp` and `hook`.
    pub bin: &'a Path,
    /// The ticket's task, written into the instructions file.
    pub brief: &'a RunBrief,
}

/// Extra arguments and environment of the agent's process.
#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct Wiring {
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
}

/// `${NAME}`: the agent expands it from its own environment when it reads the file.
fn reference(name: &str) -> String {
    format!("${{{name}}}")
}

/// The MCP config: one stdio server, `toknila mcp`, whose environment is taken from the agent's.
fn mcp_config(bin: &Path) -> Value {
    let env: serde_json::Map<String, Value> = [
        agent_env::URL,
        agent_env::TOKEN,
        agent_env::TICKET,
        agent_env::ROLE,
    ]
    .into_iter()
    .map(|name| (name.to_owned(), Value::String(reference(name))))
    .collect();
    json!({ "mcpServers": { SERVER: {
        "type": "stdio",
        "command": bin.display().to_string(),
        "args": ["mcp"],
        "env": env,
    }}})
}

/// The hook settings: every subscribed hook runs `toknila hook <name>`. Forward slashes, so the
/// command reads the same under a Unix shell and Git Bash on Windows.
fn hook_settings(bin: &Path) -> Value {
    let program = bin.display().to_string().replace('\\', "/");
    let hooks: serde_json::Map<String, Value> = HOOKS
        .iter()
        .map(|(claude, ours)| {
            let command = format!("\"{program}\" hook {ours}");
            let entry = json!([{ "hooks": [{ "type": "command", "command": command }] }]);
            ((*claude).to_owned(), entry)
        })
        .collect();
    json!({ "hooks": hooks })
}

fn write_json(path: &Path, value: &Value) -> Result<(), RunnerError> {
    let text =
        serde_json::to_string_pretty(value).map_err(|e| RunnerError::Spawn(e.to_string()))?;
    std::fs::write(path, text)
        .map_err(|e| RunnerError::Spawn(format!("cannot write {}: {e}", path.display())))
}

/// Generates the files `profile` asks for and returns the flags and environment of the process.
pub(super) fn prepare(profile: &AgentProfile, cx: &Context<'_>) -> Result<Wiring, RunnerError> {
    let mut wiring = Wiring::default();
    if let Some(url) = cx.url {
        wiring.env.push((agent_env::URL.to_owned(), url.to_owned()));
    }
    wiring.env.extend([
        (agent_env::TOKEN.to_owned(), cx.token.to_owned()),
        (agent_env::TICKET.to_owned(), cx.key.to_string()),
        (agent_env::RUN.to_owned(), cx.run.to_string()),
        (
            agent_env::ROLE.to_owned(),
            match cx.role {
                AgentRole::Reviewer => "reviewer",
                _ => "worker",
            }
            .to_owned(),
        ),
    ]);
    let files = [
        (
            profile.mcp.enabled,
            profile.mcp.config_flag.as_deref(),
            "mcp.json",
            mcp_config(cx.bin),
        ),
        (
            profile.hooks.enabled,
            profile.hooks.settings_flag.as_deref(),
            "settings.json",
            hook_settings(cx.bin),
        ),
    ];
    for (enabled, flag, name, content) in files {
        let (true, Some(flag)) = (enabled, flag) else {
            continue;
        };
        std::fs::create_dir_all(cx.dir)
            .map_err(|e| RunnerError::Spawn(format!("cannot create {}: {e}", cx.dir.display())))?;
        let path: PathBuf = cx.dir.join(name);
        write_json(&path, &content)?;
        wiring.args.push(flag.to_owned());
        wiring.args.push(path.display().to_string());
    }
    if let Some(flag) = profile.instructions_flag.as_deref() {
        let brief = instructions::RunBrief {
            key: &cx.key.to_string(),
            title: &cx.brief.title,
            prompt: &cx.brief.prompt,
            acceptance: &cx.brief.acceptance,
        };
        let text = match cx.role {
            AgentRole::Reviewer => instructions::generate_review(&brief),
            _ => instructions::generate(&brief),
        };
        std::fs::create_dir_all(cx.dir)
            .map_err(|e| RunnerError::Spawn(format!("cannot create {}: {e}", cx.dir.display())))?;
        let path = cx.dir.join("instructions.md");
        std::fs::write(&path, text)
            .map_err(|e| RunnerError::Spawn(format!("cannot write {}: {e}", path.display())))?;
        wiring.args.push(flag.to_owned());
        wiring.args.push(path.display().to_string());
    }
    Ok(wiring)
}

#[cfg(test)]
mod tests;
