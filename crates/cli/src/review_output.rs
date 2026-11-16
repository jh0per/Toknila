//! The review part of `tok show`: how the card is reviewed, the rounds, and what changed since a Return.

use toknila_api_types::TicketDetail;
use toknila_core::Actor;

use crate::check_words::check_words_now;
use crate::output::w;

/// `checked against main at 1a2b3c4, ship at 5d6e7f8`; `None` when the check recorded no main.
fn checked_line(check: &toknila_core::CheckOutcome) -> Option<String> {
    let base = check.base.as_deref()?;
    let ship = check
        .head
        .as_deref()
        .map(|h| format!(", ship at {}", short(h)))
        .unwrap_or_default();
    Some(format!("checked against main at {}{ship}", short(base)))
}

/// The first seven characters of a commit id.
fn short(commit: &str) -> &str {
    commit.get(..7).unwrap_or(commit)
}

/// How the card is reviewed, whose turn it is, the rounds so far, and the Since the Return block.
pub fn review(d: &TicketDetail) {
    let r = &d.review;
    let turn = match r.awaiting {
        Some(toknila_core::ReviewTurn::Person) => ", waiting for a person".to_owned(),
        Some(toknila_core::ReviewTurn::Agent { .. }) => ", waiting for the reviewer".to_owned(),
        Some(toknila_core::ReviewTurn::Merging) => ", merging".to_owned(),
        Some(toknila_core::ReviewTurn::Check) => {
            let words = check_words_now(r.check.as_ref());
            let words = if r.rechecking {
                words.replacen("being checked", "being checked again", 1)
            } else {
                words
            };
            format!(", {words}")
        }
        None => String::new(),
    };
    println!(
        "review: {}  reviewer: {}  rounds of the loop: {} of {}{turn}",
        w(&r.mode),
        r.reviewer,
        r.used_rounds,
        r.max_rounds
    );
    if let Some(line) = d.ticket.review.last_check.as_ref().and_then(checked_line) {
        println!("{line}");
    }
    for round in &r.rounds {
        let verdict = match round.verdict {
            toknila_core::Verdict::Approve => "approved",
            toknila_core::Verdict::RequestChanges => "asked for changes",
        };
        let who = match round.reviewer {
            Actor::Human => "person",
            Actor::System => "Toknila",
            Actor::Agent | Actor::Assistant => "reviewer agent",
        };
        let said = if round.comment.is_empty() {
            String::new()
        } else {
            format!(": {}", round.comment)
        };
        println!("  round {}: the {who} {verdict}{said}", round.number);
    }
    if let Some(since) = &d.ticket.review.since_return {
        print!("{}", since.text());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn outcome(base: Option<&str>, head: Option<&str>) -> toknila_core::CheckOutcome {
        toknila_core::CheckOutcome {
            command: "c".to_owned(),
            passed: true,
            output: String::new(),
            at: toknila_core::Timestamp(1),
            base: base.map(str::to_owned),
            head: head.map(str::to_owned),
            flaky: false,
            flaky_output: String::new(),
        }
    }

    #[test]
    fn the_round_says_which_main_the_check_saw() {
        let full = outcome(Some("1a2b3c4d5e6f"), Some("5d6e7f8a9b0c"));
        assert_eq!(
            checked_line(&full).as_deref(),
            Some("checked against main at 1a2b3c4, ship at 5d6e7f8")
        );
        assert_eq!(
            checked_line(&outcome(Some("abc"), None)).as_deref(),
            Some("checked against main at abc")
        );
        assert_eq!(checked_line(&outcome(None, Some("abc"))), None);
    }
}
