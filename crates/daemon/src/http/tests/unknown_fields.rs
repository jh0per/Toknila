//! A body with a field the daemon does not know is refused with 400 that names the field and
//! lists the accepted ones, never answered 200 and ignored; a wrong enum value likewise. And the
//! OpenAPI document says so for every body.

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

use super::Api;

/// One request of a body kind: the route, a valid body, and a field the body accepts.
struct Kind {
    name: &'static str,
    method: Method,
    uri: &'static str,
    valid: Value,
    accepted: &'static str,
}

fn kinds() -> Vec<Kind> {
    let k = |name, method, uri, valid, accepted| Kind {
        name,
        method,
        uri,
        valid,
        accepted,
    };
    vec![
        k(
            "project new",
            Method::POST,
            "/v1/projects",
            json!({ "key": "NEW", "name": "New" }),
            "name",
        ),
        k(
            "project patch",
            Method::PATCH,
            "/v1/projects/DEMO",
            json!({ "max_parallel": 2 }),
            "review_mode",
        ),
        k(
            "chain new",
            Method::POST,
            "/v1/projects/DEMO/chains",
            json!({ "name": "other", "goal": "g" }),
            "goal",
        ),
        k(
            "chain patch",
            Method::PATCH,
            "/v1/chains/demo",
            json!({ "goal": "g2" }),
            "autostart",
        ),
        k(
            "ticket new",
            Method::POST,
            "/v1/chains/demo/tickets",
            json!({ "title": "B" }),
            "prompt",
        ),
        k(
            "ticket patch",
            Method::PATCH,
            "/v1/tickets/DEMO-1",
            json!({ "title": "A2" }),
            "review_mode",
        ),
        k(
            "thread new",
            Method::POST,
            "/v1/tickets/DEMO-1/threads",
            json!({ "kind": "design", "title": "T" }),
            "kind",
        ),
        k(
            "thread message",
            Method::POST,
            "/v1/tickets/DEMO-1/threads/1/messages",
            json!({ "text": "hi" }),
            "text",
        ),
        k(
            "thread version",
            Method::POST,
            "/v1/tickets/DEMO-1/threads/1/versions",
            json!({ "format": "markdown", "body": "b" }),
            "format",
        ),
        k(
            "discussion",
            Method::POST,
            "/v1/tickets/DEMO-1/discussion",
            json!({ "text": "hi" }),
            "text",
        ),
        k(
            "settings",
            Method::PUT,
            "/v1/usage/settings",
            json!({ "machine_scan": false }),
            "machine_scan",
        ),
    ]
}

#[tokio::test]
async fn every_body_kind_refuses_an_unknown_field_naming_it_and_the_accepted_ones() {
    let api = Api::new();
    api.demo().await;
    api.post("/v1/chains/demo/tickets", json!({ "title": "A" }))
        .await;
    for kind in kinds() {
        let mut body = kind.valid.clone();
        body["review_modee"] = json!("agent");
        let (status, reply) = api.call(kind.method.clone(), kind.uri, Some(body)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{}: {reply}", kind.name);
        let text = reply["error"].as_str().unwrap_or_default();
        assert!(text.contains("`review_modee`"), "{}: {reply}", kind.name);
        assert!(
            text.contains(&format!("`{}`", kind.accepted)),
            "{} lists its accepted fields: {reply}",
            kind.name
        );
        assert_eq!(reply["code"], "bad_request", "{}: {reply}", kind.name);
    }
    // Nothing was changed by the refused requests.
    let project = api.get("/v1/projects/DEMO").await;
    assert_eq!(project["name"], "Demo");
}

#[tokio::test]
async fn a_wrong_enum_value_is_refused_naming_it_and_the_accepted_ones() {
    let api = Api::new();
    api.demo().await;
    api.post("/v1/chains/demo/tickets", json!({ "title": "A" }))
        .await;
    for (method, uri, body) in [
        (
            Method::PATCH,
            "/v1/tickets/DEMO-1",
            json!({ "review_mode": "sideways" }),
        ),
        (
            Method::PATCH,
            "/v1/projects/DEMO",
            json!({ "review_mode": "sideways" }),
        ),
    ] {
        let (status, reply) = api.call(method, uri, Some(body)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{uri}: {reply}");
        let text = reply["error"].as_str().unwrap_or_default();
        assert!(text.contains("`sideways`"), "{uri}: {reply}");
        assert!(text.contains("`human`"), "{uri}: {reply}");
    }
}

#[tokio::test]
async fn a_valid_body_is_still_taken() {
    let api = Api::new();
    api.demo().await;
    api.post("/v1/chains/demo/tickets", json!({ "title": "A" }))
        .await;
    let (status, reply) = api
        .call(
            Method::PATCH,
            "/v1/tickets/DEMO-1",
            Some(json!({ "title": "A2" })),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{reply}");
    assert_eq!(reply["ticket"]["title"], "A2");
}

/// Every schema reachable from a request body of the document, by name.
fn body_schemas(doc: &Value) -> Vec<String> {
    let mut seen = Vec::<String>::new();
    let mut todo = Vec::<String>::new();
    for item in doc["paths"].as_object().expect("paths").values() {
        for op in item.as_object().expect("operations").values() {
            if let Some(r) =
                op["requestBody"]["content"]["application/json"]["schema"]["$ref"].as_str()
            {
                todo.push(r.rsplit('/').next().expect("name").to_owned());
            }
        }
    }
    while let Some(name) = todo.pop() {
        if seen.contains(&name) {
            continue;
        }
        let schema = &doc["components"]["schemas"][&name];
        let mut refs = Vec::new();
        collect_refs(schema, &mut refs);
        todo.extend(refs);
        seen.push(name);
    }
    seen.sort();
    seen
}

fn collect_refs(v: &Value, out: &mut Vec<String>) {
    match v {
        Value::Object(m) => {
            for (k, x) in m {
                match (k.as_str(), x.as_str()) {
                    ("$ref", Some(r)) => out.push(r.rsplit('/').next().expect("name").to_owned()),
                    _ => collect_refs(x, out),
                }
            }
        }
        Value::Array(a) => a.iter().for_each(|x| collect_refs(x, out)),
        _ => {}
    }
}

/// Whether `schema` is an object that takes any field it likes.
fn open_object(schema: &Value) -> bool {
    if let Some(parts) = schema["allOf"].as_array() {
        return parts.iter().any(open_object);
    }
    schema["type"] == "object"
        && schema["properties"].is_object()
        && schema["additionalProperties"] != json!(false)
}

#[tokio::test]
async fn openapi_says_additional_properties_false_for_every_body() {
    let api = Api::new();
    let req = Request::builder()
        .uri("/openapi.json")
        .body(Body::empty())
        .expect("request");
    let resp = api.app.clone().oneshot(req).await.expect("response");
    let bytes = resp.into_body().collect().await.expect("body").to_bytes();
    let doc: Value = serde_json::from_slice(&bytes).expect("json");
    let names = body_schemas(&doc);
    // The control: the walk finds the bodies it must.
    for must in ["NewProject", "PatchProject", "PatchTicket", "NewThread"] {
        assert!(
            names.iter().any(|n| n == must),
            "{must} not found in {names:?}"
        );
    }
    let open: Vec<&String> = names
        .iter()
        .filter(|n| open_object(&doc["components"]["schemas"][n.as_str()]))
        .collect();
    assert!(
        open.is_empty(),
        "body schemas without additionalProperties false: {open:?}"
    );
}
