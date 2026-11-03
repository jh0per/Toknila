//! The read-only web page, embedded at build time and served without auth: the files hold no
//! data, and the page reads the token from its own address (`/?token=`) for every API call.

use axum::extract::Path;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};

include!(concat!(env!("OUT_DIR"), "/web_assets.rs"));

const NOT_BUILT: &str = "<!doctype html><meta charset=utf-8><title>Toknila</title>\
<p>This build has no web page. Build it with <code>cargo xtask web</code>, then rebuild the daemon.</p>";

/// Whether this binary carries the built page.
pub fn embedded() -> bool {
    ASSETS.iter().any(|(p, _, _)| *p == "index.html")
}

fn file(path: &str) -> Response {
    match ASSETS.iter().find(|(p, _, _)| *p == path) {
        Some((p, bytes, mime)) => {
            // Hashed asset names never change content; the page itself must always be fresh.
            let cache = if p.starts_with("assets/") {
                "public, max-age=31536000, immutable"
            } else {
                "no-cache"
            };
            (
                [
                    (header::CONTENT_TYPE, *mime),
                    (header::CACHE_CONTROL, cache),
                ],
                *bytes,
            )
                .into_response()
        }
        None if path == "index.html" => (
            [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
            NOT_BUILT,
        )
            .into_response(),
        None => (StatusCode::NOT_FOUND, "not found").into_response(),
    }
}

/// `GET /`.
pub async fn index() -> Response {
    file("index.html")
}

/// `GET /assets/{*path}`.
pub async fn asset(Path(path): Path<String>) -> Response {
    file(&format!("assets/{path}"))
}
