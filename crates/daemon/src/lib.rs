//! Toknila daemon: SQLite store, HTTP API on 127.0.0.1 and (next tickets) the event stream.

pub mod cleanup;
mod cli_link;
mod gh;
mod git;
pub mod github;
pub mod http;
pub mod info;
pub mod merger;
pub mod paths;
pub mod plan_files;
pub mod pr_watch;
pub mod profiles;
pub mod recovery;
pub mod runner;
pub mod server;
pub mod shipyard;
pub mod store;
pub mod stream;
pub mod tools;
pub mod trash;
pub mod usage;
pub mod web;

/// Name the daemon reports about itself.
pub fn component_name() -> String {
    format!("toknila-daemon {}", toknila_core::VERSION)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn component_name_carries_core_version() {
        assert!(component_name().ends_with(toknila_core::VERSION));
    }
}
