//! The agent side of the CLI: `tok mcp` and `tok hook` run inside an agent's process tree, so they
//! find the daemon and the run's token in the environment the runner gave the agent
//! (`toknila_api_types::runner::agent_env`), not in the data home, and never start a daemon.

use toknila_api_types::runner::agent_env;
use toknila_client::Client;
use toknila_core::AgentRole;

/// The daemon and the ticket this agent works on, with the run's own token.
#[derive(Debug)]
pub(super) struct AgentEnv {
    pub client: Client,
    /// Key of the ticket the run belongs to.
    pub ticket: String,
    /// What the run is: the ticket's worker, the reviewer next to it, or a project's planner.
    pub role: AgentRole,
    /// Key of a planner's project, which its tools name in the person's routes; only `tok mcp`
    /// of a planner needs it (see [`AgentEnv::project`]).
    project: Option<String>,
}

/// What a planner's tool answers when its run names no project.
fn no_project() -> String {
    format!(
        "{} is not set: this command runs inside a Toknila agent",
        agent_env::PROJECT
    )
}

impl AgentEnv {
    /// Reads the runner's environment; the error names the first variable that is missing.
    pub fn from_env() -> Result<AgentEnv, String> {
        let var = |name: &str| {
            std::env::var(name)
                .ok()
                .filter(|v| !v.is_empty())
                .ok_or_else(|| {
                    format!("{name} is not set: this command runs inside a Toknila agent")
                })
        };
        // Without a role the run is a worker's: the runners before review set none.
        let role = match std::env::var(agent_env::ROLE).as_deref() {
            Ok("reviewer") => AgentRole::Reviewer,
            Ok("planner") => AgentRole::Planner,
            _ => AgentRole::Worker,
        };
        Ok(AgentEnv {
            client: Client::new(var(agent_env::URL)?, var(agent_env::TOKEN)?),
            // A planner works on a project, not a ticket: its token names the project.
            ticket: if role == AgentRole::Planner {
                String::new()
            } else {
                var(agent_env::TICKET)?
            },
            role,
            project: var(agent_env::PROJECT).ok(),
        })
    }

    /// The planner's project, or the sentence its tools answer when the run names none.
    pub fn project(&self) -> Result<&str, String> {
        self.project.as_deref().ok_or_else(no_project)
    }
}
