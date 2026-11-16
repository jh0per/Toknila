//! A daemon under a test root never starts a real agent: a test that reaches the built-in `claude`
//! profiles would start the owner's Claude Code on a Mac and block the card on CI.

use std::sync::atomic::Ordering;

use toknila_app::runner::RunnerError;
use toknila_core::AgentProfile;

use super::Runners;

/// The environment variable that marks a test daemon (see `silence`).
const TEST_ROOT: &str = "TOKNILA_TEST_ROOT";

/// The reason the card blocks with.
pub const REFUSAL: &str =
    "A test may not start a real agent: give the project --sim or a fake --profile";

/// Whether this process is a test daemon.
pub(super) fn under_test_root() -> bool {
    std::env::var_os(TEST_ROOT).is_some()
}

/// Fails with [`REFUSAL`] when `refuse` is set and `profile` is a built-in one that runs a program
/// (the real agent). A profile file's command (a test's fake agent) and the simulated profile pass.
fn check(refuse: bool, profile: &AgentProfile, from_file: bool) -> Result<(), RunnerError> {
    if refuse && !from_file && !profile.command.is_empty() {
        return Err(RunnerError::Spawn(REFUSAL.to_owned()));
    }
    Ok(())
}

impl Runners {
    /// Sets whether launching a built-in real agent is refused (on by default under a test root).
    pub fn set_refuse_real_agent(&self, refuse: bool) {
        self.refuse_real_agent.store(refuse, Ordering::Relaxed);
    }

    /// The profile `name`, read from the catalog, unless it is a real agent a test daemon refuses.
    pub(super) fn profile_to_start(&self, name: &str) -> Result<AgentProfile, RunnerError> {
        let listed = crate::profiles::load(&self.profiles)
            .profiles
            .into_iter()
            .find(|p| p.profile.name == name)
            .ok_or_else(|| RunnerError::UnknownProfile(name.to_owned()))?;
        let refuse = self.refuse_real_agent.load(Ordering::Relaxed);
        check(refuse, &listed.profile, listed.file.is_some())?;
        Ok(listed.profile)
    }
}
