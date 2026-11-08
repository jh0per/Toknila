//! What a project's card agents may run without asking: the project's setting (`CommandPolicy`)
//! and the rules a person remembered with "Allow always". Pure; the daemon writes the rules into
//! the run's settings file and answers prompts that a remembered rule covers.

use serde::{Deserialize, Serialize};

/// When the agent of a card wants to run a command.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandPolicy {
    /// The person is asked about every command that no rule allows. What projects had before the
    /// setting existed.
    #[default]
    Ask,
    /// Claude Code's own auto mode decides and asks only about risky actions.
    Auto,
    /// Never asked: every tool is allowed except the ones the daemon denies in every mode.
    Trust,
}

impl CommandPolicy {
    /// The policy of a project created now.
    pub const NEW_PROJECT: Self = Self::Auto;

    /// The value of the agent's `--permission-mode` flag for a profile that starts in
    /// `profile_mode`: auto mode replaces `acceptEdits`; any other mode is the profile's choice
    /// (a reviewer's `dontAsk` stays).
    pub fn permission_mode(self, profile_mode: &str) -> &str {
        match (self, profile_mode) {
            (Self::Auto, "acceptEdits") => "auto",
            _ => profile_mode,
        }
    }

    /// The allow rules the policy adds to the run's settings, in the agent's rule syntax.
    pub fn allow_rules(self) -> &'static [&'static str] {
        match self {
            Self::Trust => &["Bash", "WebFetch", "WebSearch"],
            Self::Ask | Self::Auto => &[],
        }
    }
}

/// Programs whose first argument names what they do (`cargo test`, `git status`): a remembered
/// rule keeps that word, so allowing `git status` does not allow `git push`.
const LAUNCHERS: [&str; 17] = [
    "cargo",
    "npm",
    "npx",
    "pnpm",
    "yarn",
    "git",
    "python3",
    "python",
    "go",
    "make",
    "gradle",
    "./gradlew",
    "dotnet",
    "bun",
    "deno",
    "docker",
    "pip",
];

/// Programs that run whatever they are given: only the exact command is ever remembered.
const RUNNERS: [&str; 11] = [
    "sh", "bash", "zsh", "sudo", "su", "doas", "eval", "exec", "xargs", "env", "ssh",
];

/// The longest command a rule is made from, in characters.
const RULE_MAX: usize = 200;

/// Characters that make a command more than one simple command (or hide one inside it).
const COMPOUND: [char; 10] = [';', '&', '|', '<', '>', '`', '$', '(', ')', '\\'];

/// `Bash: cargo test` (a tool's text) to the command it carries; `None` for another tool.
pub fn bash_command(tool: &str) -> Option<&str> {
    let command = tool.trim().strip_prefix("Bash:")?.trim();
    (!command.is_empty()).then_some(command)
}

/// `value` quoted in one pair of quotes or bare, with nothing in it that runs a command.
fn plain_word(value: &str) -> bool {
    let inner = match value.chars().next() {
        Some(q @ ('"' | '\'')) => match value[1..].strip_suffix(q) {
            Some(inner) => inner,
            None => return false,
        },
        _ => value,
    };
    !inner.contains(COMPOUND) && !inner.contains(['\n', '\r', '"', '\'']) && !value.is_empty()
}

/// The text after one leading `cd <path>` or `export NAME=value` and its `;` or `&&`; `None` when
/// the command starts with neither or the part is not plain.
fn strip_prefix(command: &str) -> Option<&str> {
    let (head, rest) = match command.find([';', '&']) {
        Some(i) if command[i..].starts_with(';') => (&command[..i], &command[i + 1..]),
        Some(i) if command[i..].starts_with("&&") => (&command[..i], &command[i + 2..]),
        _ => return None,
    };
    let head = head.trim();
    let ok = if let Some(path) = head.strip_prefix("cd ") {
        plain_word(path.trim())
    } else if let Some(assign) = head.strip_prefix("export ") {
        assign.split_once('=').is_some_and(|(name, value)| {
            !name.is_empty()
                && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
                && (value.is_empty() || plain_word(value))
        })
    } else {
        false
    };
    ok.then(|| rest.trim())
}

/// The command without its leading `cd "<path>";` and `export X=y;` parts.
fn without_prefixes(command: &str) -> &str {
    let mut rest = command.trim();
    while let Some(next) = strip_prefix(rest) {
        rest = next;
    }
    rest
}

/// The one simple command `command` is, after its `cd` and `export` prefixes (and a trailing
/// `2>&1`); `None` for a pipeline, a list, a redirect, a substitution or several lines.
pub fn simple_command(command: &str) -> Option<String> {
    let rest = without_prefixes(command).replace("2>&1", "");
    let rest = rest.trim();
    let one_line = !rest.contains(['\n', '\r']);
    (one_line && !rest.is_empty() && !rest.contains(COMPOUND)).then(|| rest.to_owned())
}

