use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::CommandPolicy;

/// What an agent profile is for.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentRole {
    Worker,
    Reviewer,
    Release,
    /// A project's main agent: it plans (chains, cards, dependencies, threads) and starts,
    /// accepts and merges nothing.
    Planner,
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

/// What the agent may do without asking, in the agent's own rule syntax. The runner writes the
/// rules into the generated settings file (`hooks.settings_flag` names the flag that takes it).
/// A rule that contains `{check_command}` applies only when the project names a check command, and
/// has the command in its place.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Permissions {
    /// Allow rules, e.g. `Bash(git diff *)`.
    pub allow: Vec<String>,
}

impl Permissions {
    /// The placeholder for the project's check command inside a rule.
    pub const CHECK_COMMAND: &'static str = "{check_command}";

    /// The rules with the check command filled in; rules that need one are dropped without it.
    pub fn rules(&self, check_command: Option<&str>) -> Vec<String> {
        self.allow
            .iter()
            .filter_map(
                |rule| match (rule.contains(Self::CHECK_COMMAND), check_command) {
                    (false, _) => Some(rule.clone()),
                    (true, Some(c)) => Some(rule.replace(Self::CHECK_COMMAND, c)),
                    (true, None) => None,
                },
            )
            .collect()
    }
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
    /// Seconds without a sign of life before a nudge.
    pub idle_secs: u64,
    /// Nudges sent within `window_secs` before the ticket goes to Waiting (silent).
    pub max_nudges: u32,
    /// The sliding window, in seconds, in which `max_nudges` count.
    pub window_secs: u64,
    /// Text sent to the agent.
    pub message: String,
}

