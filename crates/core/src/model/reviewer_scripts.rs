//! The scripts of the repository a hand-in's Checks lines run (`python3 -B .github/scripts/t.py`):
//! the exact commands a reviewer may run besides its toolchain's. Pure: whether a path exists and
//! is tracked is asked of the caller.

/// The interpreters whose script a reviewer may run.
const INTERPRETERS: [&str; 4] = ["python3", "node", "sh", "bash"];

/// A path of the repository, relative to the ship and made of plain characters only.
fn plain_relative(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with(['/', '-'])
        && !path
            .split('/')
            .any(|part| part.is_empty() || part == ".." || part == ".")
        && path
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '/' | '-'))
}

/// The command a Checks line names, when it is one interpreter run on one path: `python3 -B
/// <path>` (the `-B` forced), `python3 <path>`, `node <path>`, `sh <path>` or `bash <path>`.
fn script_command(check: &str) -> Option<(String, String)> {
    let check = check.trim().trim_matches('`').trim();
    // No shell syntax, no quoting, no expansion: one plain command.
    if check.contains([
        ';', '&', '|', '>', '<', '`', '$', '"', '\'', '\\', '*', '?', '(', '{',
    ]) {
        return None;
    }
    let mut words = check.split_whitespace();
    let interpreter = words.next().filter(|w| INTERPRETERS.contains(w))?;
    let mut path = words.next()?;
    if interpreter == "python3" && path == "-B" {
        path = words.next()?;
    }
    if words.next().is_some() || !plain_relative(path) {
        return None;
    }
    let command = if interpreter == "python3" {
        format!("python3 -B {path}")
    } else {
        format!("{interpreter} {path}")
    };
    Some((command, path.to_owned()))
}

/// The exact commands the Checks lines of `hand_in` name that run a repository file with an
/// interpreter; `tracked` says whether a path exists in the ship and is tracked by git. Each
/// command once, in the order of the hand-in.
pub fn hand_in_scripts(hand_in: &str, tracked: &dyn Fn(&str) -> bool) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for line in crate::handin::sections(hand_in).checks.lines() {
        if !crate::handin::is_check_line(line) {
            continue;
        }
        let line = line
            .trim_start()
            .trim_start_matches(['-', '*'])
            .trim_start();
        let Some(rest) = crate::handin::STATUSES
            .iter()
            .find_map(|s| line.strip_prefix(s))
        else {
            continue;
        };
        let rest = rest.trim_start_matches(':').trim_start();
        let check = rest.split_once(": ").map_or(rest, |(c, _)| c);
        if let Some((command, path)) = script_command(check)
            && tracked(&path)
            && !out.contains(&command)
        {
            out.push(command);
        }
    }
    out
}

/// The allow rule of one such command.
pub fn script_rule(command: &str) -> String {
    format!("Bash({command})")
}

impl super::AgentProfile {
    /// The profile as a reviewer runs it with `scripts` (from [`hand_in_scripts`]) allowed, each by
    /// its exact command. Only a reviewer that runs Claude Code is changed.
    pub fn with_reviewer_scripts(&self, scripts: &[String]) -> Self {
        let mut out = self.clone();
        if self.role == super::AgentRole::Reviewer && self.runs_claude_code() {
            for rule in scripts.iter().map(|s| script_rule(s)) {
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

    fn hand_in(checks: &str) -> String {
        format!("**What changed:** x\n**Checks:**\n{checks}\n**Left:** nothing")
    }

    fn all(_: &str) -> bool {
        true
    }

    #[test]
    fn a_python_script_line_gives_one_exact_rule_with_b_forced() {
        let h = hand_in(
            "- PASS `python3 -B tools/t.py`: ok\n\
             - PASS node a/b.js: ok\n- PASS sh ci/run.sh: ok\n- PASS bash ci/run.sh: ok",
        );
        assert_eq!(
            hand_in_scripts(&h, &all),
            [
                "python3 -B tools/t.py",
                "node a/b.js",
                "sh ci/run.sh",
                "bash ci/run.sh"
            ]
        );
        let one = hand_in("- PASS python3 scripts/test_wait.py: 3 passed");
        assert_eq!(
            hand_in_scripts(&one, &all),
            ["python3 -B scripts/test_wait.py"]
        );
        assert_eq!(
            script_rule("python3 -B scripts/t.py"),
            "Bash(python3 -B scripts/t.py)"
        );
    }

    #[test]
    fn a_dot_folder_like_github_is_allowed_inside_the_path() {
        let h = hand_in("- PASS python3 -B .github/scripts/test_wait_for_ci.py: ok");
        assert_eq!(
            hand_in_scripts(&h, &all),
            ["python3 -B .github/scripts/test_wait_for_ci.py"]
        );
    }

    #[test]
    fn the_refused_shapes_give_no_rule() {
        for line in [
            "python3 -c print(1)",
            "python3 -c \"print(1)\"",
            "python3 /etc/evil.py",
            "python3 ../up/x.py",
            "python3 -B ~/x.py",
            "python3 x.py && rm y",
            "python3 x.py; ls",
            "python3 x.py | tail",
            "python3 x.py > out.txt",
            "python3 `cat x`.py",
            "python3 $(echo x).py",
            "python3 x.py --flag",
            "python3 *.py",
            "ruby x.rb",
            "cargo test x",
            "python3",
        ] {
            let h = hand_in(&format!("- PASS {line}: some evidence of it"));
            assert!(hand_in_scripts(&h, &all).is_empty(), "{line}");
        }
        // control: the same shape with a plain script is accepted
        let h = hand_in("- PASS python3 x.py: some evidence of it");
        assert_eq!(hand_in_scripts(&h, &all), ["python3 -B x.py"]);
    }

    #[test]
    fn an_untracked_or_missing_file_gives_no_rule() {
        let h = hand_in("- PASS python3 -B scripts/t.py: ok\n- PASS node b.js: ok");
        assert_eq!(
            hand_in_scripts(&h, &|p| p == "b.js"),
            ["node b.js"],
            "only the tracked one"
        );
        assert!(hand_in_scripts(&h, &|_| false).is_empty());
    }

    #[test]
    fn only_a_claude_code_reviewer_gets_the_script_rules() {
        let scripts = vec!["python3 -B scripts/t.py".to_owned()];
        let got = AgentProfile::claude_code_reviewer().with_reviewer_scripts(&scripts);
        assert!(
            got.permissions
                .allow
                .contains(&"Bash(python3 -B scripts/t.py)".to_owned())
        );
        assert_eq!(got.with_reviewer_scripts(&scripts), got);
        let worker = AgentProfile::claude_code();
        assert_eq!(worker.with_reviewer_scripts(&scripts), worker);
    }
}