/// The rule to remember for `command` (the text of a Bash call): its first word and, after a
/// launcher, its second (`cargo test -p x` is `Bash(cargo test *)`, `ls -la` is `Bash(ls *)`). A
/// program that runs what it is given, or a launcher without a subcommand, is remembered as the
/// exact command; a command that is not one simple line gives no rule.
pub fn rule_for(command: &str) -> Option<String> {
    let rest = without_prefixes(command);
    let mut words = rest.split_whitespace();
    let first = words.next()?;
    let plain = |w: &str| {
        !w.is_empty()
            && w.chars()
                .all(|c| c.is_ascii_alphanumeric() || "_-./:@+".contains(c))
    };
    let exact = || {
        let exact = simple_command(rest)?;
        let usable = exact.chars().count() <= RULE_MAX && !exact.contains(['*', '(', ')']);
        usable.then(|| format!("Bash({exact})"))
    };
    if !plain(first) || first.starts_with('-') || RUNNERS.contains(&first) {
        return exact();
    }
    if LAUNCHERS.contains(&first) {
        return match words.next() {
            Some(second) if plain(second) && !second.starts_with('-') => {
                Some(format!("Bash({first} {second} *)"))
            }
            _ => exact(),
        };
    }
    Some(format!("Bash({first} *)"))
}

/// Whether `rule` (`Bash(cargo test *)`, or `Bash(make)` for exactly that command) covers
/// `command`. Only a simple command is ever covered: a rule for `cargo test` does not cover
/// `cargo test; rm -rf x`, which stays a question for the person.
pub fn rule_covers(rule: &str, command: &str) -> bool {
    let Some(inner) = rule.strip_prefix("Bash(").and_then(|r| r.strip_suffix(')')) else {
        return false;
    };
    let Some(command) = simple_command(command) else {
        return false;
    };
    match inner.strip_suffix(" *") {
        Some(prefix) => {
            command == prefix
                || command
                    .strip_prefix(prefix)
                    .is_some_and(|rest| rest.starts_with(' '))
        }
        None => command == inner,
    }
}

