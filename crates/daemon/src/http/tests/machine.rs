//! The machine settings route: `max_checks` defaults to 1, is set through `PUT`, reaches the
//! line of checks at once and survives a restart; the three sound switches are off by default,
//! survive a restart, and a body without them (an older client) leaves them alone.

use axum::http::{Method, StatusCode};
use serde_json::{Value, json};

use super::Api;

const OFF: fn() -> Value = || json!({ "story_done": false, "epic_done": false, "problem": false });

#[tokio::test]
async fn max_checks_is_one_until_a_person_raises_it_and_the_line_follows() {
    let api = Api::new();
    assert_eq!(
        api.get("/v1/machine/settings").await,
        json!({ "max_checks": 1, "sounds": OFF() }),
        "nothing changes until a person raises it; every sound is off"
    );
    assert_eq!(api.store.check_line().max_checks(), 1);

    let (s, body) = api
        .call(
            Method::PUT,
            "/v1/machine/settings",
            Some(json!({ "max_checks": 3 })),
        )
        .await;
    assert_eq!(
        (s, body),
        (StatusCode::OK, json!({ "max_checks": 3, "sounds": OFF() }))
    );
    assert_eq!(api.get("/v1/machine/settings").await["max_checks"], 3);
    assert_eq!(
        api.store.check_line().max_checks(),
        3,
        "the line admits three at once from now on"
    );
}

#[tokio::test]
async fn max_checks_never_goes_below_one_and_unknown_fields_are_refused() {
    let api = Api::new();
    let (s, body) = api
        .call(
            Method::PUT,
            "/v1/machine/settings",
            Some(json!({ "max_checks": 0 })),
        )
        .await;
    assert_eq!(
        (s, body),
        (StatusCode::OK, json!({ "max_checks": 1, "sounds": OFF() }))
    );
    let (s, _) = api
        .call(
            Method::PUT,
            "/v1/machine/settings",
            Some(json!({ "max_checks": 2, "max_check": 5 })),
        )
        .await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "a misspelt field");
    assert_eq!(api.get("/v1/machine/settings").await["max_checks"], 1);
}

#[tokio::test]
async fn max_checks_survives_a_daemon_restart() {
    let api = Api::new();
    let (s, _) = api
        .call(
            Method::PUT,
            "/v1/machine/settings",
            Some(json!({ "max_checks": 4 })),
        )
        .await;
    assert_eq!(s, StatusCode::OK);
    let api = api.restart();
    assert_eq!(api.get("/v1/machine/settings").await["max_checks"], 4);
    assert_eq!(api.store.check_line().max_checks(), 4);
}

#[tokio::test]
async fn the_sound_switches_are_saved_kept_across_a_restart_and_an_old_body_leaves_them() {
    let api = Api::new();
    let on = json!({ "story_done": true, "epic_done": false, "problem": true });
    let (s, body) = api
        .call(
            Method::PUT,
            "/v1/machine/settings",
            Some(json!({ "max_checks": 2, "sounds": on })),
        )
        .await;
    assert_eq!(
        (s, body),
        (StatusCode::OK, json!({ "max_checks": 2, "sounds": on }))
    );

    // What an older `tok` or page sends: no `sounds`. It neither fails nor turns them off.
    let (s, body) = api
        .call(
            Method::PUT,
            "/v1/machine/settings",
            Some(json!({ "max_checks": 3 })),
        )
        .await;
    assert_eq!(
        (s, body),
        (StatusCode::OK, json!({ "max_checks": 3, "sounds": on }))
    );

    let api = api.restart();
    assert_eq!(
        api.get("/v1/machine/settings").await,
        json!({ "max_checks": 3, "sounds": on }),
        "the switches are the database's"
    );
}

#[tokio::test]
async fn a_database_from_before_the_sounds_reads_as_all_off() {
    let api = Api::new();
    // Only `max_checks` was ever stored, as by the daemon of TOKNILA-154.
    api.store
        .write(toknila_core::Actor::Human, crate::server::now(), |tx| {
            tx.repo().app_setting_set("max_checks", "2")
        })
        .await
        .expect("the old row is written");
    let api = api.restart();
    assert_eq!(
        api.get("/v1/machine/settings").await,
        json!({ "max_checks": 2, "sounds": OFF() })
    );
}
