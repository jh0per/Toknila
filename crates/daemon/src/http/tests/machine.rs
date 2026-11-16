//! The machine settings route: `max_checks` defaults to 1, is set through `PUT`, reaches the
//! line of checks at once and survives a restart.

use axum::http::{Method, StatusCode};
use serde_json::json;

use super::Api;

#[tokio::test]
async fn max_checks_is_one_until_a_person_raises_it_and_the_line_follows() {
    let api = Api::new();
    assert_eq!(
        api.get("/v1/machine/settings").await,
        json!({ "max_checks": 1 }),
        "nothing changes until a person raises it"
    );
    assert_eq!(api.store.check_line().max_checks(), 1);

    let (s, body) = api
        .call(
            Method::PUT,
            "/v1/machine/settings",
            Some(json!({ "max_checks": 3 })),
        )
        .await;
    assert_eq!((s, body), (StatusCode::OK, json!({ "max_checks": 3 })));
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
    assert_eq!((s, body), (StatusCode::OK, json!({ "max_checks": 1 })));
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
