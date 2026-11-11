//! What a reviewer's effective rules (its allow list against the deny list every role gets)
//! let through, judged the way Claude Code judges a Bash call in `dontAsk` mode: the deny rules
//! first, then every part of a compound command must match an allow rule, and a command that
//! starts with `VAR=value`, `cd` or `sh -c` matches none of the toolchain rules.

use serde_json::Value;
use toknila_core::{AgentProfile, AgentRole};

use super::Rig;

/// Whether `text` matches `pattern`, where `*` stands for any text.
fn glob(pattern: &str, text: &str) -> bool {
    let parts: Vec<&str> = pattern.split('*').collect();
    let (first, rest) = parts.split_first().expect("a split has a part");
    let Some(mut at) = text.strip_prefix(first).map(|t| text.len() - t.len()) else {
        return false;
    };
    for (i, part) in rest.iter().enumerate() {
        if i == rest.len() - 1 {
            return text.len() - at >= part.len() && text.ends_with(part);
        }
        match text[at..].find(part) {
            Some(p) => at += p + part.len(),
            None => return false,
        }
    }
    at == text.len()
}

/// The text of a `Bash(...)` rule.
fn bash(rule: &str) -> Option<&str> {
    rule.strip_prefix("Bash(").and_then(|r| r.strip_suffix(')'))
}

/// The parts of a command Claude Code checks one by one.
fn parts(command: &str) -> Vec<String> {
    command
        .replace("&&", "\n")
        .replace("||", "\n")
        .replace([';', '|'], "\n")
        .lines()
        .map(|p| p.trim().to_owned())
        .filter(|p| !p.is_empty())
        .collect()
}

struct Rules {
    allow: Vec<String>,
    deny: Vec<String>,
}

impl Rules {
    fn of_reviewer(check: &str) -> Rules {
        let mut rig = Rig::new();
        rig.brief.check_command = Some(check.to_owned());
        rig.brief.repo_tools = vec!["cargo".to_owned()];
        // A data home with entries beside the run's own folder, as a real one has.
        for d in ["db", "tokens", "logs", "runs/t2"] {
            std::fs::create_dir_all(rig.dir.path().join(d)).expect("mkdir");
        }
        rig.prepare_as(&AgentProfile::claude_code_reviewer(), AgentRole::Reviewer);
        let (_, text) = rig.file_of("reviewer", "settings.json");
        let v: Value = serde_json::from_str(&text).expect("json");
        let list = |k: &str| -> Vec<String> {
            v["permissions"][k]
                .as_array()
                .expect("rules")
                .iter()
                .map(|r| r.as_str().expect("rule").to_owned())
                .collect()
        };
        Rules {
            allow: list("allow"),
            deny: list("deny"),
        }
    }

    /// Whether a deny rule catches `command`, as a whole or in one of its parts.
    fn denied_by(&self, command: &str) -> Option<&str> {
        let all: Vec<String> = std::iter::once(command.to_owned())
            .chain(parts(command))
            .collect();
        self.deny
            .iter()
            .find(|r| bash(r).is_some_and(|p| all.iter().any(|c| glob(p, c))))
            .map(String::as_str)
    }

    fn allows(&self, command: &str) -> bool {
        self.denied_by(command).is_none()
            && parts(command).iter().all(|part| {
                self.allow
                    .iter()
                    .filter_map(|r| bash(r))
                    .any(|p| glob(p, part))
            })
    }
}

#[test]
fn the_matcher_can_say_no_and_yes() {
    assert!(glob("cargo test *", "cargo test -p x"));
    assert!(!glob("cargo test *", "cargo build"));
    assert!(glob("*/home/x/.ssh*", "cat /home/x/.ssh/id"));
    assert_eq!(
        parts("cd x && cargo test | tail -3"),
        ["cd x", "cargo test", "tail -3"]
    );
}

#[test]
fn a_cargo_reviewer_may_run_its_toolchain_plain_and_nothing_wrapped() {
    let rules = Rules::of_reviewer("cargo xtask verify");
    assert!(
        rules.deny.len() > 20,
        "the deny list is real: {:?}",
        rules.deny.len()
    );
    for ok in [
        "cargo test -p toknila-daemon designs",
        "cargo xtask verify",
        "git diff main...HEAD",
        "grep -n foo src/lib.rs",
    ] {
        assert!(
            rules.allows(ok),
            "{ok}: denied by {:?}; allow {:?}",
            rules.denied_by(ok),
            rules.allow
        );
    }
    assert!(
        rules.denied_by("cat ~/.ssh/id_ed25519").is_some(),
        "control: a private path is denied"
    );
    for refused in [
        "cd x && cargo test",
        "PATH=/x cargo test",
        "sh -c \"cargo test\"",
        "git commit -m x",
    ] {
        assert!(!rules.allows(refused), "{refused}");
    }
}
