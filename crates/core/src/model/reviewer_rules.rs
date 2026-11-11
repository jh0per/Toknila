//! What a reviewer may run beyond reading: the tests and checks of the project's own toolchain,
//! derived from the check command's tool and from what the repository holds. Read-only by
//! construction: nothing here writes to git, installs a package or leaves the ship. Pure.

/// The allow rules the toolchain `tool` (the first word of a command) adds to a reviewer; none for
/// a tool that has no test command of its own.
fn tool_rules(tool: &str) -> &'static [&'static str] {
    match tool {
        "cargo" => &["cargo test *", "cargo xtask *", "cargo clippy *"],
        "npm" => &["npm test *", "npm run *"],
        "pnpm" => &["pnpm test *", "pnpm run *"],
        "yarn" => &["yarn test *", "yarn run *"],
        "go" => &["go test *", "go vet *"],
        "pytest" => &["pytest *"],
        _ => &[],
    }
}

/// The first word of a check command, after a leading `cd`/`export` part; `None` when empty.
fn tool_of(command: &str) -> Option<&str> {
    let rest = super::command_policy::strip_leading(command);
    rest.split_whitespace().next()
}

/// The reviewer's extra allow rules (`Bash(cargo test *)`, ...): those of the check command's tool
/// and of each of `repo_tools` (the toolchains the repository holds, `cargo` for a `Cargo.toml`,
/// `npm` for a `package.json`). Each rule once, in a stable order.
pub fn reviewer_check_rules(check_command: Option<&str>, repo_tools: &[String]) -> Vec<String> {
    let mut tools: Vec<&str> = check_command.and_then(tool_of).into_iter().collect();
    tools.extend(repo_tools.iter().map(String::as_str));
    let mut out: Vec<String> = Vec::new();
    for tool in tools {
        for rule in tool_rules(tool) {
            let rule = format!("Bash({rule})");
            if !out.contains(&rule) {
                out.push(rule);
            }
        }
    }
    out
}

impl super::AgentProfile {
    /// The profile as a card's reviewer runs it: the tests and checks of the project's toolchain
    /// are allowed. Only a reviewer that runs Claude Code is changed.
    pub fn with_reviewer_checks(&self, check_command: Option<&str>, repo_tools: &[String]) -> Self {
        let mut out = self.clone();
        if self.role == super::AgentRole::Reviewer && self.runs_claude_code() {
            for rule in reviewer_check_rules(check_command, repo_tools) {
                if !out.permissions.allow.contains(&rule) {
                    out.permissions.allow.push(rule);
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AgentProfile;

    fn tools(t: &[&str]) -> Vec<String> {
        t.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn a_cargo_check_command_allows_test_xtask_and_clippy() {
        assert_eq!(
            reviewer_check_rules(Some("cargo xtask verify"), &[]),
            [
                "Bash(cargo test *)",
                "Bash(cargo xtask *)",
                "Bash(cargo clippy *)"
            ]
        );
    }

    #[test]
    fn a_cd_or_export_in_front_of_the_check_command_does_not_hide_its_tool() {
        assert_eq!(
            reviewer_check_rules(Some("cd \"app\"; export CI=1; npm test"), &[]),
            ["Bash(npm test *)", "Bash(npm run *)"]
        );
    }

    #[test]
    fn without_a_check_command_the_repository_decides_and_nothing_repeats() {
        assert!(reviewer_check_rules(None, &[]).is_empty());
        assert_eq!(
            reviewer_check_rules(None, &tools(&["cargo", "npm"])),
            [
                "Bash(cargo test *)",
                "Bash(cargo xtask *)",
                "Bash(cargo clippy *)",
                "Bash(npm test *)",
                "Bash(npm run *)"
            ]
        );
        assert_eq!(
            reviewer_check_rules(Some("cargo test"), &tools(&["cargo"])).len(),
            3
        );
    }

    #[test]
    fn an_unknown_tool_adds_nothing() {
        assert!(reviewer_check_rules(Some("./ci.sh"), &tools(&["bazel"])).is_empty());
    }

    #[test]
    fn no_derived_rule_writes_to_git_or_installs() {
        let all = reviewer_check_rules(
            Some("cargo xtask verify"),
            &tools(&["cargo", "npm", "pnpm", "yarn", "go", "pytest"]),
        );
        assert!(all.len() >= 10, "{all:?}");
        for rule in &all {
            for never in [
                "git", "install", " add", " ci", "commit", "push", "checkout",
            ] {
                assert!(!rule.contains(never), "{never} in {rule}");
            }
        }
    }

    #[test]
    fn only_a_claude_code_reviewer_gets_the_rules() {
        let reviewer = AgentProfile::claude_code_reviewer();
        let got = reviewer.with_reviewer_checks(Some("cargo xtask verify"), &[]);
        for want in [
            "Bash(cargo test *)",
            "Bash(cargo xtask *)",
            "Bash(cargo clippy *)",
        ] {
            assert!(got.permissions.allow.iter().any(|r| r == want), "{want}");
        }
        assert_eq!(
            got.with_reviewer_checks(Some("cargo xtask verify"), &[]),
            got,
            "twice changes nothing"
        );
        for other in [
            AgentProfile::claude_code(),
            AgentProfile::sim_reviewer(),
            AgentProfile::claude_code_planner(),
        ] {
            assert_eq!(other.with_reviewer_checks(Some("cargo test"), &[]), other);
        }
    }
}
