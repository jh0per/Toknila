//! Toknila daemon: SQLite store, HTTP API on 127.0.0.1 and (next tickets) the event stream.

pub mod http;
pub mod info;
pub mod paths;
pub mod profiles;
pub mod recovery;
pub mod server;
pub mod shipyard;
pub mod store;
pub mod stream;
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
