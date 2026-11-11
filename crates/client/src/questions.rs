//! The question routes: a person's answer, and the agent's permission ask.

use crate::{Client, ClientError, ClientResult, enc, transport_error};
use toknila_api_types::{AnswerOutcome, NewAnswer, PermissionAsk, PermissionReply};

impl Client {
    /// `POST /tickets/{ticket}/runner/answer`: a person's answer to what the agent asks.
    /// With "Allow always" the daemon says what it added; any other answer gives `None`.
    pub fn answer(&self, ticket: &str, a: &NewAnswer) -> ClientResult<Option<AnswerOutcome>> {
        self.call(
            "POST",
            &format!("/tickets/{}/runner/answer", enc(ticket)),
            Some(a),
        )
    }

    /// `POST /tickets/{ticket}/permission`: the agent's permission hook asks and waits (up to
    /// `wait`) for the person's decision.
    pub fn permission(
        &self,
        ticket: &str,
        ask: &PermissionAsk,
        wait: std::time::Duration,
    ) -> ClientResult<PermissionReply> {
        let url = format!("{}/v1/tickets/{}/permission", self.base, enc(ticket));
        let json = serde_json::to_value(ask).map_err(|e| ClientError::Decode(e.to_string()))?;
        let text = self
            .agent
            .post(&url)
            .timeout(wait)
            .set("Authorization", &format!("Bearer {}", self.token))
            .send_json(json)
            .map_err(transport_error)?
            .into_string()
            .map_err(|e| ClientError::Decode(e.to_string()))?;
        serde_json::from_str(&text).map_err(|e| ClientError::Decode(e.to_string()))
    }
}
