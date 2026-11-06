use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// What an agent profile is for.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentRole {
    Worker,
    Reviewer,
    Release,
}

/// How an agent receives its MCP server (the agent event contract): the runner writes a config
/// file under the data home and passes its path with `config_flag`.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct McpWiring {
    /// Whether the launcher wires the MCP server into this agent.
    pub enabled: bool,
    /// Flag of the agent's command that takes the path of a generated MCP config.
    pub config_flag: Option<String>,
}

/// How an agent reports through hooks, a second event source: the runner writes a settings file
/// under the data home and passes its path with `settings_flag`.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct HookWiring {
    /// Whether the launcher wires hooks into this agent.
    pub enabled: bool,
    /// Flag of the agent's command that takes the path of a generated hook settings file.
    pub settings_flag: Option<String>,
}

/// How the terminal output of an agent is kept and shown.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct OutputRules {
    /// Remove ANSI escape sequences from the text kept for search and summaries.
    pub strip_ansi: bool,
    /// Lines of output kept per run.
    pub scrollback_lines: u32,
    /// Size at which a run's log file is rotated to `<run>.log.1`, in bytes.
    pub max_log_bytes: u64,
}

impl Default for OutputRules {
    fn default() -> Self {
        Self {
            strip_ansi: true,
            scrollback_lines: 10_000,
            max_log_bytes: 20 * 1024 * 1024,
        }
    }
}

/// When a silent agent is nudged and how often.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Nudge {
    /// Seconds without output before a nudge.
    pub idle_secs: u64,
    /// Nudges sent before the ticket is blocked.
    pub max_nudges: u32,
    /// Text sent to the agent.
    pub message: String,
}

impl Default for Nudge {
    fn default() -> Self {
        Self {
            idle_secs: 300,
            max_nudges: 3,
            message: "Continue, or report with the finished event.".to_owned(),
        }
    }
}

/// How to launch an agent and in which role. A new agent is a new profile file, not new code.
/// The working directory is not part of it: the ship supplies it at launch.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentProfile {
    pub name: String,
    pub role: AgentRole,
    /// Program and arguments, never a shell string. Empty only for the simulated profile, which
    /// starts no process: a person sends its events from the CLI.
    pub command: Vec<String>,
    /// Extra environment of the process.
    pub env: BTreeMap<String, String>,
    /// Instructions file the agent reads, relative to its working directory.
    pub instructions_file: Option<String>,
    /// Flag of the agent's command that takes the path of the generated per-run instructions
    /// (under the data home; the project's own instructions file is never written).
    #[serde(default)]
    pub instructions_flag: Option<String>,
    pub mcp: McpWiring,
    pub hooks: HookWiring,
    pub output: OutputRules,
    pub nudge: Nudge,
}

impl AgentProfile {
    /// Name of the built-in simulated profile.
    pub const SIM: &str = "sim";
    /// Name of the built-in Claude Code profile.
    pub const CLAUDE_CODE: &str = "claude-code";
    /// Name of the built-in simulated reviewer profile.
    pub const SIM_REVIEWER: &str = "sim-reviewer";
    /// Name of the built-in Claude Code reviewer profile.
    pub const CLAUDE_CODE_REVIEWER: &str = "claude-code-reviewer";

    /// A profile that runs `command`, with defaults everywhere else.
    pub fn new(name: &str, role: AgentRole, command: Vec<String>) -> Self {
        Self {
            name: name.to_owned(),
            role,
            command,
            env: BTreeMap::new(),
            instructions_file: None,
            instructions_flag: None,
            mcp: McpWiring::default(),
            hooks: HookWiring::default(),
            output: OutputRules::default(),
            nudge: Nudge::default(),
        }
    }

    /// The built-in simulated profile.
    pub fn sim() -> Self {
        Self::new(Self::SIM, AgentRole::Worker, Vec::new())
    }

    /// The built-in simulated reviewer: starts no process, a person sends its verdict.
    pub fn sim_reviewer() -> Self {
        Self::new(Self::SIM_REVIEWER, AgentRole::Reviewer, Vec::new())
    }

    /// The built-in reviewer that launches Claude Code like the worker profile does, in the
    /// reviewer role.
    pub fn claude_code_reviewer() -> Self {
        Self {
            name: Self::CLAUDE_CODE_REVIEWER.to_owned(),
            role: AgentRole::Reviewer,
            ..Self::claude_code()
        }
    }

    /// The built-in profile that launches Claude Code, with the MCP server and the hooks wired in
    /// through `--mcp-config` and `--settings`.
    pub fn claude_code() -> Self {
        Self {
            instructions_file: Some("CLAUDE.md".to_owned()),
            instructions_flag: Some("--append-system-prompt-file".to_owned()),
            mcp: McpWiring {
                enabled: true,
                config_flag: Some("--mcp-config".to_owned()),
            },
            hooks: HookWiring {
                enabled: true,
                settings_flag: Some("--settings".to_owned()),
            },
            ..Self::new(
                Self::CLAUDE_CODE,
                AgentRole::Worker,
                vec!["claude".to_owned()],
            )
        }
    }
}
