//! Every domain type survives JSON serialization and back without loss, with every optional
//! field both filled and empty.

use serde::Serialize;
use serde::de::DeserializeOwned;

use super::*;

fn roundtrip<T>(value: &T)
where
    T: Serialize + DeserializeOwned + PartialEq + std::fmt::Debug,
{
    let json = serde_json::to_string(value).expect("serialize");
    let back: T = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(&back, value, "lost data through {json}");
    // A second pass must produce identical bytes, so nothing is order- or default-dependent.
    assert_eq!(serde_json::to_string(&back).expect("serialize again"), json);
}

fn key(s: &str) -> TicketKey {
    s.parse().expect("valid ticket key")
}

fn ticket(status: TicketStatus, reason: Option<StatusReason>) -> Ticket {
    Ticket {
        id: TicketId::new(),
        chain_id: ChainId::new(),
        key: key("DEMO-3"),
        title: "Додати тест".to_owned(),
        prompt: "Write the test first.\nThen the code.".to_owned(),
        acceptance: vec!["cargo test green".to_owned(), "no unwrap".to_owned()],
        status,
        status_reason: reason,
        status_since: Timestamp(1_793_487_815_000),
        paused: false,
        profile: Some("sim".to_owned()),
        review_mode: Some(ReviewMode::Both),
        branch: None,
        needs_secrets: true,
        tokens_used: 12_345,
    }
}

#[test]
fn project_roundtrips() {
    roundtrip(&Project {
        id: ProjectId::new(),
        key: "DEMO".parse().expect("key"),
        name: "Demo".to_owned(),
        repo_path: Some("C:\\work\\demo".to_owned()),
        work_branch: "develop".to_owned(),
        release_branch: "main".to_owned(),
        default_profile: "sim".to_owned(),
        default_review_mode: ReviewMode::Human,
    });
}

#[test]
fn chain_and_edges_roundtrip() {
    let chain = Chain {
        id: ChainId::new(),
        project_id: ProjectId::new(),
        name: "phase-0".to_owned(),
        goal: "Skeleton".to_owned(),
        autostart: true,
        review_mode: None,
    };
    roundtrip(&chain);
    for kind in [EdgeKind::Normal, EdgeKind::AfterRelease] {
        roundtrip(&Edge {
            chain_id: chain.id,
            from: NodeRef::Release(ReleaseId::new()),
            to: NodeRef::Ticket(TicketId::new()),
            kind,
        });
    }
}

#[test]
fn edge_kind_defaults_to_normal_when_missing() {
    let json = format!(
        r#"{{"chain_id":"{}","from":{{"kind":"ticket","id":"{}"}},"to":{{"kind":"release","id":"{}"}}}}"#,
        ChainId::new(),
        TicketId::new(),
        ReleaseId::new()
    );
    let edge: Edge = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(edge.kind, EdgeKind::Normal);
}

#[test]
fn tickets_roundtrip_in_every_status_and_reason() {
    for status in TicketStatus::ALL {
        roundtrip(&ticket(status, None));
    }
    let reasons = [
        StatusReason::Waiting(WaitingReason::Question {
            text: "який фреймворк?".to_owned(),
        }),
        StatusReason::Waiting(WaitingReason::Permission {
            text: "rm -rf build".to_owned(),
        }),
        StatusReason::Waiting(WaitingReason::Silent),
        StatusReason::Blocked {
            reason: BlockedReason::Manual {
                text: "чекаємо ключ API".to_owned(),
            },
            resume_to: TicketStatus::Running,
            waiting: None,
        },
        StatusReason::Blocked {
            reason: BlockedReason::Crashed {
                detail: "exit 101".to_owned(),
            },
            resume_to: TicketStatus::Waiting,
            waiting: Some(WaitingReason::Silent),
        },
        StatusReason::Blocked {
            reason: BlockedReason::DependencyReopened {
                dependency: key("DEMO-1"),
            },
            resume_to: TicketStatus::Running,
            waiting: None,
        },
        StatusReason::Blocked {
            reason: BlockedReason::MergeConflict {
                detail: "src/lib.rs".to_owned(),
            },
            resume_to: TicketStatus::Review,
            waiting: None,
        },
        StatusReason::Blocked {
            reason: BlockedReason::RunnerLost,
            resume_to: TicketStatus::Running,
            waiting: None,
        },
        StatusReason::Cancelled {
            text: "won't do".to_owned(),
        },
    ];
    for reason in reasons {
        roundtrip(&ticket(TicketStatus::Waiting, Some(reason)));
    }
}

