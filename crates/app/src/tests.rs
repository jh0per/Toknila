//! Use cases over an in-memory implementation of the ports: no SQLite, no HTTP.

use serde_json::Value;
use toknila_core::agent::AgentEvent;
use toknila_core::events::DomainEvent;
use toknila_core::graph::ChainView;
use toknila_core::{
    Actor, AgentProfile, Chain, ChainId, Comment, CommentKind, Edge, EdgeKind, NodeRef, Project,
    ProjectId, Release, ReleaseId, ReviewMode, Subtask, Ticket, TicketDraft, TicketId, TicketKey,
    TicketStatus, Timestamp,
};

use crate::notes::{add_comment, body_from_text};
use crate::recovery::{Recovery, recover};
use crate::tickets::{NewTicket, TicketAction};
use crate::{
    AppResult, ErrorKind, Reader, UnitOfWork, agent, chains, resolve, tickets, with_chain,
};

#[derive(Debug)]
struct Mem {
    project: Project,
    view: ChainView,
    comments: Vec<Comment>,
    subtasks: Vec<Subtask>,
    events: Vec<DomainEvent>,
    now: Timestamp,
    taken: u32,
}

impl Mem {
    /// Project DEMO with chain `demo`: DEMO-1 -> DEMO-2, both in Backlog, autostart on.
    fn demo() -> Self {
        let project = Project {
            id: ProjectId::new(),
            key: "DEMO".parse().expect("key"),
            name: "Demo".to_owned(),
            repo_path: None,
            work_branch: Project::DEFAULT_WORK_BRANCH.to_owned(),
            release_branch: Project::DEFAULT_RELEASE_BRANCH.to_owned(),
            default_profile: AgentProfile::SIM.to_owned(),
            default_review_mode: ReviewMode::Human,
        };
        let chain = Chain {
            id: ChainId::new(),
            project_id: project.id,
            name: "demo".to_owned(),
            goal: String::new(),
            autostart: true,
            review_mode: None,
        };
        let now = Timestamp(1_000);
        let tickets: Vec<Ticket> = ["A", "B"]
            .iter()
            .enumerate()
            .map(|(i, title)| {
                let key = format!("DEMO-{}", i + 1).parse().expect("key");
                let draft = TicketDraft {
                    title: (*title).to_owned(),
                    ..TicketDraft::default()
                };
                Ticket::new_backlog(TicketId::new(), chain.id, key, draft, now)
            })
            .collect();
        let edges = vec![Edge {
            chain_id: chain.id,
            from: NodeRef::Ticket(tickets[0].id),
            to: NodeRef::Ticket(tickets[1].id),
            kind: EdgeKind::default(),
        }];
        let view = ChainView {
            chain,
            tickets,
            releases: vec![],
            edges,
            release_tickets: vec![],
        };
        Self {
            project,
            view,
            comments: vec![],
            subtasks: vec![],
            events: vec![],
            now,
            taken: 2,
        }
    }

    fn ticket_by_title(&self, title: &str) -> Ticket {
        resolve::ticket(self, title).expect("ticket")
    }

    fn kinds(&self) -> Vec<String> {
        self.events
            .iter()
            .map(|e| format!("{}.{}", e.entity(), e.kind()))
            .collect()
    }
}

