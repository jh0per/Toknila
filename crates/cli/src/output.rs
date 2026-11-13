//! Human output: tables and one-line summaries. `--json` bypasses all of this.

use serde::Serialize;
use serde_json::Value;
use toknila_api_types::{ChainDetail, EventRecord, TicketDetail, TicketItem};
use toknila_core::wire::wire_name;
use toknila_core::{Actor, CommentKind, NodeRef, StatusReason};

use crate::check_words::check_words_now;

/// Who wrote a comment, in words: the daemon's own comments are Toknila's, never the agent's.
fn author_word(a: Actor) -> String {
    match a {
        Actor::System => "Toknila".to_owned(),
        other => w(&other),
    }
}

/// `90_000` ms -> `1m30s`.
pub fn duration(ms: u64) -> String {
    let s = ms / 1000;
    match s {
        0..60 => format!("{s}s"),
        60..3600 => format!("{}m{:02}s", s / 60, s % 60),
        3600..86_400 => format!("{}h{:02}m", s / 3600, (s % 3600) / 60),
        _ => format!("{}d{:02}h", s / 86_400, (s % 86_400) / 3600),
    }
}

/// Reason of a Waiting/Blocked/Cancelled status in a few words; empty when there is none.
pub fn reason(r: Option<&StatusReason>) -> String {
    r.map(ToString::to_string).unwrap_or_default()
}

/// ` (reason)`, or nothing.
fn in_parens(r: Option<&StatusReason>) -> String {
    match reason(r) {
        r if r.is_empty() => r,
        r => format!(" ({r})"),
    }
}

/// The wire name of an enum value (`running`, `tag_only`).
pub fn w<T: Serialize>(v: &T) -> String {
    wire_name(v).unwrap_or_default()
}

/// Prints rows as left-aligned columns.
pub fn table(header: &[&str], rows: &[Vec<String>]) {
    let mut widths: Vec<usize> = header.iter().map(|h| h.chars().count()).collect();
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            if let Some(w) = widths.get_mut(i) {
                *w = (*w).max(cell.chars().count());
            }
        }
    }
    let line = |cells: &[String]| {
        let parts: Vec<String> = cells
            .iter()
            .enumerate()
            .map(|(i, c)| {
                if i + 1 == cells.len() {
                    c.clone()
                } else {
                    format!("{c:<w$}", w = widths[i])
                }
            })
            .collect();
        println!("{}", parts.join("  ").trim_end());
    };
    line(&header.iter().map(|h| (*h).to_owned()).collect::<Vec<_>>());
    for row in rows {
        line(row);
    }
}

/// The status as a person reads it: a card Running under the simulated agent has no process
/// behind it, so it never reads plain `running`.
pub fn status_word(status: &toknila_core::TicketStatus, simulated: bool) -> String {
    if simulated && *status == toknila_core::TicketStatus::Running {
        "simulated".to_owned()
    } else {
        w(status)
    }
}

/// ` (queued #2)` for a card waiting for an agent slot, else nothing.
fn queued(position: Option<u32>) -> String {
    position.map_or_else(String::new, |n| format!(" (queued #{n})"))
}

/// `KEY status (reason)`: the answer of ticket actions and agent events.
pub fn ticket_line(d: &TicketDetail) {
    let t = &d.ticket;
    println!(
        "{} {}{}{}",
        t.key,
        status_word(&t.status, d.simulated),
        queued(d.queue_position),
        in_parens(t.status_reason.as_ref())
    );
}

/// `tok ls` rows from `GET /v1/tickets`.
pub fn tickets(items: &[TicketItem]) {
    let rows: Vec<Vec<String>> = items
        .iter()
        .map(|it| {
            let t = &it.ticket;
            let mut status = status_word(&t.status, it.simulated);
            if t.paused {
                status.push_str(" (paused)");
            }
            status.push_str(&queued(it.queue_position));
            if t.review.turn == Some(toknila_core::ReviewTurn::Check) {
                let words = check_words_now(it.check.as_ref());
                status.push_str(&format!(" ({words})"));
            }
            vec![
                t.key.to_string(),
                status,
                duration(it.in_status_ms),
                it.chain.clone(),
                t.title.clone(),
                reason(t.status_reason.as_ref()),
            ]
        })
        .collect();
    table(&["KEY", "STATUS", "FOR", "CHAIN", "TITLE", "REASON"], &rows);
}

