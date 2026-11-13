//! What `tok show` says of a Waiting card: why it waits and, when its agent's process was ended
//! to free the slot, that the agent is paused and not running.

use toknila_core::Ticket;

/// The `reason:` line (`reason` is the status reason's text, empty when there is none) and, for a
/// card with a paused agent, an `agent:` line with what it waits for and how it comes back.
pub fn reason(t: &Ticket, queue_position: Option<u32>, reason: &str) {
    if !reason.is_empty() {
        println!("reason: {reason}");
    }
    if let Some(line) = agent_line(t, queue_position) {
        println!("{line}");
    }
}

/// `agent: paused, not running; waiting for ...`, or `None` while the agent is not paused.
pub fn agent_line(t: &Ticket, queue_position: Option<u32>) -> Option<String> {
    let paused = t.agent_paused.as_ref()?;
    let how = if !paused.resume {
        "answer it and the agent resumes".to_owned()
    } else {
        match queue_position {
            Some(n) => format!("answered; resumes when a slot is free (#{n} in line)"),
            None => "answered; resuming".to_owned(),
        }
    };
    Some(format!(
        "agent: paused, not running; waiting for {} ({how})",
        t.waiting_for()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use toknila_core::{PausedAgent, TicketDraft, Timestamp};

    fn paused(resume: bool) -> Ticket {
        let mut t = Ticket::new_backlog(
            toknila_core::TicketId::new(),
            toknila_core::ChainId::new(),
            "DEMO-1".parse().expect("key"),
            TicketDraft::default(),
            Timestamp(0),
        );
        t.status_reason = Some(toknila_core::StatusReason::Waiting(
            toknila_core::WaitingReason::Question {
                text: "which db?".to_owned(),
                choices: Vec::new(),
            },
        ));
        t.agent_paused = Some(PausedAgent {
            session: None,
            since: Timestamp(0),
            resume,
        });
        t
    }

    #[test]
    fn a_paused_agent_is_said_to_be_paused_with_what_it_waits_for() {
        assert_eq!(
            agent_line(&paused(false), None).as_deref(),
            Some(
                "agent: paused, not running; waiting for your answer to: which db? \
                 (answer it and the agent resumes)"
            )
        );
    }

    #[test]
    fn an_answered_card_without_a_slot_says_it_resumes_when_one_is_free() {
        let line = agent_line(&paused(true), Some(2)).expect("paused");
        assert!(
            line.contains("resumes when a slot is free (#2 in line)"),
            "{line}"
        );
    }

    #[test]
    fn a_card_whose_agent_runs_says_nothing() {
        let mut t = paused(false);
        t.agent_paused = None;
        assert_eq!(agent_line(&t, None), None);
    }
}
