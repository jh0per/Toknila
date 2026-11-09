//! Whether an agent profile tells a new process its brief.

use super::AgentProfile;

impl AgentProfile {
    /// Whether the agent is told its brief when it starts: the command takes the kickoff
    /// message, or a generated instructions file. An agent that gets neither is told what a
    /// person said by typing it, as before.
    pub fn reads_brief(&self) -> bool {
        self.instructions_flag.is_some() || self.command.iter().any(|a| a.contains(Self::KICKOFF))
    }
}
