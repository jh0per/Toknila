//! A query parameter the daemon does not know is refused with 400 that names it and lists the
//! known ones, never answered 200 and ignored; a value of the wrong type names its parameter.

use axum::http::{Method, StatusCode};

use super::Api;

#[tokio::test]
async fn usage_refuses_an_unknown_window_parameter() {
    let api = Api::new();
    let (status, body) = api.call(Method::GET, "/v1/usage?window=30", None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(
        body["error"],
        "unknown query parameter `window`, expected one of `days`, `tz`"
    );
}

#[tokio::test]
async fn usage_refuses_a_wrong_type_naming_the_parameter() {
    let api = Api::new();
    let (status, body) = api.call(Method::GET, "/v1/usage?days=many", None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    let text = body["error"].as_str().expect("a sentence");
    assert!(text.contains("`days`"), "{text}");
}

#[tokio::test]
async fn every_route_that_reads_a_query_refuses_an_unknown_parameter() {
    let api = Api::new();
    api.demo_with(&["A"]).await;
    for uri in [
        "/v1/tickets?nope=1",
        "/v1/events?nope=1",
        "/v1/chains?nope=1",
        "/v1/projects/DEMO/usage/today?nope=1",
        "/v1/projects/DEMO/conversation?nope=1",
    ] {
        let (status, body) = api.call(Method::GET, uri, None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{uri}: {body}");
        let text = body["error"].as_str().expect("a sentence");
        assert!(
            text.starts_with("unknown query parameter `nope`, expected one of "),
            "{uri}: {text}"
        );
    }
    let (status, _) = api.call(Method::GET, "/v1/usage?days=30", None).await;
    assert_eq!(status, StatusCode::OK);
}
