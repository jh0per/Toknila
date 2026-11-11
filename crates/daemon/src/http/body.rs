//! The JSON body of a request: axum's `Json`, except that a body the daemon cannot read (an
//! unknown field, a wrong enum value, a missing field) is a 400 in the API's error shape that
//! carries serde's message, which names the offender and lists what is accepted.

use axum::extract::rejection::JsonRejection;
use axum::extract::{FromRequest, OptionalFromRequest, Request};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use serde::de::DeserializeOwned;

use super::error::ErrorBody;

/// A JSON request body (or response); see the module doc.
#[derive(Debug, Clone, Copy, Default)]
pub struct Json<T>(pub T);

impl<T: Serialize> IntoResponse for Json<T> {
    fn into_response(self) -> Response {
        axum::Json(self.0).into_response()
    }
}

/// A body that was refused, as the API's error shape.
#[derive(Debug)]
pub enum BodyRejection {
    /// Not JSON, or not the right content type.
    Raw(JsonRejection),
    /// JSON the target type does not take (a field it does not know, a wrong value).
    Data(String),
}

/// The fields a struct names when it is deserialised; the first struct asked for wins.
struct Capture<D> {
    inner: D,
    fields: std::rc::Rc<std::cell::RefCell<Option<&'static [&'static str]>>>,
}

macro_rules! forward {
    ($($m:ident($($a:ident: $t:ty),*);)*) => {$(
        fn $m<V: serde::de::Visitor<'de>>(self, $($a: $t,)* visitor: V) -> Result<V::Value, Self::Error> {
            self.inner.$m($($a,)* visitor)
        }
    )*};
}

impl<'de, D: serde::Deserializer<'de>> serde::Deserializer<'de> for Capture<D> {
    type Error = D::Error;

    forward! {
        deserialize_any(); deserialize_bool(); deserialize_i8(); deserialize_i16();
        deserialize_i32(); deserialize_i64(); deserialize_i128(); deserialize_u8();
        deserialize_u16(); deserialize_u32(); deserialize_u64(); deserialize_u128();
        deserialize_f32(); deserialize_f64(); deserialize_char(); deserialize_str();
        deserialize_string(); deserialize_bytes(); deserialize_byte_buf(); deserialize_option();
        deserialize_unit(); deserialize_seq(); deserialize_map(); deserialize_identifier();
        deserialize_ignored_any(); deserialize_unit_struct(n: &'static str);
        deserialize_newtype_struct(n: &'static str); deserialize_tuple(l: usize);
        deserialize_tuple_struct(n: &'static str, l: usize);
        deserialize_enum(n: &'static str, v: &'static [&'static str]);
    }

    fn deserialize_struct<V: serde::de::Visitor<'de>>(
        self,
        name: &'static str,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.fields.borrow_mut().get_or_insert(fields);
        self.inner.deserialize_struct(name, fields, visitor)
    }
}

/// `value` as a `T`, or the reason it is not: the first field `T` ignored (naming the path and,
/// for the top-level struct, the accepted fields) or serde's own message.
fn read<T: DeserializeOwned>(value: serde_json::Value) -> Result<T, String> {
    let fields = std::rc::Rc::new(std::cell::RefCell::new(None));
    let mut ignored = Vec::<String>::new();
    let capture = Capture {
        inner: value,
        fields: fields.clone(),
    };
    let parsed = serde_ignored::deserialize(capture, |path| ignored.push(path.to_string()));
    let item = parsed.map_err(|e| e.to_string())?;
    let Some(path) = ignored.first() else {
        return Ok(item);
    };
    let name = path.rsplit('.').next().unwrap_or(path);
    let mut message = format!("unknown field `{name}` at {}", display_path(path));
    // Only the top-level fields are known; a nested path names its own offender.
    if let Some(list) = *fields.borrow()
        && !path.contains('.')
    {
        let list: Vec<String> = list.iter().map(|f| format!("`{f}`")).collect();
        message.push_str(&format!(", expected one of {}", list.join(", ")));
    }
    Err(message)
}

/// serde_ignored's path (`designs.0.regionn` style) as `.designs[0].regionn`.
fn display_path(path: &str) -> String {
    let mut out = String::new();
    for part in path.split('.') {
        if part.chars().all(|c| c.is_ascii_digit()) && !part.is_empty() {
            out.push_str(&format!("[{part}]"));
        } else {
            out.push('.');
            out.push_str(part);
        }
    }
    out
}

impl IntoResponse for BodyRejection {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            Self::Data(message) => (StatusCode::BAD_REQUEST, message),
            Self::Raw(rejection) => {
                let status = match rejection {
                    // Valid JSON the types do not accept: a bad request, not "unprocessable".
                    JsonRejection::JsonDataError(_) => StatusCode::BAD_REQUEST,
                    ref other => other.status(),
                };
                let text = rejection.body_text();
                let message = text
                    .strip_prefix("Failed to deserialize the JSON body into the target type: ")
                    .or_else(|| text.strip_prefix("Failed to parse the request body as JSON: "))
                    .unwrap_or(&text)
                    .to_owned();
                (status, message)
            }
        };
        let body = ErrorBody {
            code: "bad_request".to_owned(),
            error: message,
            field: None,
            used_by: None,
        };
        (status, axum::Json(body)).into_response()
    }
}

impl<T, S> FromRequest<S> for Json<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = BodyRejection;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let axum::Json(value) =
            <axum::Json<serde_json::Value> as FromRequest<S>>::from_request(req, state)
                .await
                .map_err(BodyRejection::Raw)?;
        read(value).map(Self).map_err(BodyRejection::Data)
    }
}

impl<T, S> OptionalFromRequest<S> for Json<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = BodyRejection;

    async fn from_request(req: Request, state: &S) -> Result<Option<Self>, Self::Rejection> {
        // No body at all is "no input"; a body that is there must be readable.
        if !req.headers().contains_key(header::CONTENT_TYPE) {
            return Ok(None);
        }
        <Self as FromRequest<S>>::from_request(req, state)
            .await
            .map(Some)
    }
}

#[cfg(test)]
mod tests {
    use super::read;
    use serde::Deserialize;
    use serde_json::json;

    #[derive(Debug, Deserialize)]
    #[allow(dead_code)]
    struct Inner {
        region: u8,
    }

    #[derive(Debug, Deserialize)]
    #[allow(dead_code)]
    struct Outer {
        name: String,
        designs: Vec<Inner>,
    }

    #[test]
    fn a_top_level_unknown_field_is_named_with_the_accepted_ones() {
        let e =
            read::<Outer>(json!({ "name": "n", "designs": [], "nmae": 1 })).expect_err("refused");
        assert_eq!(
            e,
            "unknown field `nmae` at .nmae, expected one of `name`, `designs`"
        );
    }

    #[test]
    fn a_nested_unknown_field_is_named_by_its_path() {
        let e = read::<Outer>(json!({ "name": "n", "designs": [{ "region": 1, "regionn": 2 }] }))
            .expect_err("refused");
        assert_eq!(e, "unknown field `regionn` at .designs[0].regionn");
    }
}
