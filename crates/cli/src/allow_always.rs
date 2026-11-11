//! The line `tok ticket show` prints under a waiting permission question: what "Allow always"
//! would add, or why it adds nothing. Core makes the rule; this only words it.

use toknila_core::{StatusReason, WaitingReason};

/// Prints a waiting permission question's whole command, when its text was cut, and its
/// "Allow always" line.
pub fn print_request(r: Option<&StatusReason>) {
    if let Some(StatusReason::Waiting(WaitingReason::Permission {
        detail: Some(full), ..
    })) = r
    {
        println!("the full request:");
        full.lines().for_each(|line| println!("  {line}"));
    }
    if let Some(line) = allow_always_line(r) {
        println!("{line}");
    }
}

/// What "Allow always" would do for a waiting permission question: the rule it adds, or why it
/// adds none; nothing for a question that is not about a command.
pub fn allow_always_line(r: Option<&StatusReason>) -> Option<String> {
    let Some(StatusReason::Waiting(WaitingReason::Permission { text, detail })) = r else {
        return None;
    };
    let line = match toknila_core::command_policy::allow_always_for(detail.as_deref(), text)? {
        Ok(rule) => format!("Allow always: {rule}"),
        Err(why) => format!("Allow always: not offered, {why}"),
    };
    Some(line)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_waiting_permission_prints_the_rule_allow_always_adds_or_why_none() {
        let ask = |text: &str, detail: Option<&str>| {
            allow_always_line(Some(&StatusReason::Waiting(WaitingReason::Permission {
                text: text.to_owned(),
                detail: detail.map(ToOwned::to_owned),
            })))
        };
        assert_eq!(
            ask(
                "Claude Code asks for permission: Bash: cargo test -p x",
                None
            )
            .as_deref(),
            Some("Allow always: Bash(cargo test *)")
        );
        assert_eq!(
            ask("Bash: cd \"$W\"; ls", Some("Bash: cd \"$W\"; ls")).as_deref(),
            Some(
                "Allow always: not offered, a command built with $ or backticks cannot become a rule"
            )
        );
        assert_eq!(ask("Write: a.rs", None), None);
        let question = StatusReason::Waiting(WaitingReason::Question {
            text: "Bash: ls".into(),
        });
        assert_eq!(allow_always_line(Some(&question)), None);
    }
}
