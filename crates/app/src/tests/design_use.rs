//! Who uses a design, and what removing it by force does to them.

use toknila_core::design::{DesignRef, Region};
use toknila_core::{Actor, DesignId, TicketPatch};

use super::Mem;
use crate::design_use::{DesignUse, of_design, release};
use crate::pins::{NewPin, answer, ask};
use crate::{planner, tickets};

fn pin(design: DesignId, card: Option<&str>) -> NewPin {
    NewPin {
        design,
        design_name: "home.png".to_owned(),
        region: Region {
            x: 1,
            y: 2,
            w: 3,
            h: 4,
        },
        text: "Which logo?".to_owned(),
        card: card.map(str::to_owned),
    }
}

fn reference(mem: &mut Mem, title: &str, design: DesignId) {
    let card = mem.ticket_by_title(title);
    let patch = TicketPatch {
        designs: Some(vec![DesignRef {
            design,
            region: None,
        }]),
        ..TicketPatch::default()
    };
    tickets::edit(mem, &card, patch).expect("designs");
}

#[test]
fn the_users_are_the_cards_that_reference_it_and_its_open_pins() {
    let mut mem = Mem::demo();
    let (project, design) = (mem.project.id, DesignId::new());
    assert!(of_design(&mem, project, design).expect("use").is_free());
    reference(&mut mem, "A", design);
    ask(&mut mem, project, pin(design, None)).expect("open");
    let done = ask(&mut mem, project, pin(design, None)).expect("answered");
    answer(&mut mem, project, done.id, "Blue.").expect("answer");
    ask(&mut mem, project, pin(DesignId::new(), None)).expect("another design");

    let used = of_design(&mem, project, design).expect("use");
    assert_eq!(used.pins, 1, "an answered pin is not a user");
    assert_eq!(
        used.cards
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        ["DEMO-1"]
    );
    assert_eq!(
        used.sentence("home.png"),
        "home.png is used by DEMO-1 and 1 pin."
    );
}

#[test]
fn the_sentence_names_every_card_then_the_pins() {
    let used = DesignUse {
        cards: vec![
            "DEMO-12".parse().expect("key"),
            "DEMO-14".parse().expect("key"),
        ],
        pins: 2,
    };
    assert_eq!(
        used.sentence("a.png"),
        "a.png is used by DEMO-12, DEMO-14 and 2 pins."
    );
}

#[test]
fn releasing_takes_references_and_open_pins_and_tells_each_card() {
    let mut mem = Mem::demo();
    mem.actor = Actor::Human;
    let (project, design) = (mem.project.id, DesignId::new());
    reference(&mut mem, "A", design);
    let open = ask(&mut mem, project, pin(design, Some("DEMO-1"))).expect("open");
    let kept = ask(&mut mem, project, pin(design, None)).expect("kept");
    answer(&mut mem, project, kept.id, "Blue.").expect("answer");
    mem.events.clear();

    let used = release(&mut mem, project, design, "home.png").expect("release");
    assert_eq!((used.cards.len(), used.pins), (1, 1));
    assert!(mem.ticket_by_title("A").designs.is_empty());
    assert_eq!(mem.pins.iter().map(|p| p.id).collect::<Vec<_>>(), [kept.id]);
    assert!(mem.pins.iter().all(|p| p.id != open.id));
    let card = mem.ticket_by_title("A");
    let said: Vec<_> = mem
        .comments
        .iter()
        .filter(|c| c.ticket_id == card.id && c.author == Actor::System)
        .collect();
    assert_eq!(said.len(), 1);
    assert_eq!(
        said[0].body.gist,
        "Design home.png was removed from the library"
    );
    assert!(mem.kinds().contains(&"design_pin.removed".to_owned()));
    assert!(of_design(&mem, project, design).expect("use").is_free());
}

#[test]
fn the_answer_of_a_pin_without_a_card_reaches_the_plan() {
    let mut mem = Mem::demo();
    let (project, design) = (mem.project.id, DesignId::new());
    let p = ask(&mut mem, project, pin(design, None)).expect("ask");
    answer(&mut mem, project, p.id, "The blue one.").expect("answer");
    let plan = planner::plan(&mem, project, toknila_core::Timestamp(0)).expect("plan");
    assert_eq!(plan.pins.len(), 1);
    assert_eq!(plan.pins[0].answer.as_deref(), Some("The blue one."));
    assert!(
        plan.pins[0]
            .plan_line()
            .contains("answered: \"The blue one.\"")
    );
}