impl Reader for Mem {
    fn projects(&self) -> AppResult<Vec<Project>> {
        Ok(vec![self.project.clone()])
    }
    fn chain_by_name(&self, project: ProjectId, name: &str) -> AppResult<Option<Chain>> {
        let c = &self.view.chain;
        Ok((c.project_id == project && c.name == name).then(|| c.clone()))
    }
    fn project(&self, id: ProjectId) -> AppResult<Option<Project>> {
        Ok((self.project.id == id).then(|| self.project.clone()))
    }
    fn project_by_key(&self, key: &str) -> AppResult<Option<Project>> {
        Ok((self.project.key.to_string() == key).then(|| self.project.clone()))
    }
    fn chain(&self, id: ChainId) -> AppResult<Option<Chain>> {
        Ok((self.view.chain.id == id).then(|| self.view.chain.clone()))
    }
    fn chains(&self, _: Option<ProjectId>) -> AppResult<Vec<Chain>> {
        Ok(vec![self.view.chain.clone()])
    }
    fn chains_named(&self, name: &str) -> AppResult<Vec<Chain>> {
        Ok(self
            .chains(None)?
            .into_iter()
            .filter(|c| c.name == name)
            .collect())
    }
    fn ticket(&self, id: TicketId) -> AppResult<Option<Ticket>> {
        Ok(self.view.tickets.iter().find(|t| t.id == id).cloned())
    }
    fn ticket_by_key(&self, key: &TicketKey) -> AppResult<Option<Ticket>> {
        Ok(self.view.tickets.iter().find(|t| &t.key == key).cloned())
    }
    fn tickets(&self, _: Option<ChainId>) -> AppResult<Vec<Ticket>> {
        Ok(self.view.tickets.clone())
    }
    fn release(&self, id: ReleaseId) -> AppResult<Option<Release>> {
        Ok(self.view.releases.iter().find(|r| r.id == id).cloned())
    }
    fn releases_by_version(&self, version: &str) -> AppResult<Vec<Release>> {
        Ok(self
            .view
            .releases
            .iter()
            .filter(|r| r.version == version)
            .cloned()
            .collect())
    }
    fn chain_view(&self, chain: ChainId) -> AppResult<Option<ChainView>> {
        Ok((self.view.chain.id == chain).then(|| self.view.clone()))
    }
    fn subtasks(&self, ticket: TicketId) -> AppResult<Vec<Subtask>> {
        Ok(self
            .subtasks
            .iter()
            .filter(|s| s.ticket_id == ticket)
            .cloned()
            .collect())
    }
    fn comments(&self, ticket: TicketId) -> AppResult<Vec<Comment>> {
        Ok(self
            .comments
            .iter()
            .filter(|c| c.ticket_id == ticket)
            .cloned()
            .collect())
    }
    fn last_event_payload(&self, entity: &str, id: &str) -> AppResult<Option<Value>> {
        let last = self
            .events
            .iter()
            .rev()
            .find(|e| e.entity() == entity && e.entity_id() == id);
        Ok(last.map(|e| serde_json::to_value(e).expect("event json")))
    }
}

impl UnitOfWork for Mem {
    fn now(&self) -> Timestamp {
        self.now
    }
    fn actor(&self) -> Actor {
        Actor::Human
    }
    fn insert_project(&mut self, p: &Project) -> AppResult<()> {
        self.project = p.clone();
        Ok(())
    }
    fn insert_chain(&mut self, c: &Chain) -> AppResult<()> {
        self.view = ChainView {
            chain: c.clone(),
            tickets: vec![],
            releases: vec![],
            edges: vec![],
            release_tickets: vec![],
        };
        Ok(())
    }
    fn take_ticket_number(&mut self, _: ProjectId) -> AppResult<u32> {
        self.taken += 1;
        Ok(self.taken)
    }
    fn save_chain_view(&mut self, view: &ChainView) -> AppResult<()> {
        self.view = view.clone();
        Ok(())
    }
    fn upsert_comment(&mut self, c: &Comment) -> AppResult<()> {
        self.comments.retain(|x| x.id != c.id);
        self.comments.push(c.clone());
        Ok(())
    }
    fn upsert_subtask(&mut self, s: &Subtask) -> AppResult<()> {
        self.subtasks.retain(|x| x.id != s.id);
        self.subtasks.push(s.clone());
        Ok(())
    }
    fn emit(&mut self, event: &DomainEvent) -> AppResult<()> {
        self.events.push(event.clone());
        Ok(())
    }
}

#[test]
fn recovery_settles_and_reports_drift_without_a_journal() {
    let mut mem = Mem::demo();
    let report = recover(&mut mem).expect("recover");
    assert_eq!(
        report,
        Recovery {
            chains: 1,
            tickets: 2,
            running: 0,
            drift: vec!["DEMO-1".to_owned(), "DEMO-2".to_owned()],
        }
    );
    assert_eq!(mem.ticket_by_title("A").status, TicketStatus::Running);
    assert_eq!(
        mem.kinds(),
        ["ticket.status_changed", "ticket.status_changed"]
    );
    // The journal now holds a snapshot of DEMO-1; DEMO-2 never changed.
    assert_eq!(recover(&mut mem).expect("again").drift, ["DEMO-2"]);
}

