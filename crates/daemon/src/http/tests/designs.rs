//! Designs through the full router: upload, list, serve, survive a restart, Figma links with and
//! without a token (against a fake Figma on localhost), the token's whereabouts, the chain export.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

use super::{Api, TOKEN};
use crate::designs::figma::TEST_API;
use crate::designs::tests::PNG;

/// A request with a raw body; the answer's status, content type and bytes.
pub(super) async fn raw(
    api: &Api,
    method: Method,
    uri: &str,
    body: &[u8],
) -> (StatusCode, String, Vec<u8>) {
    let req = Request::builder()
        .method(method)
        .uri(uri)
        .header("authorization", format!("Bearer {TOKEN}"))
        .header("content-type", "application/octet-stream")
        .body(Body::from(body.to_vec()))
        .expect("request");
    let resp = api.app.clone().oneshot(req).await.expect("response");
    let status = resp.status();
    let ctype = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_owned();
    let bytes = resp.into_body().collect().await.expect("body").to_bytes();
    (status, ctype, bytes.to_vec())
}

async fn upload(api: &Api, name: &str, body: &[u8]) -> (StatusCode, Value) {
    let (s, _, bytes) = raw(
        api,
        Method::POST,
        &format!("/v1/projects/DEMO/designs?name={name}"),
        body,
    )
    .await;
    (s, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}

#[tokio::test]
async fn a_png_is_listed_served_and_still_there_after_a_restart() {
    let api = Api::new();
    api.post("/v1/projects", json!({ "key": "DEMO" })).await;
    let (s, made) = upload(&api, "home.png", PNG).await;
    assert_eq!(s, StatusCode::CREATED, "{made}");
    assert_eq!(
        (made["kind"].as_str(), made["thumb"].as_bool()),
        (Some("image"), Some(true))
    );
    let id = made["id"].as_str().expect("id").to_owned();

    let api = api.restart();
    let list = api.get("/v1/projects/DEMO/designs").await;
    assert_eq!(list["designs"][0]["name"], "home.png");
    assert_eq!(list["figma_token"], false);
    let (s, ctype, bytes) = raw(
        &api,
        Method::GET,
        &format!("/v1/projects/DEMO/designs/{id}/file"),
        b"",
    )
    .await;
    assert_eq!(
        (s, ctype.as_str(), bytes.as_slice()),
        (StatusCode::OK, "image/png", PNG)
    );
}

#[tokio::test]
async fn only_images_and_pdfs_are_kept_and_a_pdf_is_one_design() {
    let api = Api::new();
    api.post("/v1/projects", json!({ "key": "DEMO" })).await;
    let (s, e) = upload(&api, "x.png", b"not an image").await;
    assert_eq!(
        (s, e["code"].as_str()),
        (StatusCode::BAD_REQUEST, Some("bad_request"))
    );
    let (s, pdf) = upload(&api, "flow.pdf", b"%PDF-1.7 two pages").await;
    assert_eq!(s, StatusCode::CREATED);
    assert_eq!(
        (pdf["kind"].as_str(), pdf["thumb"].as_bool()),
        (Some("pdf"), Some(false))
    );
    let list = api.get("/v1/projects/DEMO/designs").await;
    assert_eq!(list["designs"].as_array().map(Vec::len), Some(1));
    let (s, _) = api
        .call(Method::GET, "/v1/projects/NOPE/designs", None)
        .await;
    assert_eq!(s, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn rename_and_remove() {
    let api = Api::new();
    api.post("/v1/projects", json!({ "key": "DEMO" })).await;
    let (_, made) = upload(&api, "a.png", PNG).await;
    let id = made["id"].as_str().expect("id");
    let uri = format!("/v1/projects/DEMO/designs/{id}");
    let (s, r) = api
        .call(Method::PATCH, &uri, Some(json!({ "name": "b.png" })))
        .await;
    assert_eq!((s, r["name"].as_str()), (StatusCode::OK, Some("b.png")));
    let (s, _) = api
        .call(Method::PATCH, &uri, Some(json!({ "name": "" })))
        .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    let (s, _) = api.call(Method::DELETE, &uri, None).await;
    assert_eq!(s, StatusCode::NO_CONTENT);
    let (s, _, _) = raw(&api, Method::GET, &format!("{uri}/file"), b"").await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    let (s, _) = api.call(Method::DELETE, &uri, None).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
}

const LINK: &str = "https://www.figma.com/design/AbC123/App?node-id=1-2";

#[tokio::test]
async fn a_figma_link_without_a_token_is_kept_with_an_export_hint() {
    let api = Api::new();
    api.post("/v1/projects", json!({ "key": "DEMO" })).await;
    let made = api
        .post(
            "/v1/projects/DEMO/designs/figma",
            json!({ "url": LINK, "name": "Login" }),
        )
        .await;
    assert_eq!(made["kind"], "figma");
    assert_eq!(made["figma"]["file_key"], "AbC123");
    assert_eq!(made["figma"]["node_id"], "1:2");
    assert_eq!(made["has_file"], false);
    assert!(
        made["hint"]
            .as_str()
            .unwrap_or("")
            .contains("Export the frame"),
        "{made}"
    );
    let (s, _) = api
        .call(
            Method::POST,
            "/v1/projects/DEMO/designs/figma",
            Some(json!({ "url": "https://example.com/x" })),
        )
        .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    // Fetching without a token is refused, with the way out.
    let id = made["id"].as_str().expect("id");
    let (s, e) = api
        .call(
            Method::POST,
            &format!("/v1/projects/DEMO/designs/{id}/fetch"),
            None,
        )
        .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    assert!(e["error"].as_str().unwrap_or("").contains("token"), "{e}");
}

/// A Figma on localhost: answers the images call with a link to itself, then the png. Returns
/// its address and the request heads it saw.
fn fake_figma(images_json: fn(u16) -> String) -> (String, Arc<Mutex<Vec<String>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().expect("addr").port();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = seen.clone();
    std::thread::spawn(move || {
        for _ in 0..2 {
            let Ok((mut conn, _)) = listener.accept() else {
                return;
            };
            let mut head = Vec::new();
            let mut buf = [0u8; 1024];
            while !head.windows(4).any(|w| w == b"\r\n\r\n") {
                match conn.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => head.extend_from_slice(&buf[..n]),
                }
            }
            let head = String::from_utf8_lossy(&head).into_owned();
            let first = head.lines().next().unwrap_or("").to_owned();
            log.lock().expect("log").push(head);
            let (ctype, body) = if first.contains("/v1/images/") {
                ("application/json", images_json(port).into_bytes())
            } else {
                ("image/png", PNG.to_vec())
            };
            let _ = write!(
                conn,
                "HTTP/1.1 200 OK\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = conn.write_all(&body);
        }
    });
    (format!("http://127.0.0.1:{port}"), seen)
}

/// The fake Figma's address is one process-wide setting: the tests that use it take turns.
static FIGMA_TURN: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[tokio::test]
async fn with_a_token_the_frame_is_fetched_and_the_token_stays_in_its_file() {
    let _turn = FIGMA_TURN.lock().await;
    let (base, seen) = fake_figma(|port| {
        format!(r#"{{"err":null,"images":{{"1:2":"http://127.0.0.1:{port}/frame.png"}}}}"#)
    });
    *TEST_API.lock().expect("lock") = Some(base);
    let api = Api::new();
    api.post("/v1/projects", json!({ "key": "DEMO" })).await;
    api.post("/v1/projects", json!({ "key": "COPY" })).await;
    let secret = "figd_TOPSECRET-123";
    let (s, _) = api
        .call(
            Method::PUT,
            "/v1/projects/DEMO/figma-token",
            Some(json!({ "token": secret })),
        )
        .await;
    assert_eq!(s, StatusCode::NO_CONTENT);
    let made = api
        .post(
            "/v1/projects/DEMO/designs/figma",
            json!({ "url": LINK, "name": "Login" }),
        )
        .await;
    *TEST_API.lock().expect("lock") = None;
    assert_eq!(
        (made["has_file"].as_bool(), made["mime"].as_str()),
        (Some(true), Some("image/png")),
        "{made}"
    );
    assert_eq!(made["hint"], Value::Null);
    let id = made["id"].as_str().expect("id");
    let (_, _, bytes) = raw(
        &api,
        Method::GET,
        &format!("/v1/projects/DEMO/designs/{id}/file"),
        b"",
    )
    .await;
    assert_eq!(bytes, PNG);

    // The first request carried the header and asked for the right frame; the image download
    // carried no token.
    let heads = seen.lock().expect("seen").clone();
    assert!(
        heads[0].contains("/v1/images/AbC123?ids=1%3A2&format=png"),
        "{}",
        heads[0]
    );
    assert!(
        heads[0]
            .to_lowercase()
            .contains(&format!("x-figma-token: {secret}").to_lowercase()),
        "{}",
        heads[0]
    );
    assert!(!heads[1].contains(secret), "{}", heads[1]);

    // Where the token is: one private file under secrets/, in no API answer, no index, no export.
    let home = api._dir.path();
    let files: Vec<_> = std::fs::read_dir(home.join("secrets"))
        .expect("secrets")
        .flatten()
        .collect();
    assert_eq!(files.len(), 1);
    assert_eq!(
        std::fs::read_to_string(files[0].path())
            .expect("token")
            .trim(),
        secret
    );
    assert_eq!(
        api.get("/v1/projects/DEMO/designs").await["figma_token"],
        true
    );
    assert_eq!(
        api.get("/v1/projects/COPY/designs").await["figma_token"],
        false
    );
    for url in [
        "/v1/projects/DEMO/designs",
        "/v1/projects/DEMO",
        "/v1/projects",
    ] {
        assert!(!api.get(url).await.to_string().contains(secret), "{url}");
    }
    for entry in walk(&home.join("designs")) {
        assert!(
            !std::fs::read_to_string(&entry)
                .unwrap_or_default()
                .contains(secret),
            "{entry:?}"
        );
    }
    // Forgetting it removes the file.
    let (s, _) = api
        .call(Method::DELETE, "/v1/projects/DEMO/figma-token", None)
        .await;
    assert_eq!(s, StatusCode::NO_CONTENT);
    assert_eq!(
        api.get("/v1/projects/DEMO/designs").await["figma_token"],
        false
    );
}

fn walk(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let p = e.path();
        if p.is_dir() {
            out.extend(walk(&p))
        } else {
            out.push(p)
        }
    }
    out
}

#[tokio::test]
async fn a_refused_fetch_keeps_the_link_with_the_reason_and_no_token_in_it() {
    let _turn = FIGMA_TURN.lock().await;
    let (base, _seen) = fake_figma(|_| r#"{"err":"Not found","images":{}}"#.to_owned());
    *TEST_API.lock().expect("lock") = Some(base);
    let api = Api::new();
    api.post("/v1/projects", json!({ "key": "DEMO" })).await;
    let secret = "figd_TOPSECRET-999";
    api.call(
        Method::PUT,
        "/v1/projects/DEMO/figma-token",
        Some(json!({ "token": secret })),
    )
    .await;
    let made = api
        .post("/v1/projects/DEMO/designs/figma", json!({ "url": LINK }))
        .await;
    *TEST_API.lock().expect("lock") = None;
    assert_eq!(made["has_file"], false);
    let hint = made["hint"].as_str().unwrap_or("");
    assert!(hint.contains("Figma said: Not found"), "{made}");
    assert!(!made.to_string().contains(secret));
}

#[tokio::test]
async fn a_chain_export_names_designs_by_file_name_only() {
    let api = Api::new();
    api.demo().await;
    upload(&api, "home.png", PNG).await;
    api.post(
        "/v1/projects/DEMO/designs/figma",
        json!({ "url": LINK, "name": "Login frame" }),
    )
    .await;
    api.call(
        Method::PUT,
        "/v1/projects/DEMO/figma-token",
        Some(json!({ "token": "figd_TOPSECRET-1" })),
    )
    .await;
    let toml = api.get("/v1/chains/demo/export").await["toml"]
        .as_str()
        .expect("toml")
        .to_owned();
    assert!(
        toml.contains("designs = [\"home.png\", \"Login frame\"]"),
        "{toml}"
    );
    for leak in ["figma.com", "AbC123", "figd_", "secrets", "/designs/"] {
        assert!(!toml.contains(leak), "{leak} in {toml}");
    }
    // The file imports into another project.
    api.post("/v1/projects", json!({ "key": "COPY" })).await;
    let (s, _) = api
        .call(
            Method::POST,
            "/v1/projects/COPY/import",
            Some(json!({ "toml": toml })),
        )
        .await;
    assert_eq!(s, StatusCode::CREATED);
}

#[tokio::test]
async fn deleting_the_project_deletes_its_designs_and_token() {
    let api = Api::new();
    api.post("/v1/projects", json!({ "key": "DEMO" })).await;
    upload(&api, "home.png", PNG).await;
    api.call(
        Method::PUT,
        "/v1/projects/DEMO/figma-token",
        Some(json!({ "token": "figd_x" })),
    )
    .await;
    let home = api._dir.path().to_owned();
    assert!(!walk(&home.join("designs")).is_empty());
    let (s, _) = api
        .call(Method::POST, "/v1/projects/DEMO/delete", Some(json!({})))
        .await;
    assert_eq!(s, StatusCode::OK);
    assert!(walk(&home.join("designs")).is_empty());
    assert!(walk(&home.join("secrets")).is_empty());
}
