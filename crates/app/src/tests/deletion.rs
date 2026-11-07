//! Deleting a project: refused while an agent works; otherwise the records go and the journal
//! tells of each chain and of the project.

use toknila_core::{Ship, TicketStatus};

use super::Mem;
use crate::removal::delete_project;
use crate::{ErrorKind, UnitOfWork};

#[test]
fn deleting_a_project_hands_back_its_cards_and_ships_and_journals_each_chain() {
    let mut mem = Mem::demo();
    let a = mem.ticket_by_title("A");
    let ship = Ship {
        ticket_id: a.id,
        key: a.key.clone(),
        branch: "ship/demo/demo-1-a".to_owned(),
        path: "/home/ships/demo/demo/demo-1-a".to_owned(),
    };
    mem.save_ship(&ship).expect("ship");
    let removed = delete_project(&mut mem, "DEMO").expect("deleted");
    assert_eq!(removed.project.key.to_string(), "DEMO");
    assert_eq!(removed.chains.len(), 1);
    assert_eq!(removed.tickets.len(), 2);
    assert_eq!(removed.ships, [ship]);
    assert_eq!(mem.kinds(), ["chain.deleted", "project.deleted"]);
    assert!(mem.view.tickets.is_empty());
}

#[test]
fn deleting_is_refused_while_an_agent_of_the_project_works() {
    let mut mem = Mem::demo();
    mem.view.tickets[0].status = TicketStatus::Running;
    let e = delete_project(&mut mem, "DEMO").expect_err("refused");
    assert_eq!(e.kind, ErrorKind::Conflict);
    assert_eq!(
        e.message,
        "DEMO has 1 agent(s) working; stop them before deleting the project"
    );
    assert_eq!(mem.view.tickets.len(), 2, "nothing was deleted");
    assert!(mem.events.is_empty());
}
