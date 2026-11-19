//! `tok inbox`: what waits on a person.

use anyhow::Result;
use toknila_client::Client;
use toknila_core::inbox::InboxItem;

use super::{default_project, emit};
use crate::output::w;

pub(super) fn run(c: &Client, project: Option<String>, json_out: bool) -> Result<()> {
    let project = default_project(c, project)?;
    let items = c.inbox(&project)?;
    emit(json_out, &items, |items| print!("{}", render(items)));
    Ok(())
}

/// The whole listing: the person's items first, then the informational rows (an answer that
/// waits for a slot) with the same words the window shows. It says so when nothing needs the
/// person.
fn render(items: &[InboxItem]) -> String {
    let mut out = String::new();
    if !items.iter().any(|i| i.needs_human) {
        out.push_str("nothing waits on you\n");
    }
    // The daemon sorts the person's items first; this keeps that true for any source.
    for i in items
        .iter()
        .filter(|i| i.needs_human)
        .chain(items.iter().filter(|i| !i.needs_human))
    {
        out.push_str(&lines(i));
    }
    out
}

/// One item as text: its kind, card and title, then what it carries (a review's gist and checks).
fn lines(i: &InboxItem) -> String {
    let card = i
        .ticket
        .as_ref()
        .map_or_else(|| "-".to_owned(), ToString::to_string);
    let mut out = format!("{}  {}  {}\n", w(&i.kind), card, i.title);
    if let Some(h) = &i.hand_in {
        out.push_str(&format!("  gist: {}\n", h.gist));
        let checks = h.checks_line();
        if !checks.is_empty() {
            out.push_str(&format!("  checks: {checks}\n"));
        }
        if !h.person.is_empty() {
            out.push_str("  for you:\n");
            for step in &h.person {
                out.push_str(&format!("    - {step}\n"));
            }
        }
    } else if !i.text.is_empty() {
        out.push_str(&format!("  {}\n", i.text));
    }
    if i.kind == toknila_core::inbox::InboxKind::Flaky
        && let Some(first) = &i.detail
    {
        out.push_str("  the first run:\n");
        for l in first.lines() {
            out.push_str(&format!("    {l}\n"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use toknila_core::inbox::{InboxData, compute};
    use toknila_core::{
        ChainId, PausedAgent, StatusReason, Ticket, TicketDraft, TicketId, TicketStatus, Timestamp,
        WaitingReason,
    };

    fn asking(n: u32, answered: Option<bool>) -> Ticket {
        let mut t = Ticket::new_backlog(
            TicketId::new(),
            ChainId::new(),
            format!("DEMO-{n}").parse().expect("key"),
            TicketDraft {
                title: format!("card {n}"),
                ..TicketDraft::default()
            },
            Timestamp(i64::from(n)),
        );
        t.status = TicketStatus::Waiting;
        t.status_reason = Some(StatusReason::Waiting(WaitingReason::Question {
            text: "which db?".to_owned(),
            choices: Vec::new(),
        }));
        t.agent_paused = answered.map(|resume| PausedAgent {
            session: None,
            since: Timestamp(0),
            resume,
            lost: false,
            go_on: None,
        });
        t
    }

    fn listing(tickets: Vec<Ticket>, line: &[(usize, u32)]) -> String {
        let positions = line.iter().map(|(i, n)| (tickets[*i].id, *n)).collect();
        let items = compute(&InboxData {
            tickets,
            positions,
            ..InboxData::default()
        });
        render(&items)
    }

    #[test]
    fn an_answered_question_prints_after_the_persons_items_with_its_place_in_line() {
        // The answered card is newer, yet it prints last.
        let out = listing(vec![asking(9, Some(true)), asking(2, None)], &[(0, 2)]);
        assert_eq!(
            out,
            "question  DEMO-2  card 2\n  which db?\n\
             answered  DEMO-9  card 9\n  Answered; the agent gets it when a slot is free (#2 in line)\n"
        );
    }

    #[test]
    fn a_flaky_item_prints_the_first_failing_line_and_the_first_run_output() {
        let mut t = asking(4, None);
        t.status = TicketStatus::Running;
        t.status_reason = None;
        t.review.last_check = Some(toknila_core::CheckOutcome {
            command: "cargo test".to_owned(),
            passed: true,
            output: "ok".to_owned(),
            at: Timestamp(5),
            base: None,
            head: None,
            flaky: true,
            flaky_output: "running 2 tests\ntest einval ... FAILED\ntest result: FAILED".to_owned(),
        });
        let items = compute(&InboxData {
            tickets: vec![t],
            ..InboxData::default()
        });
        let out = render(&items);
        assert_eq!(
            out,
            "nothing waits on you\n\
             flaky  DEMO-4  card 4\n  \
             A test failed once and passed on the second run while checking DEMO-4: test einval ... FAILED\n  \
             the first run:\n    running 2 tests\n    test einval ... FAILED\n    test result: FAILED\n"
        );
    }

    #[test]
    fn with_only_an_answered_row_it_says_nothing_waits_on_you_and_prints_the_row() {
        let out = listing(vec![asking(1, Some(true))], &[(0, 1)]);
        assert_eq!(
            out,
            "nothing waits on you\n\
             answered  DEMO-1  card 1\n  Answered; the agent gets it when a slot is free (#1 in line)\n"
        );
    }
}
