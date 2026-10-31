//! Toknila daemon. Skeleton only: the HTTP server, store and event stream arrive in
//! P0-06 (SQLite), P0-07 (daemon frame) and P0-08 (command API).

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
