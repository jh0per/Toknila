use super::*;

fn pin(number: u32, text: &str, answer: Option<&str>) -> DesignPin {
    DesignPin {
        id: PinId::new(),
        project: ProjectId::new(),
        design: DesignId::new(),
        design_name: "checkout.png".to_owned(),
        number,
        region: Region {
            x: 12,
            y: 30,
            w: 40,
            h: 20,
        },
        text: text.to_owned(),
        card: Some("DEMO-1".parse().expect("key")),
        answer: answer.map(str::to_owned),
        created_at: Timestamp(1),
        answered_at: answer.map(|_| Timestamp(2)),
    }
}

#[test]
fn text_is_trimmed_and_bounded() {
    assert_eq!(clean_text("  Why?  ", "question").as_deref(), Ok("Why?"));
    assert_eq!(
        clean_text("   ", "answer"),
        Err("The answer is empty.".to_owned())
    );
    assert!(clean_text(&"x".repeat(MAX_TEXT), "answer").is_ok());
    assert!(clean_text(&"x".repeat(MAX_TEXT + 1), "answer").is_err());
}

#[test]
fn open_pin_has_no_answer() {
    assert!(pin(1, "Tax?", None).is_open());
    assert!(!pin(1, "Tax?", Some("Yes")).is_open());
}

#[test]
fn section_is_appended_with_answered_pins_only() {
    let a = pin(1, "Show tax?", Some("Yes, separately."));
    let open = pin(2, "Logo fixed?", None);
    let out = with_section("Build the page.", &[&a, &open]);
    assert_eq!(
        out,
        "Build the page.\n\n## From the design\n- checkout.png #1 (x 12%, y 30%, 40% wide, 20% tall): Show tax? - Yes, separately."
    );
}

#[test]
fn section_is_replaced_in_place_and_the_rest_kept() {
    let old = "Intro.\n\n## From the design\n- old entry\n\n## Notes\nKeep me.";
    let a = pin(1, "Show tax?", Some("No."));
    let out = with_section(old, &[&a]);
    assert_eq!(
        out,
        "Intro.\n\n## From the design\n- checkout.png #1 (x 12%, y 30%, 40% wide, 20% tall): Show tax? - No.\n\n## Notes\nKeep me."
    );
    // The same call again changes nothing.
    assert_eq!(with_section(&out, &[&a]), out);
}

#[test]
fn no_answered_pin_removes_the_section() {
    let old = "Intro.\n\n## From the design\n- old entry";
    assert_eq!(with_section(old, &[]), "Intro.");
}

#[test]
fn answer_with_newlines_stays_one_entry() {
    let a = pin(1, "Tax?", Some("Yes\n## Evil\nno"));
    let out = with_section("P", &[&a]);
    assert_eq!(out.lines().filter(|l| l.starts_with("## ")).count(), 1);
    assert_eq!(out.lines().count(), 4);
}

#[test]
fn plan_line_says_open_or_the_answer() {
    assert_eq!(
        pin(2, "Logo?", None).plan_line(),
        "#2 checkout.png (x 12%, y 30%, 40% wide, 20% tall): \"Logo?\" - open"
    );
    assert!(
        pin(2, "Logo?", Some("Fixed"))
            .plan_line()
            .ends_with("answered: \"Fixed\"")
    );
}