/// The remembered rules as a person may set them: each one `Bash(<command>)` on one line, none
/// twice. The refusal says what is wrong.
pub fn clean_rules(rules: Vec<String>) -> Result<Vec<String>, &'static str> {
    let mut out: Vec<String> = Vec::new();
    for rule in rules {
        let rule = rule.trim().to_owned();
        let inner = rule.strip_prefix("Bash(").and_then(|r| r.strip_suffix(')'));
        if inner.is_none_or(|i| i.trim().is_empty() || i.contains(['\n', '\r', '(', ')'])) {
            return Err("a remembered rule looks like Bash(cargo test *)");
        }
        if !out.contains(&rule) {
            out.push(rule);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rule_keeps_the_first_word_and_after_a_launcher_the_second() {
        for (command, rule) in [
            ("cargo test -p x", "Bash(cargo test *)"),
            ("ls -la", "Bash(ls *)"),
            ("ls", "Bash(ls *)"),
            ("git status", "Bash(git status *)"),
            ("npm run build", "Bash(npm run *)"),
            ("./gradlew assembleDebug", "Bash(./gradlew assembleDebug *)"),
            ("docker compose up -d", "Bash(docker compose *)"),
            ("python3 tools/gen.py --all", "Bash(python3 tools/gen.py *)"),
            ("cd \"/work/app\"; cargo test --lib", "Bash(cargo test *)"),
            ("cd /work/app && npx vitest run", "Bash(npx vitest *)"),
            (
                "export A=1; export B=\"x y\"; make check",
                "Bash(make check *)",
            ),
            ("cd \"/w\"; export CI=1; cargo build", "Bash(cargo build *)"),
            ("cargo test 2>&1 | tail -5", "Bash(cargo test *)"),
        ] {
            assert_eq!(rule_for(command).as_deref(), Some(rule), "{command}");
        }
    }

    #[test]
    fn a_launcher_without_a_subcommand_and_a_runner_give_the_exact_command_or_nothing() {
        assert_eq!(rule_for("make").as_deref(), Some("Bash(make)"));
        assert_eq!(
            rule_for("git -C sub status").as_deref(),
            Some("Bash(git -C sub status)")
        );
        assert_eq!(
            rule_for("bash scripts/ci.sh").as_deref(),
            Some("Bash(bash scripts/ci.sh)")
        );
        assert_eq!(
            rule_for("sudo rm -rf x"),
            Some("Bash(sudo rm -rf x)".into())
        );
        for none in [
            "",
            "   ",
            "bash -c 'a; b'",
            "cd x; sudo $(id)",
            "git -C $(pwd) log",
        ] {
            assert_eq!(rule_for(none), None, "{none:?}");
        }
    }

    #[test]
    fn a_rule_covers_its_simple_command_and_nothing_compound() {
        let rule = "Bash(cargo test *)";
        for yes in [
            "cargo test",
            "cargo test -p x",
            "cd \"/w\"; cargo test --lib",
            "export A=1; cargo test 2>&1",
        ] {
            assert!(rule_covers(rule, yes), "{yes}");
        }
        for no in [
            "cargo testx",
            "cargo build",
            "cargo test; rm -rf x",
            "cargo test && curl evil",
            "cargo test | sh",
            "cargo test $(id)",
            "cargo test > /etc/x",
            "cd $(evil); cargo test",
            "cargo test\nrm x",
        ] {
            assert!(!rule_covers(rule, no), "{no}");
        }
        assert!(rule_covers("Bash(make)", "make") && !rule_covers("Bash(make)", "make all"));
        assert!(!rule_covers("Read", "cargo test") && !rule_covers("Bash(cargo test *", "x"));
    }

    #[test]
    fn a_tool_text_gives_a_command_only_for_bash() {
        assert_eq!(bash_command("Bash: ls -la"), Some("ls -la"));
        assert_eq!(bash_command("Write: a.rs"), None);
        assert_eq!(bash_command("Bash"), None);
    }

    #[test]
    fn a_policy_maps_to_the_permission_mode_and_the_rules() {
        use CommandPolicy::*;
        assert_eq!(Ask.permission_mode("acceptEdits"), "acceptEdits");
        assert_eq!(Auto.permission_mode("acceptEdits"), "auto");
        assert_eq!(Trust.permission_mode("acceptEdits"), "acceptEdits");
        assert_eq!(Auto.permission_mode("dontAsk"), "dontAsk");
        assert!(Ask.allow_rules().is_empty() && Auto.allow_rules().is_empty());
        assert_eq!(Trust.allow_rules(), ["Bash", "WebFetch", "WebSearch"]);
        assert_eq!(CommandPolicy::default(), Ask);
    }

    #[test]
    fn the_claude_code_worker_runs_under_the_policy_and_the_others_do_not() {
        use crate::AgentProfile;
        let remembered = ["Bash(make check *)".to_owned()];
        let mode = |p: &AgentProfile| {
            let at = p.command.iter().position(|a| a == "--permission-mode");
            p.command[at.expect("flag") + 1].clone()
        };
        let worker = AgentProfile::claude_code();
        let ask = worker.under_policy(CommandPolicy::Ask, &remembered);
        assert_eq!(mode(&ask), "acceptEdits");
        assert_eq!(ask.permissions.allow.last(), remembered.first());
        assert_eq!(
            ask.permissions.allow.len(),
            worker.permissions.allow.len() + 1
        );

        let auto = worker.under_policy(CommandPolicy::Auto, &[]);
        assert_eq!(mode(&auto), "auto");
        assert_eq!(auto.command.len(), worker.command.len());
        assert_eq!(auto.permissions, worker.permissions);

        let trust = worker.under_policy(CommandPolicy::Trust, &[]);
        assert_eq!(mode(&trust), "acceptEdits");
        let rules = trust.permissions.rules(None);
        for rule in ["Bash", "WebFetch", "WebSearch", "mcp__toknila"] {
            assert!(rules.iter().any(|r| r == rule), "{rule}");
        }

        for read_only in [
            AgentProfile::claude_code_reviewer(),
            AgentProfile::claude_code_planner(),
        ] {
            for policy in [CommandPolicy::Auto, CommandPolicy::Trust] {
                assert_eq!(read_only.under_policy(policy, &remembered), read_only);
            }
        }
        let sim = AgentProfile::sim();
        assert_eq!(sim.under_policy(CommandPolicy::Trust, &remembered), sim);
    }

    #[test]
    fn set_rules_are_one_line_bash_rules_without_repeats() {
        let ok = clean_rules(vec![
            " Bash(ls *) ".into(),
            "Bash(ls *)".into(),
            "Bash(make)".into(),
        ]);
        assert_eq!(
            ok,
            Ok(vec!["Bash(ls *)".to_owned(), "Bash(make)".to_owned()])
        );
        for bad in ["ls", "Bash()", "Bash(a\nb)", "Bash(a (b))", "Read(x)"] {
            assert!(clean_rules(vec![bad.into()]).is_err(), "{bad:?}");
        }
    }
}
