//! A project's conversation with its assistant, as the window and the CLI read it.

use serde::{Deserialize, Serialize};

/// Query of `GET /v1/projects/{project}/conversation`.
#[cfg_attr(feature = "openapi", derive(utoipa::IntoParams))]
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ConversationQuery {
    /// The id of the oldest message the caller has: the page holds the ones before it. Unset: the
    /// newest.
    pub before: Option<String>,
    /// At most this many (default 50, never more than 200).
    pub limit: Option<usize>,
}
