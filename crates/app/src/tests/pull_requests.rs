//! Pull-request mode through the use cases: Accept queues a merge that carries the pull request's
//! title and body, the host's answers keep the pull request on the card, and a failed check, a
//! closed pull request or a conflict block it with a typed reason that unblock retries.

use toknila_core::{
    BlockedReason, CheckRun, CheckState, MergeMethod, MergeMode, PullRequest, ReviewTurn,
    TicketStatus,
};

use super::Mem;
use super::merges::{accept, answer, blocked_reason, in_review, unblock};
use super::runs::settle;
use crate::external::ExternalEffect;
use crate::{MergeError, MergeOutcome, merges, projects};

const URL: &str = "https://github.com/o/r/pull/7";

fn github() -> Mem {
    let mut mem = in_review();
    mem.project.merge.mode = MergeMode::Github;
    mem.project.merge.method = MergeMethod::Squash;
    mem.project.merge.pr_title = "[{key}] {title}".to_owned();
    mem
}

fn pr(state: CheckState) -> PullRequest {
    PullRequest::seen(
        7,
        URL.to_owned(),
        vec![CheckRun {
            name: "verify".to_owned(),
            state,
        }],
    )
}

fn asked(mem: &Mem) -> Vec<toknila_core::pr::PrSpec> {
    mem.merger
        .asked
        .borrow()
        .iter()
        .map(|s| s.pr.clone().expect("a pull request spec"))
        .collect()
}

#[test]
fn accept_in_github_mode_asks_for_a_pull_request_with_the_title_body_and_method() {
    let mut mem = github();
    accept(&mut mem);
    let Some(ExternalEffect::Merge {
        spec: Some(spec), ..
    }) = mem
        .deferred
        .iter()
        .find(|e| matches!(e, ExternalEffect::Merge { .. }))
    else {
        panic!("a merge is queued: {:?}", mem.deferred);
    };
    let pr = spec.pr.as_ref().expect("a pull request spec");
    assert_eq!(pr.title, "[DEMO-1] A");
    assert_eq!(pr.method, MergeMethod::Squash);
    assert!(pr.watching.is_none(), "the first step pushes and opens");
    assert!(pr.body.starts_with("## Task"), "{}", pr.body);
    assert!(pr.body.contains("1. person approved"), "{}", pr.body);
    assert_eq!(spec.base, "develop", "the pull request goes into the base");
}

#[test]
fn local_mode_asks_for_no_pull_request() {
    let mut mem = in_review();
    accept(&mut mem);
    settle(&mut mem);
    assert!(mem.merger.asked.borrow()[0].pr.is_none());
}

#[test]
fn an_open_pull_request_stays_on_the_card_and_the_ticker_only_looks_at_it() {
    let mut mem = github();
    answer(
        &mem,
        Ok(MergeOutcome::PrOpen {
            pr: pr(CheckState::Pending),
        }),
    );
    accept(&mut mem);
    settle(&mut mem);
    let a = mem.ticket_by_title("A");
    assert_eq!(
        (a.status, a.review.turn),
        (TicketStatus::Review, Some(ReviewTurn::Merging))
    );
    assert_eq!(a.pr, Some(pr(CheckState::Pending)));

    let keys = merges::poll(&mut mem).expect("poll");
    assert_eq!(keys.len(), 1);
    settle(&mut mem);
    let specs = asked(&mem);
    assert_eq!(specs.len(), 2);
    assert_eq!(specs[1].watching, Some(pr(CheckState::Pending)));
}

#[test]
fn an_unchanged_pull_request_is_not_a_new_event() {
    let mut mem = github();
    answer(
        &mem,
        Ok(MergeOutcome::PrOpen {
            pr: pr(CheckState::Pending),
        }),
    );
    accept(&mut mem);
    settle(&mut mem);
    let before = mem.events.len();
    merges::poll(&mut mem).expect("poll");
    settle(&mut mem);
    assert_eq!(
        mem.events.len(),
        before,
        "nothing changed, nothing journaled"
    );
}

#[test]
fn the_ticker_ignores_cards_without_a_pull_request_or_not_merging() {
    let mut mem = github();
    assert!(merges::poll(&mut mem).expect("poll").is_empty());
    // Accepted, the host has not answered yet: merging, but no pull request to look at.
    accept(&mut mem);
    assert!(merges::poll(&mut mem).expect("poll").is_empty());
    // The host answers with a failure: blocked, not merging.
    answer(&mem, Err(MergeError::GhNotLoggedIn));
    settle(&mut mem);
    assert!(merges::poll(&mut mem).expect("poll").is_empty());
}

#[test]
fn a_merged_pull_request_merges_the_card_and_keeps_its_link() {
    let mut mem = github();
    answer(
        &mem,
        Ok(MergeOutcome::PrMerged {
            pr: pr(CheckState::Passed),
        }),
    );
    accept(&mut mem);
    settle(&mut mem);
    let a = mem.ticket_by_title("A");
    assert_eq!(a.status, TicketStatus::Merged);
    assert_eq!(a.pr, Some(pr(CheckState::Passed)));
    assert_eq!(
        mem.yard.removed.borrow().len(),
        1,
        "the worktree is removed"
    );
}