/// `tok chain show` from `GET /v1/chains/{chain}`.
pub fn chain(d: &ChainDetail) {
    let s = &d.summary;
    let name = match &s.version {
        Some(v) => format!("{} v{v}", d.chain.name),
        None => d.chain.name.clone(),
    };
    println!(
        "{}  {}  {}  done {}/{}  released {}{}",
        name,
        s.phase.word(),
        w(&s.status),
        s.done,
        s.total,
        s.released,
        if d.chain.autostart {
            ""
        } else {
            "  (stories start only when you start them)"
        }
    );
    if let Some(f) = &s.finish {
        let tag = f
            .tag
            .as_ref()
            .map(|t| format!(", tag {t}"))
            .unwrap_or_default();
        println!("Release v{}: {}{tag}", f.version, w(&f.state));
    }
    if let Some(stop) = &s.stopped_at {
        let r = match reason(stop.reason.as_ref()) {
            r if r.is_empty() => r,
            r => format!(": {r}"),
        };
        println!(
            "stopped at {} ({} for {}{r})",
            stop.ticket,
            w(&stop.status),
            duration(stop.waiting_ms),
        );
    }
    if let Some(next) = &s.next_release {
        println!(
            "next release {} ({}, {})",
            next.version,
            w(&next.state),
            w(&next.channel)
        );
    }
    let deps = |n: NodeRef| -> String {
        d.edges
            .iter()
            .filter(|e| e.to == n)
            .map(|e| node_label(d, e.from))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let rows: Vec<Vec<String>> = d
        .order
        .iter()
        .filter_map(|&n| match n {
            NodeRef::Ticket(id) => d.tickets.iter().find(|t| t.id == id).map(|t| {
                vec![
                    t.key.to_string(),
                    w(&t.status),
                    t.title.clone(),
                    deps(n),
                    reason(t.status_reason.as_ref()),
                ]
            }),
            NodeRef::Release(id) => d.releases.iter().find(|r| r.id == id).map(|r| {
                vec![
                    format!("<> {}", r.version),
                    w(&r.state),
                    format!("release, {}, {}", w(&r.channel), w(&r.start_mode)),
                    deps(n),
                    String::new(),
                ]
            }),
        })
        .collect();
    table(&["NODE", "STATUS", "TITLE", "AFTER", "REASON"], &rows);
}

fn node_label(d: &ChainDetail, node: NodeRef) -> String {
    let label = match node {
        NodeRef::Ticket(id) => d
            .tickets
            .iter()
            .find(|t| t.id == id)
            .map(|t| t.key.to_string()),
        NodeRef::Release(id) => d
            .releases
            .iter()
            .find(|r| r.id == id)
            .map(|r| r.version.clone()),
    };
    label.unwrap_or_else(|| "?".to_owned())
}

/// `tok show` from `GET /v1/tickets/{ticket}`.
pub fn ticket(d: &TicketDetail) {
    let t = &d.ticket;
    println!(
        "{}  {}  [{}{}] for {}",
        t.key,
        t.title,
        status_word(&t.status, d.simulated),
        queued(d.queue_position),
        duration(d.in_status_ms)
    );
    crate::paused_output::reason(t, d.queue_position, &reason(t.status_reason.as_ref()));
    crate::allow_always::print_request(t.status_reason.as_ref());
    println!(
        "epic: {}  profile: {}",
        d.chain,
        t.profile.as_deref().unwrap_or("(project default)")
    );
    if let Some(m) = &t.model {
        println!("model: {m}");
    }
    if let Some(s) = &d.ship {
        println!("workspace: {}  ({})", s.path, s.branch);
    }
    if let Some(pr) = &t.pr {
        let checks: Vec<String> = pr
            .checks
            .iter()
            .map(|c| format!("{} {}", c.name, w(&c.state)))
            .collect();
        println!("pr: {}  checks: {}", pr.url, checks.join(", "));
    }
    crate::review_output::review(d);
    if let Some(wc) = &t.wildcard {
        let from = wc.from.as_ref().map(|k| format!(" during {k}"));
        let approved = if wc.approved { ", approved" } else { "" };
        println!(
            "wildcard: proposed by {}{}{approved}",
            w(&wc.proposed_by),
            from.unwrap_or_default()
        );
    }
    if let Some(o) = &t.done_out_of_order {
        let behind: Vec<String> = o.behind.iter().map(ToString::to_string).collect();
        println!("done out of order: {} was still open", behind.join(", "));
    }
    if !t.prompt.is_empty() {
        println!("\n{}", t.prompt);
    }
    if !t.acceptance.is_empty() {
        println!("\nacceptance:");
        for a in &t.acceptance {
            println!("  - {a}");
        }
    }
    if !d.subtasks.is_empty() {
        println!("\nsubtasks:");
        for s in &d.subtasks {
            let by = s
                .done_by
                .map(|b| format!(" (by {})", w(&b)))
                .unwrap_or_default();
            let mark = if s.done { "x" } else { " " };
            println!("  {}. [{mark}] {}{by}", s.position, s.text);
        }
    }
    if !d.comments.is_empty() {
        println!("\ncomments:");
        for c in &d.comments {
            let delivered = match (c.kind, c.author, c.delivered_at) {
                (CommentKind::Instruction, Actor::Human, None) => " (pending)",
                (CommentKind::Instruction, Actor::Human, Some(_)) => " (delivered)",
                _ => "",
            };
            println!(
                "  [{}] {} by {}{delivered}",
                w(&c.kind),
                c.body.gist,
                author_word(c.author)
            );
        }
    }
}

/// The clock time (`HH:MM:SS`) of a journal entry in the given UTC offset; `--:--:--` if the
/// timestamp is out of range.
pub fn clock(e: &EventRecord, offset: chrono::FixedOffset) -> String {
    chrono::DateTime::from_timestamp_millis(e.at.0)
        .map(|t| t.with_timezone(&offset).format("%H:%M:%S").to_string())
        .unwrap_or_else(|| "--:--:--".to_owned())
}

/// The UTC offset in force on this machine at the time of the entry.
pub fn local_offset(e: &EventRecord) -> chrono::FixedOffset {
    use chrono::{Local, Offset, TimeZone};
    chrono::DateTime::from_timestamp_millis(e.at.0)
        .map(|t| Local.from_utc_datetime(&t.naive_utc()).offset().fix())
        .unwrap_or_else(|| chrono::FixedOffset::east_opt(0).expect("zero offset is valid"))
}

/// One `tok watch` line for a journal entry, starting with its time in `offset`.
pub fn event(e: &EventRecord, offset: chrono::FixedOffset) -> String {
    let p = &e.payload;
    let s = |v: &Value| v.as_str().unwrap_or("").to_owned();
    let payload_reason = serde_json::from_value::<StatusReason>(p["reason"].clone()).ok();
    let what = match (e.entity.as_str(), e.kind.as_str()) {
        ("ticket", "status_changed") => format!(
            "{} {} -> {}{}",
            s(&p["key"]),
            s(&p["from"]),
            s(&p["to"]),
            in_parens(payload_reason.as_ref())
        ),
        ("ticket", kind) => format!("{} {kind}", s(&p["ticket"]["key"])),
        ("release", "state_changed") => format!(
            "release {} {} -> {}",
            s(&p["version"]),
            s(&p["from"]),
            s(&p["to"])
        ),
        ("comment", kind) => format!(
            "{} comment {kind}: [{}] {}",
            s(&p["ticket"]),
            s(&p["comment"]["kind"]),
            s(&p["comment"]["body"]["gist"])
        ),
        ("window", "show_card") => format!("window show_card {}", s(&p["ticket"])),
        ("window", "suggest") => format!("window suggest {}", s(&p["suggestion"]["text"])),
        ("window", "suggestion_cleared") => "window suggestion_cleared".to_owned(),
        (entity, kind) => format!("{entity} {kind}"),
    };
    format!("{} #{} {} {what}", clock(e, offset), e.seq, w(&e.actor))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use toknila_core::{BlockedReason, TicketStatus, Timestamp, WaitingReason};

    #[test]
    fn durations() {
        assert_eq!(duration(0), "0s");
        assert_eq!(duration(59_999), "59s");
        assert_eq!(duration(90_000), "1m30s");
        assert_eq!(duration(3_600_000 * 2 + 60_000), "2h01m");
        assert_eq!(duration(86_400_000 + 3_600_000), "1d01h");
    }

    #[test]
    fn the_daemons_own_comments_are_by_toknila() {
        assert_eq!(author_word(Actor::System), "Toknila");
        assert_eq!(author_word(Actor::Agent), "agent");
        assert_eq!(author_word(Actor::Human), "human");
    }

    #[test]
    fn reasons() {
        let waiting = StatusReason::Waiting(WaitingReason::Question {
            text: "?".to_owned(),
            choices: Vec::new(),
        });
        assert_eq!(reason(Some(&waiting)), "question: ?");
        let blocked = StatusReason::Blocked {
            reason: BlockedReason::Manual {
                text: "key".to_owned(),
            },
            resume_to: TicketStatus::Running,
            waiting: None,
        };
        assert_eq!(reason(Some(&blocked)), "manual: key");
        let no_dir = StatusReason::Blocked {
            reason: BlockedReason::NoWorkingDir,
            resume_to: TicketStatus::Running,
            waiting: None,
        };
        assert_eq!(
            reason(Some(&no_dir)),
            "no_working_dir: the project has no repository, so the ticket has no ship; give it one or use the sim profile"
        );
        assert_eq!(reason(None), "");
    }

    #[test]
    fn event_lines() {
        let e = EventRecord {
            seq: 4,
            at: Timestamp(0),
            actor: Actor::System,
            entity: "ticket".to_owned(),
            entity_id: String::new(),
            kind: "status_changed".to_owned(),
            payload: json!({"key": "DEMO-1", "from": "ready", "to": "running", "reason": null}),
        };
        let utc = chrono::FixedOffset::east_opt(0).expect("valid");
        assert_eq!(event(&e, utc), "00:00:00 #4 system DEMO-1 ready -> running");
        // 2024-01-02T03:04:05Z shown at UTC+02:00 is 05:04:05.
        let e = EventRecord {
            at: Timestamp(1_704_164_645_000),
            ..e
        };
        let plus2 = chrono::FixedOffset::east_opt(2 * 3600).expect("valid");
        assert_eq!(
            event(&e, plus2),
            "05:04:05 #4 system DEMO-1 ready -> running"
        );
        assert_eq!(event(&e, utc), "03:04:05 #4 system DEMO-1 ready -> running");
    }

    #[test]
    fn a_show_card_event_names_the_card() {
        let e = EventRecord {
            seq: 9,
            at: Timestamp(0),
            actor: Actor::Agent,
            entity: "window".to_owned(),
            entity_id: "01JBQ7ZJ0000000000000000P1".to_owned(),
            kind: "show_card".to_owned(),
            payload: json!({"project": "01JBQ7ZJ0000000000000000P1", "project_key": "DEMO",
                            "chain": "plan", "ticket": "DEMO-2"}),
        };
        let utc = chrono::FixedOffset::east_opt(0).expect("valid");
        assert_eq!(event(&e, utc), "00:00:00 #9 agent window show_card DEMO-2");
    }

    #[test]
    fn suggestion_events_say_the_step_and_its_end() {
        let at = |kind: &str, payload| EventRecord {
            seq: 3,
            at: Timestamp(0),
            actor: Actor::Agent,
            entity: "window".to_owned(),
            entity_id: "01JBQ7ZJ0000000000000000P1".to_owned(),
            kind: kind.to_owned(),
            payload,
        };
        let utc = chrono::FixedOffset::east_opt(0).expect("valid");
        let set = at("suggest", json!({"suggestion": {"text": "Start DEMO-2"}}));
        assert_eq!(
            event(&set, utc),
            "00:00:00 #3 agent window suggest Start DEMO-2"
        );
        let cleared = at("suggestion_cleared", json!({"project_key": "DEMO"}));
        assert_eq!(
            event(&cleared, utc),
            "00:00:00 #3 agent window suggestion_cleared"
        );
    }
}
