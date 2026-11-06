//! Reading one profile file: TOML into a validated [`AgentProfile`], each problem reported with
//! its line and column.

use serde::Deserialize;
use std::collections::BTreeMap;
use toml::Spanned;

use crate::{AgentProfile, AgentRole, HookWiring, McpWiring, Nudge, OutputRules};

/// A problem in a profile file. `line` and `column` are 1-based.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{line}:{column}: {message}")]
pub struct ProfileError {
    pub line: usize,
    pub column: usize,
    pub message: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Raw {
    role: Option<AgentRole>,
    command: Spanned<Vec<String>>,
    #[serde(default)]
    env: BTreeMap<String, Spanned<String>>,
    instructions_file: Option<String>,
    #[serde(default)]
    mcp: McpWiring,
    #[serde(default)]
    hooks: HookWiring,
    #[serde(default)]
    output: OutputRules,
    #[serde(default)]
    nudge: Nudge,
}

/// Whether `name` can name a profile: lowercase letters, digits, `-` and `_`, starting with a
/// letter or digit.
pub fn valid_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphanumeric())
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
}

/// Parses and validates the profile file `text` as the profile `name`.
pub fn parse(name: &str, text: &str) -> Result<AgentProfile, ProfileError> {
    let at = |offset: usize, message: String| {
        let before = &text[..offset.min(text.len())];
        let line = before.matches('\n').count() + 1;
        let column = before.rsplit('\n').next().map_or(0, |l| l.chars().count()) + 1;
        ProfileError {
            line,
            column,
            message,
        }
    };
    if !valid_name(name) {
        return Err(at(
            0,
            format!("{name:?} is not a profile name: use lowercase letters, digits, - and _"),
        ));
    }
    let raw: Raw = toml::from_str(text).map_err(|e| {
        at(
            e.span().map_or(0, |s| s.start),
            e.message().trim().to_owned(),
        )
    })?;
    let command = raw.command.get_ref();
    if command
        .first()
        .is_none_or(|program| program.trim().is_empty())
    {
        return Err(at(
            raw.command.span().start,
            "command must start with a program name".to_owned(),
        ));
    }
    for (key, value) in &raw.env {
        if key.is_empty() || key.contains('=') {
            return Err(at(
                value.span().start,
                format!("environment variable name {key:?} is not valid"),
            ));
        }
    }
    if raw.nudge.idle_secs == 0 {
        let offset = text.find("idle_secs").unwrap_or(0);
        return Err(at(offset, "nudge.idle_secs must be at least 1".to_owned()));
    }
    Ok(AgentProfile {
        name: name.to_owned(),
        role: raw.role.unwrap_or(AgentRole::Worker),
        command: raw.command.into_inner(),
        env: raw
            .env
            .into_iter()
            .map(|(k, v)| (k, v.into_inner()))
            .collect(),
        instructions_file: raw.instructions_file,
        mcp: raw.mcp,
        hooks: raw.hooks,
        output: raw.output,
        nudge: raw.nudge,
    })
}