#[test]
fn subtasks_roundtrip() {
    for done_by in [
        None,
        Some(Actor::Agent),
        Some(Actor::Human),
        Some(Actor::System),
    ] {
        roundtrip(&Subtask {
            id: SubtaskId::new(),
            ticket_id: TicketId::new(),
            position: 1,
            text: "write the failing test".to_owned(),
            done: done_by.is_some(),
            done_by,
        });
    }
}

#[test]
fn comments_roundtrip_full_and_sparse() {
    let full = CommentBody {
        gist: "rusqlite and one writer".to_owned(),
        what: Some("Use rusqlite".to_owned()),
        why: Some("No races".to_owned()),
        how_to_verify: Some("cargo test".to_owned()),
        alternatives: Some("sqlx".to_owned()),
        consequences: Some("No DATABASE_URL".to_owned()),
    };
    let sparse = CommentBody {
        gist: "Solid".to_owned(),
        ..CommentBody::default()
    };
    for (kind, body) in [
        (CommentKind::Decision, full),
        (CommentKind::Instruction, sparse),
    ] {
        roundtrip(&Comment {
            id: CommentId::new(),
            ticket_id: TicketId::new(),
            author: Actor::Human,
            kind,
            body,
            created_at: Timestamp(1),
            delivered_at: Some(Timestamp(2)),
        });
    }
}

#[test]
fn sparse_comment_body_omits_empty_fields() {
    let body = CommentBody {
        gist: "Solid".to_owned(),
        ..CommentBody::default()
    };
    assert_eq!(
        serde_json::to_string(&body).expect("serialize"),
        r#"{"gist":"Solid"}"#
    );
}

#[test]
fn releases_roundtrip() {
    let channels = [
        ReleaseChannel::TagOnly,
        ReleaseChannel::TestFlight,
        ReleaseChannel::PlayInternal,
        ReleaseChannel::PlayClosed,
        ReleaseChannel::PlayProduction,
    ];
    for channel in channels {
        for state in [
            ReleaseState::Planned,
            ReleaseState::Cut,
            ReleaseState::Released,
        ] {
            roundtrip(&Release {
                id: ReleaseId::new(),
                chain_id: ChainId::new(),
                version: "1.0.0".to_owned(),
                channel,
                start_mode: StartMode::Manual,
                state,
                branch: Some("release/1.0.0".to_owned()),
                cut_at: Some(Timestamp(10)),
                released_at: None,
            });
        }
    }
}

#[test]
fn agent_profile_roundtrips() {
    roundtrip(&AgentProfile::sim());
    for role in [AgentRole::Worker, AgentRole::Reviewer, AgentRole::Release] {
        roundtrip(&AgentProfile {
            name: "x".to_owned(),
            role,
            kind: ProfileKind::Sim,
        });
    }
}

#[test]
fn pinned_ticket_json_shape() {
    // Other processes (CLI --json, web page) read this shape; changes must be deliberate.
    let mut t = ticket(TicketStatus::Blocked, None);
    t.id = "01JBQ7ZJ0000000000000000T1".parse().expect("ulid");
    t.chain_id = "01JBQ7ZJ0000000000000000C1".parse().expect("ulid");
    t.status_reason = Some(StatusReason::Blocked {
        reason: BlockedReason::Manual {
            text: "key".to_owned(),
        },
        resume_to: TicketStatus::Running,
        waiting: None,
    });
    let json = serde_json::to_string(&t).expect("serialize");
    assert_eq!(
        json,
        concat!(
            r#"{"id":"01JBQ7ZJ0000000000000000T1","chain_id":"01JBQ7ZJ0000000000000000C1","#,
            r#""key":"DEMO-3","title":"Додати тест","prompt":"Write the test first.\nThen the code.","#,
            r#""acceptance":["cargo test green","no unwrap"],"status":"blocked","#,
            r#""status_reason":{"status":"blocked","reason":{"kind":"manual","text":"key"},"resume_to":"running"},"#,
            r#""status_since":1793487815000,"paused":false,"profile":"sim","review_mode":"both","#,
            r#""branch":null,"needs_secrets":true,"tokens_used":12345}"#
        )
    );
}
