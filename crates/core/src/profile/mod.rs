//! Agent profiles as TOML: parsing and validating one profile file, and the catalog of built-in
//! profiles plus the files of the profiles folder. Pure: the daemon reads the files.
//!
//! ```toml
//! role = "worker"                       # worker (default) | reviewer | release
//! command = ["my-agent", "{kickoff}"]   # argv list, never a shell string; {kickoff} is the first prompt
//! instructions_file = "AGENTS.md"       # optional, relative to the working directory
//! instructions_flag = "--append-system-prompt-file"  # takes the path of the generated per-run instructions
//!
//! [env]
//! MY_AGENT_MODE = "headless"
//!
//! [mcp]                                 # the runner writes a config file and passes its path with the flag
//! enabled = false
//! config_flag = "--mcp-config"
//!
//! [hooks]                               # the runner writes a settings file and passes its path with the flag
//! enabled = false
//! settings_flag = "--settings"
//!
//! [permissions]                         # allow rules in the agent's syntax, written into the settings file
//! allow = ["Bash(git diff *)", "Bash({check_command} *)"]   # {check_command}: the project's, rule dropped without one
//!
//! [output]
//! strip_ansi = true
//! scrollback_lines = 10000
//! max_log_bytes = 20971520              # a run's log rotates to <run>.log.1 at this size
//!
//! [nudge]
//! idle_secs = 120                       # no sign of life (MCP, hook, else output) and no child process
//! max_nudges = 3                        # nudges within window_secs, then Waiting (silent)
//! window_secs = 900
//! message = "Report your state through the Toknila tools: ..."
//! ```
//!
//! The profile's name is the file's name without `.toml`.

mod catalog;
mod parse;

pub use catalog::{Catalog, ListedProfile, ProfileFile, ProfileProblem, catalog};
pub use parse::{ProfileError, parse};

#[cfg(test)]
mod tests;
