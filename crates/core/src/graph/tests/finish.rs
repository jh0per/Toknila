//! The Finish node: the chain's version, its end release and where the release stands.

use super::*;

fn merged(b: &mut Builder, n: usize) {
    b.view.tickets[n].status = TicketStatus::Merged;
}

#[test]
fn a_chain_without_a_version_or_an_end_release_has_no_finish() {
    let mut b = Builder::new(false);
    b.ticket();
    let s = b.view.summary(T);
    assert_eq!((s.version, s.finish), (None, None));
}

#[test]
fn the_chains_own_version_is_planned_until_every_card_is_merged_then_ready() {
    let mut b = Builder::new(false);
    b.view.chain.version = Some("0.3.0".to_owned());
    b.ticket();
    b.ticket();
    let finish = |b: &Builder| b.view.summary(T).finish.expect("a finish");
    assert_eq!(
        finish(&b),
        Finish {
            version: "0.3.0".to_owned(),
            state: FinishState::Planned,
            tag: None,
            released_at: None,
        }
    );
    merged(&mut b, 0);
    assert_eq!(finish(&b).state, FinishState::Planned, "one card is open");
    merged(&mut b, 1);
    assert_eq!(finish(&b).state, FinishState::Ready);
    assert_eq!(b.view.summary(T).version.as_deref(), Some("0.3.0"));
}

#[test]
fn an_end_release_is_the_finish_and_gives_the_chain_its_version() {
    let mut b = Builder::new(false);
    let a = b.ticket();
    let r = b.release("1.4.0", StartMode::Manual);
    b.edge(a, r);
    // The chain's own version is older than its end release: the end release wins.
    b.view.chain.version = Some("0.1.0".to_owned());
    assert_eq!(b.view.version().as_deref(), Some("1.4.0"));
    assert_eq!(
        b.view.finish().map(|f| (f.version, f.state)),
        Some(("1.4.0".to_owned(), FinishState::Planned))
    );
    merged(&mut b, 0);
    assert_eq!(b.view.finish().map(|f| f.state), Some(FinishState::Ready));
}

#[test]
fn a_released_end_release_reads_released_with_its_tag_and_date() {
    let mut b = Builder::new(false);
    let a = b.ticket();
    let r = b.release("1.4.0", StartMode::Manual);
    b.edge(a, r);
    merged(&mut b, 0);
    let mut v = b.view;
    v.cut_release(rid(r), T).expect("cut");
    v.tag_release(rid(r), "v1.4.0".to_owned()).expect("tag");
    v.complete_release(rid(r), Timestamp(2_000))
        .expect("release");
    assert_eq!(v.end_release(), None, "nothing is left to release");
    assert_eq!(
        v.finish(),
        Some(Finish {
            version: "1.4.0".to_owned(),
            state: FinishState::Released,
            tag: Some("v1.4.0".to_owned()),
            released_at: Some(Timestamp(2_000)),
        })
    );
}

#[test]
fn a_planned_end_release_waits_for_the_cards_nothing_else_waits_for() {
    let mut b = Builder::new(false);
    let (a, c) = (b.ticket(), b.ticket());
    b.edge(a, c);
    let r = b.release("0.3.0", StartMode::Manual);
    let edges = b.view.end_release_edges(rid(r));
    assert_eq!(edges.len(), 1, "only the last card: {edges:?}");
    assert_eq!((edges[0].from, edges[0].to), (c, r));
    for e in edges {
        b.view.add_edge(e).expect("edge");
    }
    assert_eq!(b.view.end_release_edges(rid(r)), [], "nothing is left over");
    // A release that was cut is not wired again.
    b.view.releases[0].state = ReleaseState::Cut;
    b.ticket();
    assert_eq!(b.view.end_release_edges(rid(r)), []);
}
