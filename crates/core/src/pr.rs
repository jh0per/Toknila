//! Pull-request mode: what the host learns about a pull request (its checks) and the rules that
//! turn it into a decision, the title and the body of the pull request a card opens.
//!
//! The host (`gh`, a fake in tests) only reports facts; whether checks are good enough to merge
//! is [`judge`], here.

use serde::{Deserialize, Serialize};

use crate::{Actor, MergeMethod, Project, Ticket, Verdict};

/// How one check of a pull request stands.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckState {
    /// Queued or running.
    Pending,
    /// Succeeded, or was skipped.
    Passed,
    /// Failed, timed out or was cancelled.
    Failed,
}

/// One check of a pull request.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckRun {
    pub name: String,
    pub state: CheckState,
}

/// The pull request a card opened, as last seen on the host.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PullRequest {
    pub number: u64,
    pub url: String,
    /// The checks at the last look; empty before any was reported.
    #[serde(default)]
    #[cfg_attr(feature = "openapi", schema(required = true))]
    pub checks: Vec<CheckRun>,
}

/// What the checks of a pull request add up to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChecksVerdict {
    /// Some check has not finished, or none has been reported yet (CI may not have started).
    Pending,
    /// Every check finished well: the pull request can be merged.
    Passed,
    /// Every check finished and these failed.
    Failed(Vec<String>),
}

/// Whether the pull request may merge. Waits until every check finished, so a failure names all
/// the checks that failed and not only the first.
pub fn judge(checks: &[CheckRun]) -> ChecksVerdict {
    if checks.is_empty() || checks.iter().any(|c| c.state == CheckState::Pending) {
        return ChecksVerdict::Pending;
    }
    let failed: Vec<String> = checks
        .iter()
        .filter(|c| c.state == CheckState::Failed)
        .map(|c| c.name.clone())
        .collect();
    if failed.is_empty() {
        ChecksVerdict::Passed
    } else {
        ChecksVerdict::Failed(failed)
    }
}

/// What a card's pull request is made from, decided here and executed by the host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrSpec {
    pub title: String,
    pub body: String,
    pub method: MergeMethod,
    /// The pull request already opened and being watched: only its checks are looked at,
    /// nothing is pushed or created. `None` (a merge starts: accept, unblock) pushes the branch
    /// and opens or reuses the pull request.
    pub watching: Option<PullRequest>,
}

impl PrSpec {
    /// The pull request of `ticket` under `project`'s settings.
    pub fn for_ticket(project: &Project, ticket: &Ticket) -> Self {
        Self {
            title: title(&project.merge.pr_title, ticket),
            body: body(ticket),
            method: project.merge.method,
            watching: ticket.pr.clone(),
        }
    }
}

/// The pull request title: the template with `{key}` and `{title}` filled in.
pub fn title(template: &str, ticket: &Ticket) -> String {
    template
        .replace("{key}", &ticket.key.to_string())
        .replace("{title}", &ticket.title)
}

/// The pull request body: the task, the acceptance items and a line per review round.
pub fn body(ticket: &Ticket) -> String {
    let mut out = format!("## Task\n\n{}\n", ticket.prompt.trim());
    if !ticket.acceptance.is_empty() {
        out.push_str("\n## Acceptance\n\n");
        for item in &ticket.acceptance {
            out.push_str(&format!("- {}\n", item.trim()));
        }
    }
    if !ticket.review.rounds.is_empty() {
        out.push_str("\n## Review\n\n");
        for r in &ticket.review.rounds {
            let who = match r.reviewer {
                Actor::Agent => "reviewer agent",
                _ => "person",
            };
            let verdict = match r.verdict {
                Verdict::Approve => "approved",
                Verdict::RequestChanges => "asked for changes",
            };
            out.push_str(&format!("{}. {who} {verdict}", r.number));
            if !r.comment.is_empty() {
                out.push_str(&format!(": {}", r.comment));
            }
            out.push('\n');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ChainId, TicketDraft, TicketId, Timestamp};

    fn check(name: &str, state: CheckState) -> CheckRun {
        CheckRun {
            name: name.to_owned(),
            state,
        }
    }

    fn ticket() -> Ticket {
        let draft = TicketDraft {
            title: "Add login".into(),
            prompt: "Let people sign in.".into(),
            acceptance: vec!["A form exists".into(), "Bad passwords are refused".into()],
            ..TicketDraft::default()
        };
        Ticket::new_backlog(
            TicketId::new(),
            ChainId::new(),
            "DEMO-3".parse().expect("key"),
            draft,
            Timestamp(1),
        )
    }

    #[test]
    fn checks_pass_only_when_all_finished_well_and_at_least_one_was_reported() {
        use CheckState::*;
        assert_eq!(judge(&[]), ChecksVerdict::Pending);
        assert_eq!(
            judge(&[check("a", Passed), check("b", Pending)]),
            ChecksVerdict::Pending
        );
        assert_eq!(judge(&[check("a", Passed)]), ChecksVerdict::Passed);
    }

    #[test]
    fn a_failure_waits_for_the_rest_and_names_every_failed_check() {
        use CheckState::*;
        let mixed = [check("a", Failed), check("b", Pending), check("c", Failed)];
        assert_eq!(judge(&mixed), ChecksVerdict::Pending);
        let done = [check("a", Failed), check("b", Passed), check("c", Failed)];
        assert_eq!(
            judge(&done),
            ChecksVerdict::Failed(vec!["a".into(), "c".into()])
        );
    }

    #[test]
    fn the_title_template_takes_the_key_and_the_title() {
        let t = ticket();
        assert_eq!(title("{key}: {title}", &t), "DEMO-3: Add login");
        assert_eq!(
            title("[{key}] {title} (toknila)", &t),
            "[DEMO-3] Add login (toknila)"
        );
    }

    #[test]
    fn the_body_has_the_task_the_acceptance_items_and_the_rounds() {
        let mut t = ticket();
        t.review.record(
            Actor::Agent,
            Verdict::RequestChanges,
            "add a test",
            Timestamp(2),
        );
        t.review
            .record(Actor::Human, Verdict::Approve, "", Timestamp(3));
        assert_eq!(
            body(&t),
            "## Task\n\nLet people sign in.\n\n## Acceptance\n\n- A form exists\n- Bad passwords are refused\n\n## Review\n\n1. reviewer agent asked for changes: add a test\n2. person approved\n"
        );
    }

    #[test]
    fn a_card_without_acceptance_or_rounds_has_the_task_only() {
        let mut t = ticket();
        t.acceptance.clear();
        assert_eq!(body(&t), "## Task\n\nLet people sign in.\n");
    }
}
