//! P1-01: wildcard cards through the real `tok` binary and daemon. The simulated agent of a
//! running ticket finds work nobody planned; the card waits for the owner, does not count in
//! progress, and once approved starts when its dependencies allow. A card that would go before
//! work already running cannot be approved and is rejected instead. The `--json` output is
//! pinned by snapshots in `tests/snapshots/`.

mod common;

use common::{Tok, snapshot};

const CHAIN: &str = r#"
[chain]
name = "demo"
goal = "Sign in with a token"

[[ticket]]
id = "LOGIN"
title = "Login"
profile = "sim"

[[ticket]]
id = "PROFILE"
title = "Profile page"
after = ["LOGIN"]
profile = "sim"
"#;

#[test]
fn wildcard_cards_wait_for_the_owner() {
    let dir = tempfile::tempdir().expect("tempdir");
    let tok = Tok::in_dir(dir.path());
    let file = dir.path().join("demo.toml");
    std::fs::write(&file, CHAIN).expect("write chain");
    tok.run(&["project", "new", "DEMO", "--name", "Demo"]);
    tok.run(&[
        "chain",
        "import",
        file.to_str().expect("utf-8 path"),
        "--project",
        "DEMO",
    ]);
    assert_eq!(tok.statuses(), ["DEMO-1 running", "DEMO-2 backlog"]);

    // 1. The agent of the running ticket finds unplanned work: a card, not a silent change.
    let card = tok.run(&[
        "sim",
        "DEMO-1",
        "propose",
        "Token refresh endpoint",
        "--prompt",
        "Login needs a token refresh endpoint first",
        "--accept",
        "an expired token is refreshed",
        "--after",
        "DEMO-1",
        "--before",
        "DEMO-2",
    ]);
    let t = &card["ticket"];
    assert_eq!(
        (&t["key"], &t["status"], &t["wildcard"]["from"]),
        (&"DEMO-3".into(), &"proposed".into(), &"DEMO-1".into())
    );
    assert_eq!(t["wildcard"]["proposed_by"], "agent");
    snapshot("wildcard_proposed", card);

    // 2. It waits on a person, changes nothing else and does not count in progress.
    let human = tok.run(&["ls", "--needs-human"]);
    let items = human.as_array().expect("a list");
    assert_eq!(items.len(), 1, "{human}");
    assert_eq!(items[0]["ticket"]["key"], "DEMO-3");
    assert_eq!(items[0]["ticket"]["status_reason"]["status"], "proposed");
    snapshot("wildcard_needs_human", human);
    let s = &tok.chain()["summary"];
    assert_eq!(
        (&s["total"], &s["done"], &s["status"]),
        (&2.into(), &0.into(), &"needs_human".into())
    );
    assert_eq!(
        tok.statuses(),
        ["DEMO-1 running", "DEMO-2 backlog", "DEMO-3 proposed"]
    );

    // 3. The owner approves: its edges take effect and the graph decides when it starts.
    let approved = tok.run(&["approve", "DEMO-3"]);
    assert_eq!(
        approved["ticket"]["status"], "backlog",
        "it waits for DEMO-1"
    );
    assert_eq!(approved["ticket"]["wildcard"]["approved"], true);
    snapshot("wildcard_approved", approved);
    assert_eq!(tok.chain()["summary"]["total"], 3);
    tok.run(&["sim", "DEMO-1", "finish", "login works"]);
    tok.run(&["accept", "DEMO-1"]);
    assert_eq!(
        tok.statuses(),
        ["DEMO-1 merged", "DEMO-2 backlog", "DEMO-3 running"],
        "the card starts; DEMO-2 now waits for it"
    );
    tok.run(&["sim", "DEMO-3", "finish", "refresh works"]);
    tok.run(&["accept", "DEMO-3"]);
    assert_eq!(tok.status("DEMO-2"), "running");

    // 4. A card that would go before running work cannot be approved; the owner rejects it.
    let late = tok.run(&[
        "propose",
        "demo",
        "Rename the page",
        "--before",
        "DEMO-2",
        "--from",
        "DEMO-2",
    ]);
    assert_eq!(late["ticket"]["key"], "DEMO-4");
    assert_eq!(late["ticket"]["wildcard"]["proposed_by"], "human");
    let msg = tok.fails(&["approve", "DEMO-4"]);
    assert!(
        msg.contains(
            "DEMO-2 is already running; a card can only go before work that has not started"
        ),
        "{msg}"
    );
    assert_eq!(
        tok.status("DEMO-4"),
        "proposed",
        "a refused approval changes nothing"
    );
    let rejected = tok.run(&["reject", "DEMO-4", "out of scope"]);
    assert_eq!(rejected["ticket"]["status"], "cancelled");
    assert_eq!(rejected["ticket"]["status_reason"]["text"], "out of scope");
    snapshot("wildcard_rejected", rejected);
    let s = &tok.chain()["summary"];
    assert_eq!(
        (&s["total"], &s["done"], &s["status"]),
        (&3.into(), &2.into(), &"running".into()),
        "a rejected card never joined the plan"
    );
    assert_eq!(tok.run(&["ls", "--needs-human"]), serde_json::json!([]));
}