#[test]
fn failing_checks_block_with_the_link_and_names_and_unblock_pushes_again() {
    let mut mem = github();
    answer(
        &mem,
        Ok(MergeOutcome::CiFailed {
            pr: pr(CheckState::Failed),
            failed: vec!["verify".to_owned()],
        }),
    );
    accept(&mut mem);
    settle(&mut mem);
    let a = mem.ticket_by_title("A");
    assert_eq!(a.status, TicketStatus::Blocked);
    assert_eq!(
        blocked_reason(&a),
        BlockedReason::CiFailed {
            url: URL.to_owned(),
            checks: vec!["verify".to_owned()]
        }
    );
    assert_eq!(a.pr, Some(pr(CheckState::Failed)));

    answer(
        &mem,
        Ok(MergeOutcome::PrMerged {
            pr: pr(CheckState::Passed),
        }),
    );
    unblock(&mut mem);
    settle(&mut mem);
    assert_eq!(mem.ticket_by_title("A").status, TicketStatus::Merged);
    assert!(
        asked(&mem)[1].watching.is_none(),
        "unblock runs the whole merge: the ship may have new commits to push"
    );
}

#[test]
fn a_closed_pull_request_and_a_github_conflict_block_with_the_link() {
    let mut mem = github();
    answer(
        &mem,
        Ok(MergeOutcome::PrClosed {
            url: URL.to_owned(),
        }),
    );
    accept(&mut mem);
    settle(&mut mem);
    assert_eq!(
        blocked_reason(&mem.ticket_by_title("A")),
        BlockedReason::PrClosed {
            url: URL.to_owned()
        }
    );

    answer(
        &mem,
        Ok(MergeOutcome::PrConflict {
            url: URL.to_owned(),
        }),
    );
    unblock(&mut mem);
    settle(&mut mem);
    let a = mem.ticket_by_title("A");
    assert_eq!(a.status, TicketStatus::Running, "back to the agent");
    let c = mem
        .comments
        .iter()
        .rfind(|c| c.kind == toknila_core::CommentKind::Instruction)
        .expect("instruction");
    assert!(
        format!("{} {:?}", c.body.gist, c.body.what).contains(URL),
        "{c:?}"
    );
}

#[test]
fn gh_missing_and_gh_not_logged_in_are_typed_blocks_with_the_fix() {
    let mut mem = github();
    answer(&mem, Err(MergeError::GhNotLoggedIn));
    accept(&mut mem);
    settle(&mut mem);
    let BlockedReason::MergeFailed { detail } = blocked_reason(&mem.ticket_by_title("A")) else {
        panic!("merge_failed");
    };
    assert!(detail.contains("gh auth login"), "{detail}");

    answer(&mem, Err(MergeError::GhMissing));
    unblock(&mut mem);
    settle(&mut mem);
    let BlockedReason::MergeFailed { detail } = blocked_reason(&mem.ticket_by_title("A")) else {
        panic!("merge_failed");
    };
    assert!(detail.contains("cli.github.com"), "{detail}");
}

#[test]
fn the_merge_settings_are_saved_and_refused_when_unusable() {
    let mut mem = Mem::demo();
    let p = projects::patch(
        &mut mem,
        "DEMO",
        projects::PatchProject {
            pr_title: Some("{title} ({key})".to_owned()),
            merge_method: Some(MergeMethod::Rebase),
            ..projects::PatchProject::default()
        },
    )
    .expect("patch");
    assert_eq!(
        (p.merge.mode, p.merge.pr_title.as_str(), p.merge.method),
        (MergeMode::Local, "{title} ({key})", MergeMethod::Rebase)
    );
    let github = projects::PatchProject {
        merge_mode: Some(MergeMode::Github),
        ..projects::PatchProject::default()
    };
    let err = projects::patch(&mut mem, "DEMO", github.clone()).expect_err("no repository");
    assert!(err.message.contains("repository"), "{}", err.message);
    mem.project.repo_path = Some("/repo".to_owned());
    let p = projects::patch(&mut mem, "DEMO", github).expect("patch");
    assert_eq!(p.merge.mode, MergeMode::Github);
    let blank = projects::PatchProject {
        pr_title: Some(" ".to_owned()),
        ..projects::PatchProject::default()
    };
    projects::patch(&mut mem, "DEMO", blank).expect_err("blank title");
}

#[test]
fn the_no_checks_timeout_defaults_to_ten_minutes_and_is_kept_within_bounds() {
    let mut mem = Mem::demo();
    assert_eq!(mem.project.merge.no_checks_timeout_secs, 600);
    let set = |secs| projects::PatchProject {
        no_checks_timeout_secs: Some(secs),
        ..projects::PatchProject::default()
    };
    let p = projects::patch(&mut mem, "DEMO", set(90)).expect("patch");
    assert_eq!(p.merge.no_checks_timeout_secs, 90);
    for bad in [0, 7 * 24 * 3600 + 1] {
        projects::patch(&mut mem, "DEMO", set(bad)).expect_err("out of bounds");
    }
    assert_eq!(mem.project.merge.no_checks_timeout_secs, 90);
}
