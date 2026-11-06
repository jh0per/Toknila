//! The agent side of the CLI: `tok mcp` and `tok hook` run inside an agent's process tree, so they
//! find the daemon and the run's token in the environment the runner gave the agent
//! (`toknila_api_types::runner::agent_env`), not in the data home, and never start a daemon.

use toknila_api_types::runner::agent_env;
use toknila_client::Client;

/// The daemon and the ticket this agent works on, with the run's own token.
#[derive(Debug)]
pub(super) struct AgentEnv {
    pub client: Client,
    /// Key of the ticket the run belongs to.
    pub ticket: String,
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
        Ok(AgentEnv {
            client: Client::new(var(agent_env::URL)?, var(agent_env::TOKEN)?),
            ticket: var(agent_env::TICKET)?,
        })
    }
}
