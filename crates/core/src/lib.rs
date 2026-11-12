//! Toknila core: the domain model and pure rules (state machine, chain graph).
//!
//! This crate performs no I/O and does not depend on an async runtime or a database.
//! The daemon owns persistence and transport; core owns meaning.

pub mod address;
pub mod agent;
pub mod conversation;
pub mod dag;
pub mod defaults;
pub mod design;
pub mod discussion;
pub mod events;
pub mod graph;
pub mod guard;
pub mod handin;
pub mod hook;
pub mod inbox;
pub mod instructions;
pub mod machine;
pub mod migrations;
pub mod model;
pub mod nudge;
pub mod plan;
pub mod planner;
pub mod plans;
pub mod pr;
pub mod private;
pub mod profile;
pub mod question;
pub mod release_run;
pub mod removal;
pub mod repo;
pub mod roadmap;
pub mod schedule;
pub mod spec;
pub mod strays;
pub mod suggest;
pub mod suggestion;
pub mod system;
pub mod template;
pub mod thread;
pub mod toolpath;
pub mod usage;
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
