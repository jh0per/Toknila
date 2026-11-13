//! The end release waits for every story of its epic: the leaves, however a story got there.

use super::*;

/// Applies the rule the way the application does after a change, and returns the labels of what
/// the release waits for, sorted.
fn rewired(v: &mut ChainView, release: NodeRef) -> Vec<String> {
    let plan = v.end_release_wiring(rid(release));
    v.edges.retain(|e| !plan.remove.contains(e));
    for e in plan.add {
        v.add_edge(e).expect("valid edge");
    }
    let mut after: Vec<String> = v
        .edges
        .iter()
        .filter(|e| e.to == release)
        .map(|e| v.label(e.from))
        .collect();
    after.sort();
    after
}

/// An epic as the import makes it: DEMO-1 -> DEMO-2, and the release after DEMO-2.
fn imported() -> (Builder, Vec<NodeRef>, NodeRef) {
    let mut b = Builder::new(false);
    let (a, c) = (b.ticket(), b.ticket());
    b.edge(a, c);
    let r = b.release("0.3.3", StartMode::Manual);
    rewired(&mut b.view, r);
    (b, vec![a, c], r)
}

#[test]
fn a_story_added_after_the_import_goes_before_the_release() {
    let (mut b, _, r) = imported();
    b.ticket(); // a loose story, as the window's New story or the assistant adds it
    assert_eq!(rewired(&mut b.view, r), ["DEMO-2", "DEMO-3"]);
}

#[test]
fn a_story_the_cli_appends_after_the_tail_replaces_the_tail_in_the_releases_list() {
    let (mut b, cards, r) = imported();
    let new = b.ticket();
    b.edge(cards[1], new); // `tok ticket new ... --after DEMO-2`
    assert_eq!(rewired(&mut b.view, r), ["DEMO-3"]);
}

#[test]
fn a_story_that_another_story_depends_on_leaves_the_releases_list() {
    let (mut b, cards, r) = imported();
    let loose = b.ticket();
    assert_eq!(rewired(&mut b.view, r), ["DEMO-2", "DEMO-3"]);
    let after = b.ticket();
    b.edge(loose, after);
    assert_eq!(rewired(&mut b.view, r), ["DEMO-2", "DEMO-4"]);
    let _ = cards;
}

#[test]
fn a_story_moved_in_from_another_epic_goes_before_the_release() {
    let (mut b, cards, r) = imported();
    let moved_in = b.ticket();
    b.edge(cards[1], moved_in); // joins at the end, after the tail
    assert_eq!(rewired(&mut b.view, r), ["DEMO-3"]);
}

#[test]
fn a_story_moved_out_leaves_the_release_and_its_predecessor_takes_its_place() {
    let (mut b, cards, r) = imported();
    let (gone, first) = (tid(cards[1]), cards[0]);
    b.view
        .edges
        .retain(|e| e.from != cards[1] && e.to != cards[1]);
    b.view.tickets.retain(|t| t.id != gone);
    assert_eq!(rewired(&mut b.view, r), [b.view.label(first)]);
}

#[test]
fn a_cut_release_is_left_alone() {
    let (mut b, _, r) = imported();
    b.view.releases[0].state = ReleaseState::Cut;
    b.ticket();
    assert_eq!(rewired(&mut b.view, r), ["DEMO-2"]);
}