#[test]
fn sim_agents_get_instructions_on_the_next_round() {
    let mut mem = Mem::demo();
    recover(&mut mem).expect("settle");
    let a = mem.ticket_by_title("A");
    let view = mem.view.clone();
    let body = body_from_text("add a test");
    add_comment(
        &mut mem,
        &view,
        a.id,
        Actor::Human,
        CommentKind::Instruction,
        body,
    )
    .expect("comment");
    assert_eq!(mem.comments[0].delivered_at, None);
    with_chain(&mut mem, a.chain_id, |_| Ok::<_, crate::AppError>(())).expect("round");
    assert_eq!(mem.comments[0].delivered_at, Some(mem.now));
    assert_eq!(
        mem.kinds()[2..],
        ["comment.added".to_owned(), "comment.delivered".to_owned()]
    );
}

#[test]
fn finished_moves_to_review_and_keeps_the_report() {
    let mut mem = Mem::demo();
    recover(&mut mem).expect("settle");
    let a = mem.ticket_by_title("A");
    let summary = AgentEvent::Finished {
        summary: "done\nall green".to_owned(),
    };
    agent::ticket_event(&mut mem, &a, summary).expect("finished");
    assert_eq!(mem.ticket_by_title("A").status, TicketStatus::Review);
    let report = &mem.comments[0];
    assert_eq!(
        (report.author, report.kind, report.body.gist.as_str()),
        (Actor::Agent, CommentKind::Report, "done")
    );
    assert_eq!(report.body.what.as_deref(), Some("all green"));
}

#[test]
fn agent_events_are_checked_before_anything_changes() {
    let mut mem = Mem::demo();
    recover(&mut mem).expect("settle");
    let before = mem.events.len();
    let a = mem.ticket_by_title("A");
    let empty = AgentEvent::Finished {
        summary: " ".to_owned(),
    };
    let e = agent::ticket_event(&mut mem, &a, empty).expect_err("empty summary");
    assert_eq!(
        (e.kind, e.message.as_str()),
        (ErrorKind::BadRequest, "finished needs a summary")
    );
    let b = mem.ticket_by_title("B");
    let e = agent::ticket_event(&mut mem, &b, AgentEvent::Started).expect_err("backlog");
    assert_eq!(e.kind, ErrorKind::Conflict);
    assert_eq!(mem.events.len(), before);
}

#[test]
fn new_ticket_waits_for_its_dependency_and_gets_its_checklist() {
    let mut mem = Mem::demo();
    recover(&mut mem).expect("settle");
    let input = NewTicket {
        title: " C ".to_owned(),
        after: vec!["demo-2".to_owned()],
        subtasks: vec!["write it".to_owned()],
        ..NewTicket::default()
    };
    let id = tickets::create(&mut mem, "demo", input).expect("create");
    let c = mem.ticket_by_title("C");
    assert_eq!((c.id, c.key.to_string()), (id, "DEMO-3".to_owned()));
    assert_eq!(c.status, TicketStatus::Backlog);
    assert_eq!(mem.subtasks.len(), 1);
    let edge = mem.view.edges.last().expect("edge");
    let b = mem.ticket_by_title("B");
    assert_eq!(
        (edge.from, edge.to),
        (NodeRef::Ticket(b.id), NodeRef::Ticket(id))
    );
    let e = tickets::create(&mut mem, "demo", NewTicket::default()).expect_err("no title");
    assert_eq!(e.message, "ticket title is empty");
}

#[test]
fn ticket_actions_parse_and_demand_their_text() {
    let block: TicketAction = "block".parse().expect("known");
    assert_eq!(
        block
            .event(Some("  ".to_owned()))
            .expect_err("blank")
            .message,
        "block needs a text"
    );
    assert!(block.event(Some("waiting on CI".to_owned())).is_ok());
    let e = "launch".parse::<TicketAction>().expect_err("unknown");
    assert_eq!(
        (e.kind, e.message.as_str()),
        (ErrorKind::NotFound, "action launch not found")
    );
}

#[test]
fn export_then_import_conflicts_on_the_same_name() {
    let mut mem = Mem::demo();
    let toml = chains::export(&mem, "demo").expect("export");
    assert!(
        toml.contains("id = \"DEMO-2\"\ntitle = \"B\"\nafter = [\"DEMO-1\"]"),
        "{toml}"
    );
    let e = chains::import(&mut mem, "DEMO", &toml).expect_err("same name");
    assert_eq!(
        (e.kind, e.message.as_str()),
        (ErrorKind::Conflict, "chain demo already exists in DEMO")
    );
}
