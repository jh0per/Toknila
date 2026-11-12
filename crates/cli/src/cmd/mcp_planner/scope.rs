//! What `show_plan` reads: the overview, one chain in full, or one card in full.

use serde::Deserialize;
use serde_json::Value;

use super::super::mcp_tools::ToolError;

/// What `show_plan` reads.
#[derive(Debug, PartialEq, Eq)]
pub(in crate::cmd) enum PlanScope {
    Overview,
    Chain(String),
    Card(String),
}

#[derive(Deserialize)]
struct ShowPlan {
    chain: Option<String>,
    card: Option<String>,
}

/// The scope the arguments of `show_plan` name; `chain` and `card` together are refused.
pub(super) fn plan_scope(arguments: &Value) -> Result<PlanScope, ToolError> {
    let ShowPlan { chain, card } = super::args(arguments)?;
    match (chain, card) {
        (None, None) => Ok(PlanScope::Overview),
        (Some(c), None) => Ok(PlanScope::Chain(c)),
        (None, Some(c)) => Ok(PlanScope::Card(c)),
        (Some(_), Some(_)) => Err(ToolError::BadArguments(
            "give `chain` or `card`, not both".to_owned(),
        )),
    }
}
