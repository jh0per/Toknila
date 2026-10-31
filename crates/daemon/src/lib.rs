//! Toknila daemon: SQLite store, and (next tickets) the HTTP API and event stream.

pub mod store;

/// Name the daemon reports about itself, e.g. in `/healthz` later on.
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