impl Default for Nudge {
    fn default() -> Self {
        Self {
            idle_secs: 120,
            max_nudges: 3,
            window_secs: 900,
            message: "Report your state through the Toknila tools: call note with progress, \
                      ask_human if you are stuck, or finished if you are done."
                .to_owned(),
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
    #[serde(default)]
    pub permissions: Permissions,
    pub output: OutputRules,
    pub nudge: Nudge,
    /// Seconds after spawn without any sign of life from the agent (hook or MCP) before the
    /// window shows that it waits on its screen. 0 turns the check off.
    #[serde(default = "default_startup_secs")]
    pub startup_secs: u64,
}

fn default_startup_secs() -> u64 {
    crate::question::DEFAULT_STARTUP_SECS
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
    /// Name of the built-in Claude Code planner profile.
    pub const CLAUDE_CODE_PLANNER: &str = "claude-code-planner";

    /// The placeholder in `command` for the first prompt of the run (see
    /// `instructions::kickoff`). It is an argv element, or part of one (`--prompt={kickoff}`), so
    /// the profile decides where the agent's CLI takes its first prompt. Without it the agent
    /// starts at an empty prompt.
    pub const KICKOFF: &'static str = "{kickoff}";

    /// The command with the first prompt filled in.
    pub fn argv(&self, kickoff: &str) -> Vec<String> {
        self.command
            .iter()
            .map(|a| a.replace(Self::KICKOFF, kickoff))
            .collect()
    }

    /// Whether the command starts Claude Code (a program named `claude`, with or without a path
    /// or `.exe`): only such a profile understands its `--model` flag.
    pub fn runs_claude_code(&self) -> bool {
        let Some(program) = self.command.first() else {
            return false;
        };
        let name = program.rsplit(['/', '\\']).next().unwrap_or(program);
        let name = name.strip_suffix(".exe").unwrap_or(name);
        name.eq_ignore_ascii_case("claude")
    }

    /// The arguments that run the command on `model` (`--model <model>`, from the CLI reference
    /// of Claude Code): none without a model, for a profile that does not run Claude Code, or
    /// when the profile's own command already names one, which then stays the profile's choice.
    pub fn model_args(&self, model: Option<&str>) -> Vec<String> {
        let Some(model) = model.map(str::trim).filter(|m| !m.is_empty()) else {
            return Vec::new();
        };
        let named = self
            .command
            .iter()
            .any(|a| a == "--model" || a.starts_with("--model="));
        if !self.runs_claude_code() || named {
            return Vec::new();
        }
        vec!["--model".to_owned(), model.to_owned()]
    }

    /// The inherited environment variables of the agent's own kind that must not reach it:
    /// `CLAUDECODE` and `CLAUDE_CODE_*` describe the session that started Toknila (a child-session
    /// marker, the entrypoint), not this run. A variable the profile's `env` sets is its choice and
    /// stays. `names` are the names in the daemon's environment.
    pub fn inherited_to_drop<I: IntoIterator<Item = String>>(&self, names: I) -> Vec<String> {
        let same = |a: &str, b: &str| a.eq_ignore_ascii_case(b);
        names
            .into_iter()
            .filter(|n| {
                let up = n.to_ascii_uppercase();
                (up == "CLAUDECODE" || up.starts_with("CLAUDE_CODE_"))
                    && !self.env.keys().any(|k| same(k, n))
            })
            .collect()
    }

    /// The profile as a card's worker runs it under a project's setting: the project's
    /// [`CommandPolicy`] picks the permission mode of the command (auto mode replaces
    /// `acceptEdits`) and adds its allow rules, and the rules a person remembered are allowed too.
    /// Only a worker that runs Claude Code is changed; a reviewer or planner stays as it is (its
    /// `dontAsk` is what keeps it read-only).
    pub fn under_policy(&self, policy: CommandPolicy, remembered: &[String]) -> Self {
        let mut out = self.clone();
        if self.role != AgentRole::Worker || !self.runs_claude_code() {
            return out;
        }
        let mut args = out.command.iter_mut().peekable();
        while let Some(arg) = args.next() {
            if arg == "--permission-mode"
                && let Some(mode) = args.peek_mut()
            {
                **mode = policy.permission_mode(mode).to_owned();
            }
        }
        out.permissions
            .allow
            .extend(policy.allow_rules().iter().map(|r| (*r).to_owned()));
        out.permissions.allow.extend(remembered.iter().cloned());
        out
    }

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
            permissions: Permissions::default(),
            output: OutputRules::default(),
            nudge: Nudge::default(),
            startup_secs: default_startup_secs(),
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
    ///
    /// It runs in `dontAsk` mode: nobody can answer its permission prompts, so what it may do is
    /// the allow-list and nothing else. Read-only git, the project's check command, and the
    /// filters a check is piped through (every part of a piped command must be allowed, see
    /// the permissions page of the Claude Code docs, "Compound commands").
    pub fn claude_code_reviewer() -> Self {
        let mut allow: Vec<String> = [
            "git status *",
            "git diff *",
            "git log *",
            "git show *",
            "git merge-base *",
            "git rev-parse *",
            "grep *",
            "head *",
            "tail *",
            "wc *",
        ]
        .iter()
        .map(|r| format!("Bash({r})"))
        .collect();
        allow.push(format!("Bash({} *)", Permissions::CHECK_COMMAND));
        allow.push("mcp__toknila".to_owned());
        Self {
            name: Self::CLAUDE_CODE_REVIEWER.to_owned(),
            role: AgentRole::Reviewer,
            command: ["claude", "--permission-mode", "dontAsk", Self::KICKOFF]
                .map(str::to_owned)
                .to_vec(),
            permissions: Permissions { allow },
            ..Self::claude_code()
        }
    }

    /// The built-in planner: Claude Code in the project's repository, read-only.
    ///
    /// It runs in `dontAsk` mode: what it may do is the allow-list and nothing else, so it can
    /// read the code (Read, Grep, Glob, read-only git) and use the Toknila planner tools, and
    /// it cannot edit or write a file or run anything that changes the repository. Hooks are off
    /// (a planner is not a card's agent: nothing is nudged, nothing is handed in); the settings
    /// file carries the allow rules only.
    pub fn claude_code_planner() -> Self {
        let mut allow: Vec<String> = ["Read", "Grep", "Glob"].map(str::to_owned).to_vec();
        allow.extend(
            [
                "git status *",
                "git diff *",
                "git log *",
                "git show *",
                "git ls-files *",
                "git merge-base *",
                "git rev-parse *",
            ]
            .iter()
            .map(|r| format!("Bash({r})")),
        );
        allow.push("mcp__toknila".to_owned());
        Self {
            name: Self::CLAUDE_CODE_PLANNER.to_owned(),
            role: AgentRole::Planner,
            command: ["claude", "--permission-mode", "dontAsk", Self::KICKOFF]
                .map(str::to_owned)
                .to_vec(),
            hooks: HookWiring {
                enabled: false,
                settings_flag: Some("--settings".to_owned()),
            },
            permissions: Permissions { allow },
            // The person talks to it; no screen-wait rule applies.
            startup_secs: 0,
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
            permissions: Permissions {
                allow: [
                    "Bash(git status *)",
                    "Bash(git diff *)",
                    "Bash(git log *)",
                    "Bash(git show *)",
                    "Bash(git add *)",
                    "Bash(git commit *)",
                    "Bash({check_command} *)",
                    "mcp__toknila",
                ]
                .map(str::to_owned)
                .to_vec(),
            },
            ..Self::new(
                Self::CLAUDE_CODE,
                AgentRole::Worker,
                ["claude", "--permission-mode", "acceptEdits", Self::KICKOFF]
                    .map(str::to_owned)
                    .to_vec(),
            )
        }
    }
}
