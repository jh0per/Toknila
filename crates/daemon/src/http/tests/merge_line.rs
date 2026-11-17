//! The merge line through the router: a release run's turn kept in the journal holds back the
//! merge of a card accepted meanwhile, the card says what it waits for, and the start of a daemon
//! (recovery) frees the turn of a run that was cut off, so the card merges.

use std::path::PathBuf;

use serde_json::json;

use super::releases::{Env, env, merge_card};
use crate::git::testing::commit_file;
use crate::store::release_runs::Entry;
use toknila_core::release_run::ReleaseStep::Tag;

/// Journals what a daemon that died while its release run held the merge line left.
async fn run_cut_off_holding_the_line(e: &Env, release: &str) {
    let id: toknila_core::ReleaseId = release.parse().expect("release id");
    let entries = [
        Entry::RunStarted {
            version: "1.0.0".to_owned(),
            tag: "v1.0.0".to_owned(),
            steps: vec![Tag],
        },
        Entry::TurnTaken,
        Entry::StepStarted { step: Tag },
    ];
    e.api
        .store
        .write(
            toknila_core::Actor::System,
            crate::server::now(),
            move |tx| -> Result<(), crate::store::StoreError> {
                for entry in &entries {
                    tx.journal_release_run(id, entry)?;
                }
                Ok(())
            },
        )
        .await
        .expect("journal");
}

/// A card of epic `web` with work on its ship, handed in and accepted.
async fn accepted_card(e: &Env) -> String {
    e.api
        .post("/v1/projects/DEMO/chains", json!({ "name": "web" }))
        .await;
    let t = e
        .api
        .post("/v1/chains/web/tickets", json!({ "title": "Later" }))
        .await;
    let key = t["ticket"]["key"].as_str().expect("key").to_owned();
    let d = e.api.get(&format!("/v1/tickets/{key}")).await;
    let ship = PathBuf::from(d["ship"]["path"].as_str().expect("ship"));
    commit_file(&ship, "later.txt", "x\n", "later");
    e.api
        .post(
            &format!("/v1/tickets/{key}/events"),
            json!({ "event": "finished", "summary": toknila_core::handin::from_plain("done") }),
        )
        .await;
    e.api
        .post(&format!("/v1/tickets/{key}/actions/accept"), json!({}))
        .await;
    key
}

#[tokio::test]
async fn a_restart_frees_the_turn_of_a_release_cut_off_and_the_card_accepted_meanwhile_merges() {
    let e = env().await;
    merge_card(&e, "First", "a.txt").await;
    let chain = e
        .api
        .post("/v1/chains/auth/releases", json!({ "version": "1.0.0" }))
        .await;
    let release = chain["releases"][0]["id"].as_str().expect("id").to_owned();
    run_cut_off_holding_the_line(&e, &release).await;

    let key = accepted_card(&e).await;
    let card = e.api.get(&format!("/v1/tickets/{key}")).await;
    assert_eq!(card["ticket"]["status"], "review", "{card}");
    assert_eq!(card["ticket"]["review"]["turn"]["by"], "merging");
    assert_eq!(
        card["review"]["merge_wait"],
        json!({ "for": "release", "version": "1.0.0" }),
        "the card waits for the release that holds the line: {card}"
    );

    // The daemon starts again: the run is interrupted, nobody holds its turn, the card merges.
    crate::recovery::recover(&e.api.store)
        .await
        .expect("recover");
    e.api.store.settled().await;
    let run = e.api.get("/v1/releases/1.0.0/run").await;
    assert_eq!(run["state"], "interrupted", "{run}");
    let card = e.api.get(&format!("/v1/tickets/{key}")).await;
    assert_eq!(card["ticket"]["status"], "merged", "{card}");
    assert_eq!(card["review"]["merge_wait"], serde_json::Value::Null);
}
