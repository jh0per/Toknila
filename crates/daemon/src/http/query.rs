//! The query string of a request: axum's `Query`, except that a parameter the target type does
//! not know is a 400 in the API's error shape naming it and the known ones, and a value of the
//! wrong type is a 400 naming the parameter. The sibling of the strict body in `body.rs`.

use axum::extract::{FromRequestParts, Query as AxumQuery};
use axum::http::request::Parts;
use axum::response::{IntoResponse, Response};
use serde::de::DeserializeOwned;

use super::body::Capture;
use super::error::ErrorBody;

/// A strict query string; see the module doc.
#[derive(Debug, Clone, Copy, Default)]
pub struct Query<T>(pub T);

/// A query string that was refused, as the API's error shape.
#[derive(Debug)]
pub struct QueryRejection(String);

impl IntoResponse for QueryRejection {
    fn into_response(self) -> Response {
        let body = ErrorBody {
            code: "bad_request".to_owned(),
            error: self.0,
            field: None,
            used_by: None,
        };
        (axum::http::StatusCode::BAD_REQUEST, axum::Json(body)).into_response()
    }
}

/// The parameter names `T` takes, as serde reports them for its struct; `None` for a type that
/// is not a struct (a map takes anything).
fn known<T: DeserializeOwned>() -> Option<&'static [&'static str]> {
    let fields = std::rc::Rc::new(std::cell::RefCell::new(None));
    let probe = Capture {
        inner: serde_json::Value::Object(Default::default()),
        fields: fields.clone(),
    };
    // Only the field list matters; the empty object may well fail a required field.
    let _ = T::deserialize(probe);
    fields.take()
}

/// `raw` (the part after `?`) as a `T`, or the sentence that says why not.
fn read<T: DeserializeOwned>(raw: &str) -> Result<T, String> {
    let pairs: Vec<(String, String)> = form_pairs(raw);
    if let Some(list) = known::<T>() {
        // `token` is the bearer token a browser puts in a WebSocket URL; the auth layer, which
        // runs before any handler, reads it on every route.
        if let Some((name, _)) = pairs
            .iter()
            .find(|(k, _)| k != "token" && !list.contains(&k.as_str()))
        {
            let list: Vec<String> = list.iter().map(|f| format!("`{f}`")).collect();
            return Err(format!(
                "unknown query parameter `{name}`, expected one of {}",
                list.join(", ")
            ));
        }
    }
    let uri = format!("/?{raw}")
        .parse::<axum::http::Uri>()
        .map_err(|e| format!("the query string is not valid: {e}"))?;
    AxumQuery::<T>::try_from_uri(&uri)
        .map(|q| q.0)
        .map_err(|e| wrong_value::<T>(&pairs, e.body_text()))
}

/// The sentence for a value `T` refused: it names the first parameter that does not parse alone.
fn wrong_value<T: DeserializeOwned>(pairs: &[(String, String)], text: String) -> String {
    let reason = text
        .strip_prefix("Failed to deserialize query string: ")
        .unwrap_or(&text);
    for (name, value) in pairs {
        let one = format!("/?{}={}", enc(name), enc(value));
        let Ok(uri) = one.parse::<axum::http::Uri>() else {
            continue;
        };
        if let Err(e) = AxumQuery::<T>::try_from_uri(&uri) {
            let why = e.body_text();
            // A missing sibling is not this parameter's fault.
            if !why.contains("missing field") {
                let why = why
                    .strip_prefix("Failed to deserialize query string: ")
                    .unwrap_or(&why);
                return format!("query parameter `{name}` is not valid: {why}");
            }
        }
    }
    format!("the query string is not valid: {reason}")
}

/// The `key=value` pairs of a query string, percent-decoded.
fn form_pairs(raw: &str) -> Vec<(String, String)> {
    raw.split('&')
        .filter(|p| !p.is_empty())
        .map(|p| {
            let (k, v) = p.split_once('=').unwrap_or((p, ""));
            (decode(k), decode(v))
        })
        .collect()
}

fn decode(s: &str) -> String {
    let s = s.replace('+', " ");
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = bytes
            .get(i + 1..i + 3)
            .and_then(|h| std::str::from_utf8(h).ok())
            .and_then(|h| u8::from_str_radix(h, 16).ok());
        match (bytes[i], hex) {
            (b'%', Some(b)) => {
                out.push(b);
                i += 3;
            }
            (b, _) => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Percent-encodes everything that is not unreserved.
fn enc(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

impl<T, S> FromRequestParts<S> for Query<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = QueryRejection;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        read(parts.uri.query().unwrap_or(""))
            .map(Self)
            .map_err(QueryRejection)
    }
}

#[cfg(test)]
mod tests {
    use super::read;
    use serde::Deserialize;

    #[derive(Debug, Deserialize, PartialEq)]
    struct Usage {
        days: Option<u32>,
        project: Option<String>,
    }

    #[test]
    fn an_unknown_parameter_is_named_with_the_known_ones() {
        let e = read::<Usage>("window=30").expect_err("refused");
        assert_eq!(
            e,
            "unknown query parameter `window`, expected one of `days`, `project`"
        );
    }

    #[test]
    fn a_wrong_type_names_the_parameter() {
        let e = read::<Usage>("days=many").expect_err("refused");
        assert!(
            e.starts_with("query parameter `days` is not valid: "),
            "{e}"
        );
    }

    #[test]
    fn the_auth_token_is_accepted_on_any_route() {
        let q = read::<Usage>("days=7&token=abc").expect("accepted");
        assert_eq!(q.days, Some(7));
    }

    #[test]
    fn known_parameters_pass_and_none_is_fine() {
        assert_eq!(
            read::<Usage>("days=30&project=a%20b").expect("accepted"),
            Usage {
                days: Some(30),
                project: Some("a b".into())
            }
        );
        assert_eq!(
            read::<Usage>("").expect("accepted"),
            Usage {
                days: None,
                project: None
            }
        );
    }
}
