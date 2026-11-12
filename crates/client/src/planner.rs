//! The planner routes: a person starts a project's planner and uses its terminal; the planner
//! itself (a client with its run token) reads and edits the plan.

use toknila_api_types::{
    CardEdit, ChainDetail, ConversationPage, DesignPin, Say, EdgeInput, EditPlanChain, MoveInput, NewCard, NewMessage,
    NewPlanChain, NewSuggestion, NewThread, NewVersion, PlanCard, PlanChain, PlannerDesign,
    PlannerOverview, PlannerPlan, PlannerState, StartPlanner, TerminalSize, ThreadDetail,
    TicketDetail,
};
use toknila_core::RunId;
use toknila_core::conversation::Message;
use toknila_core::suggestion::Suggestion;

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

    /// `POST /projects/{project}/planner/stop`; stopping a stopped planner is fine, so a lost
    /// connection is retried.
    pub fn stop_planner(&self, project: &str) -> ClientResult<PlannerState> {
        let path = format!("/projects/{}/planner/stop", enc(project));
        self.call_idempotent("POST", &path, Some(&serde_json::json!({})))
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
    pub fn planner_plan(&self) -> ClientResult<PlannerPlan> {
        self.call("GET", "/planner/plan", NO_BODY)
    }

    /// `GET /planner/overview`: the plan in short records, at any size.
    pub fn planner_overview(&self) -> ClientResult<PlannerOverview> {
        self.call("GET", "/planner/overview", NO_BODY)
    }

    /// `GET /planner/chains/{chain}`: one chain with every card in full.
    pub fn planner_chain(&self, chain: &str) -> ClientResult<PlanChain> {
        self.call("GET", &format!("/planner/chains/{}", enc(chain)), NO_BODY)
    }

    /// `GET /planner/cards/{card}`: one card in full.
    pub fn planner_card(&self, card: &str) -> ClientResult<PlanCard> {
        self.call("GET", &format!("/planner/cards/{}", enc(card)), NO_BODY)
    }

    /// `GET /planner/designs/{design}`.
    pub fn planner_design(&self, design: &str) -> ClientResult<PlannerDesign> {
        self.call("GET", &format!("/planner/designs/{}", enc(design)), NO_BODY)
    }

    /// `POST /planner/pins`.
    pub fn planner_pin(&self, pin: &toknila_api_types::NewDesignPin) -> ClientResult<DesignPin> {
        self.call("POST", "/planner/pins", Some(pin))
    }

    /// `GET /projects/{project}/design-pins`.
    pub fn design_pins(&self, project: &str) -> ClientResult<Vec<DesignPin>> {
        let path = format!("/projects/{}/design-pins", enc(project));
        self.call("GET", &path, NO_BODY)
    }

    /// `POST /projects/{project}/design-pins/{pin}/answer`.
    pub fn answer_pin(&self, project: &str, pin: &str, text: &str) -> ClientResult<DesignPin> {
        let path = format!("/projects/{}/design-pins/{}/answer", enc(project), enc(pin));
        let body = toknila_api_types::PinAnswer {
            text: text.to_owned(),
        };
        self.call("POST", &path, Some(&body))
    }

    /// `POST /planner/chains`.
    pub fn planner_new_chain(&self, c: &NewPlanChain) -> ClientResult<ChainDetail> {
        self.call("POST", "/planner/chains", Some(c))
    }

    /// `PATCH /planner/chains/{chain}`.
    pub fn planner_edit_chain(&self, chain: &str, e: &EditPlanChain) -> ClientResult<ChainDetail> {
        self.call("PATCH", &format!("/planner/chains/{}", enc(chain)), Some(e))
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

    /// `POST /planner/suggestion`: keeps the planner's next step, replacing the last one.
    pub fn planner_suggest(&self, s: &NewSuggestion) -> ClientResult<Suggestion> {
        self.call("POST", "/planner/suggestion", Some(s))
    }

    /// `POST /planner/say`: the planner says something to the person.
    pub fn planner_say(&self, s: &Say) -> ClientResult<Message> {
        self.call("POST", "/planner/say", Some(s))
    }

    /// `GET /projects/{project}/conversation`: a page of the project's conversation with its
    /// assistant; `before` is the id of the oldest message already read.
    pub fn conversation(
        &self,
        project: &str,
        before: Option<&str>,
        limit: Option<usize>,
    ) -> ClientResult<ConversationPage> {
        let mut query = Vec::new();
        if let Some(b) = before {
            query.push(format!("before={}", enc(b)));
        }
        if let Some(l) = limit {
            query.push(format!("limit={l}"));
        }
        let query = if query.is_empty() {
            String::new()
        } else {
            format!("?{}", query.join("&"))
        };
        let path = format!("/projects/{}/conversation{query}", enc(project));
        self.call("GET", &path, NO_BODY)
    }

    /// `GET /projects/{project}/suggestion`: the planner's standing suggestion, if any.
    pub fn suggestion(&self, project: &str) -> ClientResult<Option<Suggestion>> {
        let path = format!("/projects/{}/suggestion", enc(project));
        self.call("GET", &path, NO_BODY)
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
