//! The review part of `tok show`: how the card is reviewed, the rounds, and what changed since a Return.

use toknila_api_types::TicketDetail;
use toknila_core::Actor;

use crate::check_words::check_words_now;
use crate::output::w;

/// How the card is reviewed, whose turn it is, the rounds so far, and the Since the Return block.
pub fn review(d: &TicketDetail) {
    let r = &d.review;
    let turn = match r.awaiting {
        Some(toknila_core::ReviewTurn::Person) => ", waiting for a person".to_owned(),
        Some(toknila_core::ReviewTurn::Agent { .. }) => ", waiting for the reviewer".to_owned(),
        Some(toknila_core::ReviewTurn::Merging) => ", merging".to_owned(),
        Some(toknila_core::ReviewTurn::Check) => {
            format!(", {}", check_words_now(r.check.as_ref()))
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
    for round in &r.rounds {
        let verdict = match round.verdict {
            toknila_core::Verdict::Approve => "approved",
            toknila_core::Verdict::RequestChanges => "asked for changes",
        };
        let who = match round.reviewer {
            Actor::Human => "person",
            Actor::System => "Toknila",
            Actor::Agent => "reviewer agent",
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
