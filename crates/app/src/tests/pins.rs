//! The assistant's pins through the use cases: asking, answering, and the card's prompt.

use toknila_core::design::Region;
use toknila_core::{Actor, DesignId, TicketPatch};

use super::Mem;
use crate::pins::{NewPin, answer, ask};
use crate::{ErrorKind, tickets};

fn new_pin(design: DesignId, text: &str, card: Option<&str>) -> NewPin {
    NewPin {
        design,
        design_name: "home.png".to_owned(),
        region: Region {
            x: 10,
            y: 20,
            w: 30,
            h: 40,
        },
        text: text.to_owned(),
        card: card.map(str::to_owned),
    }
}

#[test]
fn asking_pins_an_open_numbered_question_and_says_so_in_the_journal() {
    let mut mem = Mem::demo();
    mem.actor = Actor::Agent;
    let (project, design) = (mem.project.id, DesignId::new());
    let a = ask(
        &mut mem,
        project,
        new_pin(design, "  Is the logo fixed? ", Some("DEMO-1")),
    )
    .expect("ask");
    let b = ask(&mut mem, project, new_pin(design, "Tax?", None)).expect("ask");
    assert_eq!((a.number, b.number), (1, 2));
    assert_eq!(a.text, "Is the logo fixed?");
    assert!(a.is_open());
    assert_eq!(
        a.card.as_ref().map(ToString::to_string).as_deref(),
        Some("DEMO-1")
    );
    assert_eq!(b.card, None);
    assert_eq!(mem.kinds(), ["design_pin.asked", "design_pin.asked"]);
}

#[test]
fn a_bad_region_text_or_card_is_refused() {
    let mut mem = Mem::demo();
    let (project, design) = (mem.project.id, DesignId::new());
    let mut bad = new_pin(design, "Q?", None);
    bad.region.w = 0;
    assert_eq!(
        ask(&mut mem, project, bad).expect_err("bad region").kind,
        ErrorKind::BadRequest
    );
    let empty = new_pin(design, "   ", None);
    assert_eq!(
        ask(&mut mem, project, empty).expect_err("empty text").kind,
        ErrorKind::BadRequest
    );
    let ghost = new_pin(design, "Q?", Some("DEMO-99"));
    assert_eq!(
        ask(&mut mem, project, ghost)
            .expect_err("no such card")
            .kind,
        ErrorKind::NotFound
    );
    assert!(mem.pins.is_empty());
}

#[test]
fn answering_puts_question_and_answer_in_the_cards_prompt() {
    let mut mem = Mem::demo();
    let (project, design) = (mem.project.id, DesignId::new());
    let first = mem.ticket_by_title("A");
    tickets::edit(
        &mut mem,
        &first,
        TicketPatch {
            prompt: Some("Build the page.".to_owned()),
            ..TicketPatch::default()
        },
    )
    .expect("prompt");
    let pin = ask(
        &mut mem,
        project,
        new_pin(design, "Show tax?", Some("DEMO-1")),
    )
    .expect("ask");

    let done = answer(&mut mem, project, pin.id, "Yes, separately.").expect("answer");
    assert_eq!(done.answer.as_deref(), Some("Yes, separately."));
    let prompt = mem.ticket_by_title("A").prompt;
    assert_eq!(
        prompt,
        "Build the page.\n\n## From the design\n- home.png #1 (x 10%, y 20%, 30% wide, 40% tall): Show tax? - Yes, separately."
    );

    // A changed answer replaces the entry instead of adding one.
    answer(&mut mem, project, pin.id, "No.").expect("answer again");
    let prompt = mem.ticket_by_title("A").prompt;
    assert_eq!(prompt.matches("## From the design").count(), 1);
    assert!(prompt.ends_with("Show tax? - No."), "{prompt}");
    assert!(mem.kinds().contains(&"design_pin.answered".to_owned()));
}

#[test]
fn an_answer_to_a_pin_without_a_card_changes_no_prompt() {
    let mut mem = Mem::demo();
    let (project, design) = (mem.project.id, DesignId::new());
    let before = mem.ticket_by_title("A").prompt;
    let pin = ask(&mut mem, project, new_pin(design, "Q?", None)).expect("ask");
    answer(&mut mem, project, pin.id, "A.").expect("answer");
    assert_eq!(mem.ticket_by_title("A").prompt, before);
}

#[test]
fn a_pin_of_another_project_cannot_be_answered() {
    let mut mem = Mem::demo();
    let design = DesignId::new();
    let project = mem.project.id;
    let pin = ask(&mut mem, project, new_pin(design, "Q?", None)).expect("ask");
    let other = toknila_core::ProjectId::new();
    let err = answer(&mut mem, other, pin.id, "A.").expect_err("another project");
    assert_eq!(err.kind, ErrorKind::NotFound);
    assert!(mem.pins[0].is_open());
}
