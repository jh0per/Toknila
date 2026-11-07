//! Moving a card rewires dependencies; the rest of the order stays as it was.

use super::*;

/// Applies a plan the way the application does and returns the cards in dependency order.
fn moved(v: &mut ChainView, card: NodeRef, anchor: NodeRef, place: Place) -> Vec<String> {
    let plan = v.plan_move(tid(card), tid(anchor), place).expect("plan");
    v.edges.retain(|e| !plan.remove.contains(e));
    for e in plan.add {
        v.add_edge(e).expect("valid edge");
    }
    v.topo_order().iter().map(|n| v.label(*n)).collect()
}

fn line(b: &mut Builder, n: usize) -> Vec<NodeRef> {
    let cards: Vec<NodeRef> = (0..n).map(|_| b.ticket()).collect();
    for w in cards.windows(2) {
        b.edge(w[0], w[1]);
    }
    cards
}

#[test]
fn a_card_moves_after_a_later_one_in_a_line() {
    let mut b = Builder::new(false);
    let c = line(&mut b, 4);
    let order = moved(&mut b.view, c[0], c[2], Place::After);
    assert_eq!(order, ["DEMO-2", "DEMO-3", "DEMO-1", "DEMO-4"]);
}

#[test]
fn a_card_moves_before_an_earlier_one_in_a_line() {
    let mut b = Builder::new(false);
    let c = line(&mut b, 4);
    let order = moved(&mut b.view, c[3], c[1], Place::Before);
    assert_eq!(order, ["DEMO-1", "DEMO-4", "DEMO-2", "DEMO-3"]);
}

#[test]
fn a_card_moves_to_the_front_and_the_back() {
    let mut b = Builder::new(false);
    let c = line(&mut b, 3);
    assert_eq!(
        moved(&mut b.view, c[2], c[0], Place::Before),
        ["DEMO-3", "DEMO-1", "DEMO-2"]
    );
    assert_eq!(
        moved(&mut b.view, c[2], c[1], Place::After),
        ["DEMO-1", "DEMO-2", "DEMO-3"]
    );
}

#[test]
fn moving_a_card_out_of_a_fork_keeps_its_neighbours_joined() {
    let mut b = Builder::new(false);
    let (a, x, y, z) = (b.ticket(), b.ticket(), b.ticket(), b.ticket());
    b.edge(a, x);
    b.edge(x, y);
    b.edge(a, z);
    b.edge(y, z);
    let order = moved(&mut b.view, x, z, Place::After);
    assert_eq!(order, ["DEMO-1", "DEMO-3", "DEMO-4", "DEMO-2"]);
    assert!(b.view.validate().is_ok());
}

#[test]
fn a_card_that_has_started_cannot_move_and_cannot_be_put_before_started_work() {
    let mut b = Builder::new(false);
    let c = line(&mut b, 3);
    b.view.tickets[0].status = TicketStatus::Running;
    assert!(!b.view.can_move(tid(c[0])));
    assert!(matches!(
        b.view.plan_move(tid(c[0]), tid(c[2]), Place::After),
        Err(GraphError::BeforeStarted { .. })
    ));
    b.view.tickets[1].status = TicketStatus::Merged;
    assert!(matches!(
        b.view.plan_move(tid(c[2]), tid(c[1]), Place::Before),
        Err(GraphError::BeforeStarted { .. })
    ));
}

#[test]
fn a_card_cannot_move_next_to_itself() {
    let mut b = Builder::new(false);
    let c = line(&mut b, 2);
    assert!(matches!(
        b.view.plan_move(tid(c[0]), tid(c[0]), Place::After),
        Err(GraphError::SelfLoop(_))
    ));
}

#[test]
fn the_tail_is_the_cards_nothing_waits_for() {
    let mut b = Builder::new(false);
    let (a, x, y) = (b.ticket(), b.ticket(), b.ticket());
    b.edge(a, x);
    b.edge(a, y);
    let mut tail = b.view.tail_tickets();
    tail.sort_by_key(|t| b.view.ticket(*t).map(|t| t.key.number()));
    assert_eq!(tail, [tid(x), tid(y)]);
    b.view.tickets[2].status = TicketStatus::Cancelled;
    assert_eq!(b.view.tail_tickets(), [tid(x)]);
}
