//! What a profile adds to the command of a run: the model, the environment it must not inherit,
//! and the form that resumes an earlier session.

use super::AgentProfile;

impl AgentProfile {
    /// The placeholder in `resume` for the agent's own session id.
    pub const SESSION: &'static str = "{session}";

    /// The arguments that run the command on `model` (`--model <model>`, from the CLI reference
    /// of Claude Code): none without a model, for a profile that does not run Claude Code, or
    /// when the profile's own command already names one, which then stays the profile's choice.
    pub fn model_args(&self, model: Option<&str>) -> Vec<String> {
        let Some(model) = model.map(str::trim).filter(|m| !m.is_empty()) else {
            return Vec::new();
        };
        let named = self
            .command
            .iter()
            .any(|a| a == "--model" || a.starts_with("--model="));
        if !self.runs_claude_code() || named {
            return Vec::new();
        }
        vec!["--model".to_owned(), model.to_owned()]
    }

    /// The inherited environment variables of the agent's own kind that must not reach it:
    /// `CLAUDECODE` and `CLAUDE_CODE_*` describe the session that started Toknila (a child-session
    /// marker, the entrypoint), not this run. A variable the profile's `env` sets is its choice and
    /// stays. `names` are the names in the daemon's environment.
    pub fn inherited_to_drop<I: IntoIterator<Item = String>>(&self, names: I) -> Vec<String> {
        let same = |a: &str, b: &str| a.eq_ignore_ascii_case(b);
        names
            .into_iter()
            .filter(|n| {
                let up = n.to_ascii_uppercase();
                (up == "CLAUDECODE" || up.starts_with("CLAUDE_CODE_"))
                    && !self.env.keys().any(|k| same(k, n))
            })
            .collect()
    }

    /// The command that continues the agent's earlier `session` with `input` as its first
    /// message: the profile's command with its `resume` arguments after the program and `input`
    /// where the kickoff goes. `None` when the profile has no resume form, so the run starts
    /// fresh instead.
    pub fn resume_argv(&self, session: &str, input: &str) -> Option<Vec<String>> {
        if self.resume.is_empty() || session.trim().is_empty() {
            return None;
        }
        let mut argv = self.argv(input);
        let form = self
            .resume
            .iter()
            .map(|a| a.replace(Self::SESSION, session));
        argv.splice(1..1, form);
        Some(argv)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AgentRole;

    fn claude() -> AgentProfile {
        AgentProfile::claude_code()
    }

    #[test]
    fn claude_code_resumes_a_session_with_the_input_as_its_first_message() {
        let argv = claude().resume_argv("s-9", "yes, go on").expect("resumes");
        assert_eq!(
            argv,
            [
                "claude",
                "--resume",
                "s-9",
                "--permission-mode",
                "acceptEdits",
                "yes, go on"
            ]
        );
    }

    #[test]
    fn a_profile_without_a_resume_form_or_a_session_resumes_nothing() {
        let plain = AgentProfile::new("x", AgentRole::Worker, vec!["agent".to_owned()]);
        assert_eq!(plain.resume_argv("s-9", "hi"), None);
        assert_eq!(claude().resume_argv("  ", "hi"), None);
    }
}
