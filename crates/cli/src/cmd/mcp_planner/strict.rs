//! The arguments of a planner tool call, read strictly: a key the tool does not take is refused.

use serde_json::{Value, json};

use super::super::mcp_tools::ToolError;

/// The arguments as a `T`; no arguments at all are an empty object.
pub(super) fn args<T: serde::de::DeserializeOwned>(arguments: &Value) -> Result<T, ToolError> {
    let value = match arguments {
        Value::Null => json!({}),
        other => other.clone(),
    };
    serde_json::from_value(value).map_err(|e| ToolError::BadArguments(e.to_string()))
}

/// A key the tool's schema does not name is refused, listing the keys it takes; serde would drop
/// it silently, and a misspelled `modle` would then look like a success.
pub(super) fn refuse_unknown_keys(
    name: &str,
    tool: &Value,
    arguments: &Value,
) -> Result<(), ToolError> {
    let (Value::Object(given), Some(known)) =
        (arguments, tool["inputSchema"]["properties"].as_object())
    else {
        return Ok(());
    };
    let unknown: Vec<&str> = given
        .keys()
        .map(String::as_str)
        .filter(|k| !known.contains_key(*k))
        .collect();
    if unknown.is_empty() {
        return Ok(());
    }
    let accepted: Vec<&str> = known.keys().map(String::as_str).collect();
    Err(ToolError::BadArguments(format!(
        "{name} takes no {}; its arguments are {}",
        unknown.join(", "),
        accepted.join(", ")
    )))
}
