//! A run that continues a paused or lost agent's session instead of starting fresh.

use toknila_app::runner::RunBrief;
use toknila_core::AgentProfile;

/// The command that continues the paused agent's session with the answer as its first input, when
/// the brief asks for it and the profile has a resume form; `None` starts the run fresh.
pub(super) fn resumed_argv(profile: &AgentProfile, brief: &RunBrief) -> Option<Vec<String>> {
    let resume = brief.resume.as_ref()?;
    profile.resume_argv(resume.session.as_deref()?, &resume.input)
}

/// Whether the run's first message carries the brief's waiting instructions: the profile reads a
/// kickoff or an instructions file, or the answer is the first input of a resumed session.
pub(super) fn briefed(profile: &AgentProfile, brief: &RunBrief) -> bool {
    profile.reads_brief() || resumed_argv(profile, brief).is_some()
}
