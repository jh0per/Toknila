//! Human output: tables and one-line summaries. `--json` bypasses all of this.

use serde::Serialize;
use serde_json::Value;
use toknila_api_types::{ChainDetail, EventRecord, TicketDetail, TicketItem};
use toknila_core::wire::wire_name;
use toknila_core::{Actor, CommentKind, NodeRef, StatusReason};

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
        w(&t.status),
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
            let mut status = w(&t.status);
            if t.paused {
                status.push_str(" (paused)");
            }
            status.push_str(&queued(it.queue_position));
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
    println!(
        "{}  {}  done {}/{}  released {}{}",
        d.chain.name,
        w(&s.status),
        s.done,
        s.total,
        s.released,
        if d.chain.autostart {
            ""
        } else {
            "  (autostart off)"
        }
    );
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
        w(&t.status),
        queued(d.queue_position),
        duration(d.in_status_ms)
    );
    let r = reason(t.status_reason.as_ref());
    if !r.is_empty() {
        println!("reason: {r}");
    }
    println!(
        "chain: {}  profile: {}",
        d.chain,
        t.profile.as_deref().unwrap_or("(project default)")
    );
    if let Some(s) = &d.ship {
        println!("ship: {}  ({})", s.path, s.branch);
    }
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
                w(&c.author)
            );
        }
    }
}

/// One `tok watch` line for a journal entry.
pub fn event(e: &EventRecord) -> String {
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
        (entity, kind) => format!("{entity} {kind}"),
    };
    format!("#{} {} {what}", e.seq, w(&e.actor))
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
    fn reasons() {
        let waiting = StatusReason::Waiting(WaitingReason::Question {
            text: "?".to_owned(),
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
        assert_eq!(event(&e), "#4 system DEMO-1 ready -> running");
    }
}
