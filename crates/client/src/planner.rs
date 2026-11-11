//! The planner routes: a person starts a project's planner and uses its terminal; the planner
//! itself (a client with its run token) reads and edits the plan.

use toknila_api_types::{
    CardEdit, ChainDetail, EdgeInput, MoveInput, NewCard, NewMessage, NewPlanChain, NewThread,
    NewVersion, PlanView, PlannerState, StartPlanner, TerminalSize, ThreadDetail, TicketDetail,
};
use toknila_core::RunId;

use crate::{Client, ClientResult, NO_BODY, Scrollback, enc, transport_error};

impl Client {
    // ---- a person runs the planner ----

    /// `GET /projects/{project}/planner`.
    pub fn planner(&self, project: &str) -> ClientResult<PlannerState> {
        self.call(
            "GET",
            &format!("/projects/{}/planner", enc(project)),
            NO_BODY,
        )
    }

    /// `POST /projects/{project}/planner/start`; `profile` names a planner profile.
    pub fn start_planner(
        &self,
        project: &str,
        profile: Option<&str>,
    ) -> ClientResult<PlannerState> {
        let body = StartPlanner {
            profile: profile.map(str::to_owned),
        };
        let path = format!("/projects/{}/planner/start", enc(project));
        self.call("POST", &path, Some(&body))
    }

    /// `POST /projects/{project}/planner/stop`.
    pub fn stop_planner(&self, project: &str) -> ClientResult<PlannerState> {
        let path = format!("/projects/{}/planner/stop", enc(project));
        self.call("POST", &path, Some(&serde_json::json!({})))
    }

    /// `GET /projects/{project}/planner/scrollback`.
    pub fn planner_scrollback(&self, project: &str) -> ClientResult<Scrollback> {
        let url = format!(
            "{}/v1/projects/{}/planner/scrollback",
            self.base,
            enc(project)
        );
        self.scrollback_at(&url)
    }

    /// `POST /projects/{project}/planner/input`: types `bytes` into the planner's terminal.
    pub fn planner_input(&self, project: &str, bytes: &[u8]) -> ClientResult<()> {
        let url = format!("{}/v1/projects/{}/planner/input", self.base, enc(project));
        self.agent
            .post(&url)
            .set("Authorization", &format!("Bearer {}", self.token))
            .set("Content-Type", "application/octet-stream")
            .send_bytes(bytes)
            .map(|_| ())
            .map_err(transport_error)
    }

    /// `POST /projects/{project}/planner/size`.
    pub fn planner_size(&self, project: &str, size: &TerminalSize) -> ClientResult<()> {
        let path = format!("/projects/{}/planner/size", enc(project));
        self.call("POST", &path, Some(size))
    }

    /// The WebSocket address of the planner's live terminal output, continuing after `cursor`.
    pub fn planner_stream_url(&self, project: &str, cursor: Option<(RunId, u64)>) -> String {
        let ws = self.base.replacen("http://", "ws://", 1);
        let from = cursor.map_or_else(String::new, |(run, at)| format!("&run={run}&from={at}"));
        format!(
            "{ws}/v1/projects/{}/planner/stream?token={}{from}",
            enc(project),
            self.token
        )
    }

    // ---- the planner plans, with its run token ----

    /// `GET /planner/plan`.
    pub fn planner_plan(&self) -> ClientResult<PlanView> {
        self.call("GET", "/planner/plan", NO_BODY)
    }

    /// `POST /planner/chains`.
    pub fn planner_new_chain(&self, c: &NewPlanChain) -> ClientResult<ChainDetail> {
        self.call("POST", "/planner/chains", Some(c))
    }

    /// `POST /planner/chains/{chain}/cards`.
    pub fn planner_add_card(&self, chain: &str, c: &NewCard) -> ClientResult<TicketDetail> {
        let path = format!("/planner/chains/{}/cards", enc(chain));
        self.call("POST", &path, Some(c))
    }

    /// `PATCH /planner/cards/{card}`.
    pub fn planner_edit_card(&self, card: &str, e: &CardEdit) -> ClientResult<TicketDetail> {
        self.call("PATCH", &format!("/planner/cards/{}", enc(card)), Some(e))
    }

    /// `POST /planner/chains/{chain}/edges`.
    pub fn planner_add_dependency(&self, chain: &str, e: &EdgeInput) -> ClientResult<ChainDetail> {
        let path = format!("/planner/chains/{}/edges", enc(chain));
        self.call("POST", &path, Some(e))
    }

    /// `DELETE /planner/chains/{chain}/edges`.
    pub fn planner_remove_dependency(
        &self,
        chain: &str,
        from: &str,
        to: &str,
    ) -> ClientResult<ChainDetail> {
        let path = format!(
            "/planner/chains/{}/edges?from={}&to={}",
            enc(chain),
            enc(from),
            enc(to)
        );
        self.call("DELETE", &path, NO_BODY)
    }

    /// `POST /planner/chains/{chain}/cards/{card}/move`.
    pub fn planner_move_card(
        &self,
        chain: &str,
        card: &str,
        m: &MoveInput,
    ) -> ClientResult<ChainDetail> {
        let path = format!("/planner/chains/{}/cards/{}/move", enc(chain), enc(card));
        self.call("POST", &path, Some(m))
    }

    /// `POST /planner/cards/{card}/show`.
    pub fn planner_show_card(&self, card: &str) -> ClientResult<TicketDetail> {
        let path = format!("/planner/cards/{}/show", enc(card));
        self.call("POST", &path, NO_BODY)
    }

    /// `POST /planner/cards/{card}/threads`.
    pub fn planner_open_thread(&self, card: &str, t: &NewThread) -> ClientResult<ThreadDetail> {
        let path = format!("/planner/cards/{}/threads", enc(card));
        self.call("POST", &path, Some(t))
    }

    /// `POST /planner/cards/{card}/threads/{thread}/messages`.
    pub fn planner_thread_message(
        &self,
        card: &str,
        thread: &str,
        m: &NewMessage,
    ) -> ClientResult<ThreadDetail> {
        let path = format!(
            "/planner/cards/{}/threads/{}/messages",
            enc(card),
            enc(thread)
        );
        self.call("POST", &path, Some(m))
    }

    /// `POST /planner/cards/{card}/threads/{thread}/versions`.
    pub fn planner_thread_version(
        &self,
        card: &str,
        thread: &str,
        v: &NewVersion,
    ) -> ClientResult<ThreadDetail> {
        let path = format!(
            "/planner/cards/{}/threads/{}/versions",
            enc(card),
            enc(thread)
        );
        self.call("POST", &path, Some(v))
    }
}
