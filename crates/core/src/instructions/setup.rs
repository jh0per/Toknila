//! What a worker is told about the project's setup command: the daemon ran it in the workspace
//! before the agent started, so the agent neither runs it again nor asks for it.

use super::RunBrief;

/// The section of the instructions file that names the setup command; empty without one.
pub(super) fn section(brief: &RunBrief<'_>) -> String {
    match brief.setup_command {
        Some(c) => format!(
            "\n## Workspace setup\n\nToknila already ran the project's setup command `{c}` in \
             your workspace before you started. Do not run it again and do not ask for it; \
             what it installs or generates is there.\n"
        ),
        None => String::new(),
    }
}

/// The sentence of the kickoff that says the setup ran; empty without a setup command.
pub(super) fn kickoff_part(brief: &RunBrief<'_>) -> String {
    brief
        .setup_command
        .map(|c| {
            format!(" The setup command `{c}` already ran in your workspace; do not run it again.")
        })
        .unwrap_or_default()
}
