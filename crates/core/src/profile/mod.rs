//! Agent profiles as TOML: parsing and validating one profile file, and the catalog of built-in
//! profiles plus the files of the profiles folder. Pure: the daemon reads the files.
//!
//! ```toml
//! role = "worker"                       # worker (default) | reviewer | release
//! command = ["my-agent", "--yolo"]      # argv list, never a shell string
//! instructions_file = "AGENTS.md"       # optional, relative to the working directory
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
//! [output]
//! strip_ansi = true
//! scrollback_lines = 10000
//! max_log_bytes = 20971520              # a run's log rotates to <run>.log.1 at this size
//!
//! [nudge]
//! idle_secs = 300
//! max_nudges = 3
//! message = "Continue, or report with the finished event."
//! ```
//!
//! The profile's name is the file's name without `.toml`.

mod catalog;
mod parse;

pub use catalog::{Catalog, ListedProfile, ProfileFile, ProfileProblem, catalog};
pub use parse::{ProfileError, parse};

#[cfg(test)]
mod tests;
