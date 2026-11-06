use super::*;

#[test]
fn done_behind_a_ticket_that_is_not_done_is_out_of_order_through_releases_too() {
    let toml = r#"
[chain]
name = "c"

[[ticket]]
id = "A"

[[ticket]]
id = "B"
done = true
after = ["A"]

[[ticket]]
id = "C"
done = true
after = ["B"]

[[ticket]]
id = "D"
done = true

[[release]]
id = "R1"
after = ["A"]

[[ticket]]
id = "E"
done = true
after_release = ["R1"]
"#;
    let plan = parse(toml).expect("valid");
    // A is open; B and C wait for it (C through B); D is independent; E waits for A via R1.
    assert_eq!(
        plan.out_of_order(),
        [vec![], vec![0], vec![0], vec![], vec![0]]
    );
}
