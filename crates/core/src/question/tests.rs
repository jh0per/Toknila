use super::*;

#[test]
fn what_each_kind_offers() {
    assert!(QuestionKind::Question.answers().is_empty());
    assert_eq!(
        QuestionKind::Permission.answers(),
        [AnswerKind::Allow, AnswerKind::Deny]
    );
    assert_eq!(
        QuestionKind::Screen.answers(),
        [
            AnswerKind::Enter,
            AnswerKind::One,
            AnswerKind::Two,
            AnswerKind::Escape,
            AnswerKind::Text
        ]
    );
    assert!(QuestionKind::Permission.accepts(AnswerKind::Deny));
    assert!(!QuestionKind::Permission.accepts(AnswerKind::Text));
    assert!(!QuestionKind::Question.accepts(AnswerKind::Allow));
}

#[test]
fn a_waiting_hook_gets_a_decision_and_a_denial_carries_the_comment() {
    assert_eq!(
        deliver(AnswerKind::Allow, None, true),
        Ok(Delivery::Hook(Decision::Allow))
    );
    assert_eq!(
        deliver(AnswerKind::Deny, Some(" not now "), true),
        Ok(Delivery::Hook(Decision::Deny {
            message: "not now".into()
        }))
    );
    let Ok(Delivery::Hook(Decision::Deny { message })) =
        deliver(AnswerKind::Deny, Some("  "), true)
    else {
        panic!("expected a denial");
    };
    assert!(!message.is_empty());
}

#[test]
fn without_a_waiting_hook_an_answer_is_keys() {
    let keys = |a, t| deliver(a, t, false);
    assert_eq!(
        keys(AnswerKind::Allow, None),
        Ok(Delivery::Keys(b"1".to_vec()))
    );
    assert_eq!(keys(AnswerKind::Deny, None), Ok(Delivery::Keys(vec![0x1b])));
    assert_eq!(
        keys(AnswerKind::Enter, None),
        Ok(Delivery::Keys(b"\r".to_vec()))
    );
    assert_eq!(
        keys(AnswerKind::One, None),
        Ok(Delivery::Keys(b"1".to_vec()))
    );
    assert_eq!(
        keys(AnswerKind::Two, None),
        Ok(Delivery::Keys(b"2".to_vec()))
    );
    assert_eq!(
        keys(AnswerKind::Escape, None),
        Ok(Delivery::Keys(vec![0x1b]))
    );
    assert_eq!(
        keys(AnswerKind::Text, Some(" yes please ")),
        Ok(Delivery::Keys(b"yes please\r".to_vec()))
    );
    assert_eq!(
        keys(AnswerKind::Text, Some(" ")),
        Err(AnswerError::EmptyText)
    );
}

#[test]
fn the_startup_rule_is_a_function_of_spawn_activity_and_now() {
    let spawned = Timestamp(1_000);
    let at = |ms: i64| Timestamp(1_000 + ms);
    assert!(!waits_on_screen(spawned, None, at(19_999), 20), "not yet");
    assert!(
        waits_on_screen(spawned, None, at(20_000), 20),
        "at the limit"
    );
    assert!(
        !waits_on_screen(spawned, Some(at(5_000)), at(60_000), 20),
        "the first sign of life ends it"
    );
    assert!(
        !waits_on_screen(spawned, None, at(600_000), 0),
        "0 turns it off"
    );
    assert!(
        !waits_on_screen(spawned, None, Timestamp(0), 20),
        "a clock behind the spawn"
    );
}

#[test]
fn run_facts_apply_the_rule() {
    let facts = RunFacts {
        ticket: TicketId::new(),
        role: AgentRole::Reviewer,
        spawned: Timestamp(0),
        first_activity: None,
        startup_secs: 20,
        tail: Vec::new(),
    };
    assert!(facts.waits_on_screen(Timestamp(20_000)));
    assert!(!facts.waits_on_screen(Timestamp(1_000)));
}

#[test]
fn ansi_is_stripped_and_the_last_lines_are_kept() {
    let out = b"\x1b[2J\x1b[1;1H\x1b[1mDo you trust this folder?\x1b[0m\r\n\r\n\x1b[32m> 1. Yes\x1b[0m\r\n  2. No\r\n";
    assert_eq!(
        last_lines(out, 15),
        ["Do you trust this folder?", "> 1. Yes", "  2. No"]
    );
    assert_eq!(last_lines(out, 2), ["> 1. Yes", "  2. No"]);
    assert_eq!(last_lines(b"", 15), Vec::<String>::new());
}

#[test]
fn a_screen_drawn_with_cursor_moves_reads_row_by_row_and_osc_is_dropped() {
    let out = b"\x1b]0;title\x07\x1b[1;1Hfirst row\x1b[2;1Hsecond\x1b[3Cgap\x1b[3;1Hthird";
    assert_eq!(last_lines(out, 15), ["first row", "second gap", "third"]);
}

#[test]
fn at_most_the_screen_limit_of_lines_come_back() {
    let out: String = (0..40).map(|i| format!("line {i}\n")).collect();
    let lines = last_lines(out.as_bytes(), SCREEN_LINES);
    assert_eq!(lines.len(), SCREEN_LINES);
    assert_eq!(lines.last().map(String::as_str), Some("line 39"));
}
