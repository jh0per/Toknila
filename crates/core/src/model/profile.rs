use serde::{Deserialize, Serialize};

/// What an agent profile is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentRole {
    Worker,
    Reviewer,
    Release,
}

/// How a profile runs its agent. Phase 0 has only `sim`, which starts no process: a person sends
/// the agent's events from the CLI.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProfileKind {
    Sim,
}

/// How to launch an agent and in which role. A new agent is a new profile, not new code.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentProfile {
    pub name: String,
    pub role: AgentRole,
    pub kind: ProfileKind,
}

impl AgentProfile {
    /// The built-in simulated profile.
    pub fn sim() -> Self {
        Self {
            name: "sim".to_owned(),
            role: AgentRole::Worker,
            kind: ProfileKind::Sim,
        }
    }
}
