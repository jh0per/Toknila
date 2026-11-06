//! Toknila core: the domain model and pure rules (state machine, chain graph).
//!
//! This crate performs no I/O and does not depend on an async runtime or a database.
//! The daemon owns persistence and transport; core owns meaning.

pub mod agent;
pub mod dag;
pub mod events;
pub mod graph;
pub mod hook;
pub mod instructions;
pub mod machine;
pub mod model;
pub mod nudge;
pub mod plan;
pub mod profile;
pub mod schedule;
pub mod spec;
pub mod template;
pub mod thread;
pub mod wire;

pub use model::*;

/// Crate version, shared by every binary in the workspace.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_semver_like() {
        let parts: Vec<&str> = VERSION.split('.').collect();
        assert_eq!(parts.len(), 3, "unexpected version {VERSION}");
        assert!(parts.iter().all(|p| p.parse::<u32>().is_ok()));
    }
}
