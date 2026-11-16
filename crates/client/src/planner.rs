//! The planner routes: a person starts a project's planner and uses its terminal; the planner
//! itself (a client with its run token) reads and edits the plan.

use toknila_api_types::{
    CardEdit, ChainDetail, ChainSettings, ConversationPage, DesignPin, EdgeInput, EditPlanChain,
    MoveInput, MoveToChain, NewCard, NewChain, NewMessage, NewPlanChain, NewSuggestion, NewThread,
    NewVersion, PlanCard, PlanChain, PlannerActivity, PlannerDesign, PlannerOverview, PlannerPlan,
    PlannerState, Say, StartPlanner, TerminalSize, ThreadDetail, TicketDetail,
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

    // ---- the planner plans, with its run token, on the person's routes (TOKNILA-149) ----

    /// `GET /projects/{project}/plan`.
    pub fn planner_plan(&self, project: &str) -> ClientResult<PlannerPlan> {
        self.call("GET", &format!("/projects/{}/plan", enc(project)), NO_BODY)
    }

    /// `GET /projects/{project}/plan/overview`: the plan in short records, at any size.
    pub fn planner_overview(&self, project: &str) -> ClientResult<PlannerOverview> {
        let path = format!("/projects/{}/plan/overview", enc(project));
        self.call("GET", &path, NO_BODY)
    }

    /// `GET /chains/{chain}/plan`: one chain with every card in full.
    pub fn planner_chain(&self, chain: &str) -> ClientResult<PlanChain> {
        self.call("GET", &format!("/chains/{}/plan", enc(chain)), NO_BODY)
    }

    /// `GET /tickets/{card}/plan`: one card in full.
    pub fn planner_card(&self, card: &str) -> ClientResult<PlanCard> {
        self.call("GET", &format!("/tickets/{}/plan", enc(card)), NO_BODY)
    }

    /// `GET /projects/{project}/designs/{design}`.
    pub fn planner_design(&self, project: &str, design: &str) -> ClientResult<PlannerDesign> {
        let path = format!("/projects/{}/designs/{}", enc(project), enc(design));
        self.call("GET", &path, NO_BODY)
    }

    /// `POST /projects/{project}/design-pins`.
    pub fn planner_pin(
        &self,
        project: &str,
        pin: &toknila_api_types::NewDesignPin,
    ) -> ClientResult<DesignPin> {
        let path = format!("/projects/{}/design-pins", enc(project));
        self.call("POST", &path, Some(pin))
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

    /// `POST /projects/{project}/chains` with the planner's fields; its epic waits for the owner.
    pub fn planner_new_chain(&self, project: &str, c: &NewPlanChain) -> ClientResult<ChainDetail> {
        let path = format!("/projects/{}/chains", enc(project));
        self.call("POST", &path, Some(&NewChain::from(c.clone())))
    }

    /// `PATCH /chains/{chain}` with the planner's fields.
    pub fn planner_edit_chain(&self, chain: &str, e: &EditPlanChain) -> ClientResult<ChainDetail> {
        let settings = ChainSettings::from(e.clone());
        self.call("PATCH", &format!("/chains/{}", enc(chain)), Some(&settings))
    }

    /// `POST /chains/{chain}/tickets` with the planner's fields: at the end of the epic unless it
    /// names what it waits for, its designs by file name or id.
    pub fn planner_add_card(&self, chain: &str, c: &NewCard) -> ClientResult<TicketDetail> {
        let path = format!("/chains/{}/tickets", enc(chain));
        self.call("POST", &path, Some(&c.clone().into_story()))
    }

    /// `PATCH /tickets/{card}` with the planner's fields and checklist changes, in one command.
    pub fn planner_edit_card(&self, card: &str, e: &CardEdit) -> ClientResult<TicketDetail> {
        let patch = e.clone().into_story();
        self.call("PATCH", &format!("/tickets/{}", enc(card)), Some(&patch))
    }

    /// `POST /chains/{chain}/edges`.
    pub fn planner_add_dependency(&self, chain: &str, e: &EdgeInput) -> ClientResult<ChainDetail> {
        let path = format!("/chains/{}/edges", enc(chain));
        self.call("POST", &path, Some(e))
    }

    /// `DELETE /chains/{chain}/edges`.
    pub fn planner_remove_dependency(
        &self,
        chain: &str,
        from: &str,
        to: &str,
    ) -> ClientResult<ChainDetail> {
        let path = format!(
            "/chains/{}/edges?from={}&to={}",
            enc(chain),
            enc(from),
            enc(to)
        );
        self.call("DELETE", &path, NO_BODY)
    }

    /// `POST /chains/{chain}/tickets/{card}/move`: puts `card` next to `at`'s anchor in its epic.
    pub fn planner_move_card(
        &self,
        chain: &str,
        card: &str,
        at: &MoveInput,
    ) -> ClientResult<ChainDetail> {
        let path = format!("/chains/{}/tickets/{}/move", enc(chain), enc(card));
        self.call("POST", &path, Some(at))
    }

    /// `POST /tickets/{card}/move-to-chain?from={chain}`: moves `card` of epic `chain` to epic
    /// `to`, at the end unless `place` says where; answers that epic (`GET /chains/{id}`).
    pub fn planner_move_card_to(
        &self,
        chain: &str,
        card: &str,
        to: &str,
        place: Option<MoveInput>,
    ) -> ClientResult<ChainDetail> {
        let path = format!("/tickets/{}/move-to-chain?from={}", enc(card), enc(chain));
        let body = MoveToChain {
            chain: to.to_owned(),
            place,
        };
        let moved: TicketDetail = self.call("POST", &path, Some(&body))?;
        let epic = moved.ticket.chain_id.to_string();
        self.call("GET", &format!("/chains/{}", enc(&epic)), NO_BODY)
    }

    /// `POST /tickets/{card}/show`.
    pub fn planner_show_card(&self, card: &str) -> ClientResult<TicketDetail> {
        let path = format!("/tickets/{}/show", enc(card));
        self.call("POST", &path, NO_BODY)
    }

    /// `PUT /projects/{project}/suggestion`: keeps the planner's next step, replacing the last
    /// one.
    pub fn planner_suggest(&self, project: &str, s: &NewSuggestion) -> ClientResult<Suggestion> {
        let path = format!("/projects/{}/suggestion", enc(project));
        self.call("PUT", &path, Some(s))
    }

    /// `POST /planner/say`: the planner says something to the person.
    pub fn planner_say(&self, s: &Say) -> ClientResult<Message> {
        self.call("POST", "/planner/say", Some(s))
    }

    /// `POST /planner/activity`: the planner's process reports its turn (a hook fired).
    pub fn planner_activity(&self, a: &PlannerActivity) -> ClientResult<()> {
        self.call("POST", "/planner/activity", Some(a))
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

    /// `POST /tickets/{card}/threads`.
    pub fn planner_open_thread(&self, card: &str, t: &NewThread) -> ClientResult<ThreadDetail> {
        let path = format!("/tickets/{}/threads", enc(card));
        self.call("POST", &path, Some(t))
    }

    /// `POST /tickets/{card}/threads/{thread}/messages`.
    pub fn planner_thread_message(
        &self,
        card: &str,
        thread: &str,
        m: &NewMessage,
    ) -> ClientResult<ThreadDetail> {
        let path = format!("/tickets/{}/threads/{}/messages", enc(card), enc(thread));
        self.call("POST", &path, Some(m))
    }

    /// `POST /tickets/{card}/threads/{thread}/versions`.
    pub fn planner_thread_version(
        &self,
        card: &str,
        thread: &str,
        v: &NewVersion,
    ) -> ClientResult<ThreadDetail> {
        let path = format!("/tickets/{}/threads/{}/versions", enc(card), enc(thread));
        self.call("POST", &path, Some(v))
    }
}
