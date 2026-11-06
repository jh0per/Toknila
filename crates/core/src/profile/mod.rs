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
//! [mcp]                                 # wiring filled in by the MCP ticket
//! enabled = false
//! config_flag = "--mcp-config"
//!
//! [hooks]                               # wiring filled in by the MCP ticket
//! enabled = false
//! settings_flag = "--settings"
//!
//! [output]
//! strip_ansi = true
//! scrollback_lines = 10000
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
