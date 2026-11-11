//! The OpenAPI document says a request body takes no field it does not list: every object schema
//! reachable from a request body gets `additionalProperties: false`. The types themselves stay
//! tolerant (a stored row with an extra field must still load); the daemon refuses at the boundary,
//! see `body.rs`.

use serde_json::Value;
use utoipa::openapi::OpenApi;

/// `api` with every object schema reachable from a request body closed.
pub fn close_bodies(api: OpenApi) -> OpenApi {
    let Ok(mut doc) = serde_json::to_value(&api) else {
        return api;
    };
    let mut todo = Vec::<String>::new();
    if let Some(paths) = doc["paths"].as_object() {
        for op in paths
            .values()
            .filter_map(Value::as_object)
            .flat_map(|o| o.values())
        {
            let schema = &op["requestBody"]["content"]["application/json"]["schema"];
            refs(schema, &mut todo);
        }
    }
    let mut seen = Vec::<String>::new();
    while let Some(name) = todo.pop() {
        if seen.contains(&name) {
            continue;
        }
        if let Some(schema) = doc["components"]["schemas"].get_mut(&name) {
            refs(schema, &mut todo);
            close(schema);
        }
        seen.push(name);
    }
    serde_json::from_value(doc).unwrap_or(api)
}

/// The component names `v` refers to, anywhere inside it.
fn refs(v: &Value, out: &mut Vec<String>) {
    match v {
        Value::Object(m) => {
            for (k, x) in m {
                match (k.as_str(), x.as_str()) {
                    ("$ref", Some(r)) => out.push(r.rsplit('/').next().unwrap_or(r).to_owned()),
                    _ => refs(x, out),
                }
            }
        }
        Value::Array(a) => a.iter().for_each(|x| refs(x, out)),
        _ => {}
    }
}

/// Close the object `schema` and the object schemas written inline inside it.
fn close(schema: &mut Value) {
    let Some(m) = schema.as_object_mut() else {
        return;
    };
    if m.get("type").and_then(Value::as_str) == Some("object") && m.contains_key("properties") {
        m.insert("additionalProperties".to_owned(), Value::Bool(false));
    }
    for key in ["allOf", "oneOf", "anyOf"] {
        if let Some(parts) = m.get_mut(key).and_then(Value::as_array_mut) {
            parts.iter_mut().for_each(close);
        }
    }
    if let Some(props) = m.get_mut("properties").and_then(Value::as_object_mut) {
        props.values_mut().for_each(close);
    }
    if let Some(items) = m.get_mut("items") {
        close(items);
    }
}
