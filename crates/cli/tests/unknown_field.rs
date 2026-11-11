//! The real daemon behind the real binary refuses a body with an unknown field: 400, naming the
//! field and listing the accepted ones, and the project is left as it was.

mod common;

use common::Tok;

#[test]
fn a_misspelt_field_is_a_400_that_names_it_and_changes_nothing() {
    let dir = tempfile::tempdir().expect("tempdir");
    let tok = Tok::in_dir(dir.path());
    tok.run(&["project", "new", "DEMO"]);
    let info = tok.info();

    let sent = ureq::request("PATCH", &format!("{}/v1/projects/DEMO", info.base_url()))
        .set("Authorization", &format!("Bearer {}", info.token))
        .send_json(serde_json::json!({ "review_modee": "agent" }));
    let Err(ureq::Error::Status(400, reply)) = sent else {
        panic!("expected a 400, got {sent:?}");
    };
    let body: serde_json::Value = reply.into_json().expect("json");
    let text = body["error"].as_str().expect("message");
    assert!(text.contains("`review_modee`"), "{body}");
    assert!(text.contains("`review_mode`"), "{body}");

    let shown = tok.run(&["project", "show", "DEMO"]);
    assert_ne!(shown["default_review_mode"], "agent", "{shown}");
}
