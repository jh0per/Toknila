//! Human output: tables and one-line summaries. `--json` bypasses all of this.

use serde_json::Value;

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

/// Reason of a Waiting/Blocked/Cancelled status in a few words.
pub fn reason(r: &Value) -> String {
    if r.is_null() {
        return String::new();
    }
    let text = |v: &Value| {
        v["text"]
            .as_str()
            .or(v["detail"].as_str())
            .unwrap_or("")
            .to_owned()
    };
    match r["status"].as_str() {
        Some("waiting") => match r["kind"].as_str() {
            Some("silent") => "silent".to_owned(),
            Some(kind) => format!("{kind}: {}", text(r)),
            None => String::new(),
        },
        Some("blocked") => {
            let inner = &r["reason"];
            match inner["kind"].as_str() {
                Some("dependency_reopened") => format!(
                    "dependency {} reopened",
                    inner["dependency"].as_str().unwrap_or("?")
                ),
                Some("runner_lost") => "runner lost".to_owned(),
                Some(kind) => format!("{kind}: {}", text(inner)),
                None => String::new(),
            }
        }
        Some("cancelled") => format!("cancelled: {}", text(r)),
        _ => String::new(),
    }
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
    let line = |cells: Vec<String>| {
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
    line(header.iter().map(|h| (*h).to_owned()).collect());
    for row in rows {
        line(row.clone());
    }
}

/// `tok ls` rows from `GET /v1/tickets`.
pub fn tickets(items: &Value) {
    let rows: Vec<Vec<String>> = items
        .as_array()
        .into_iter()
        .flatten()
        .map(|it| {
            let t = &it["ticket"];
            let mut status = t["status"].as_str().unwrap_or("").to_owned();
            if t["paused"].as_bool() == Some(true) {
                status.push_str(" (paused)");
            }
            vec![
                t["key"].as_str().unwrap_or("").to_owned(),
                status,
                duration(it["in_status_ms"].as_u64().unwrap_or(0)),
                it["chain"].as_str().unwrap_or("").to_owned(),
                t["title"].as_str().unwrap_or("").to_owned(),
                reason(&t["status_reason"]),
            ]
        })
        .collect();
    table(&["KEY", "STATUS", "FOR", "CHAIN", "TITLE", "REASON"], &rows);
}

/// `tok chain show` from `GET /v1/chains/{chain}`.
pub fn chain(d: &Value) {
    let c = &d["chain"];
    let s = &d["summary"];
    println!(
        "{}  {}  done {}/{}  released {}{}",
        c["name"].as_str().unwrap_or(""),
        s["status"].as_str().unwrap_or(""),
        s["done"],
        s["total"],
        s["released"],
        if c["autostart"].as_bool() == Some(false) {
            "  (autostart off)"
        } else {
            ""
        }
    );
    let stop = &s["stopped_at"];
    if !stop.is_null() {
        println!(
            "stopped at {} ({} for {}{})",
            stop["ticket"].as_str().unwrap_or(""),
            stop["status"].as_str().unwrap_or(""),
            duration(stop["waiting_ms"].as_u64().unwrap_or(0)),
            match reason(&stop["reason"]) {
                r if r.is_empty() => String::new(),
                r => format!(": {r}"),
            }
        );
    }
    let next = &s["next_release"];
    if !next.is_null() {
        println!(
            "next release {} ({}, {})",
            next["version"].as_str().unwrap_or(""),
            next["state"].as_str().unwrap_or(""),
            next["channel"].as_str().unwrap_or("")
        );
    }
    let deps = |id: &Value| -> String {
        d["edges"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|e| e["to"]["id"] == *id)
            .map(|e| node_label(d, &e["from"]))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let mut rows = Vec::new();
    for n in d["order"].as_array().into_iter().flatten() {
        let id = &n["id"];
        match n["kind"].as_str() {
            Some("ticket") => {
                if let Some(t) = d["tickets"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .find(|t| t["id"] == *id)
                {
                    rows.push(vec![
                        t["key"].as_str().unwrap_or("").to_owned(),
                        t["status"].as_str().unwrap_or("").to_owned(),
                        t["title"].as_str().unwrap_or("").to_owned(),
                        deps(id),
                        reason(&t["status_reason"]),
                    ]);
                }
            }
            Some("release") => {
                if let Some(r) = d["releases"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .find(|r| r["id"] == *id)
                {
                    rows.push(vec![
                        format!("<> {}", r["version"].as_str().unwrap_or("")),
                        r["state"].as_str().unwrap_or("").to_owned(),
                        format!(
                            "release, {}, {}",
                            r["channel"].as_str().unwrap_or(""),
                            r["start_mode"].as_str().unwrap_or("")
                        ),
                        deps(id),
                        String::new(),
                    ]);
                }
            }
            _ => {}
        }
    }
    table(&["NODE", "STATUS", "TITLE", "AFTER", "REASON"], &rows);
}

fn node_label(d: &Value, node: &Value) -> String {
    let id = &node["id"];
    match node["kind"].as_str() {
        Some("ticket") => d["tickets"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|t| t["id"] == *id)
            .and_then(|t| t["key"].as_str())
            .unwrap_or("?")
            .to_owned(),
        _ => d["releases"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|r| r["id"] == *id)
            .and_then(|r| r["version"].as_str())
            .unwrap_or("?")
            .to_owned(),
    }
}

/// `tok show` from `GET /v1/tickets/{ticket}`.
pub fn ticket(d: &Value) {
    let t = &d["ticket"];
    println!(
        "{}  {}  [{}] for {}",
        t["key"].as_str().unwrap_or(""),
        t["title"].as_str().unwrap_or(""),
        t["status"].as_str().unwrap_or(""),
        duration(d["in_status_ms"].as_u64().unwrap_or(0))
    );
    let r = reason(&t["status_reason"]);
    if !r.is_empty() {
        println!("reason: {r}");
    }
    println!(
        "chain: {}  profile: {}",
        d["chain"].as_str().unwrap_or(""),
        t["profile"].as_str().unwrap_or("(project default)")
    );
    if let Some(p) = t["prompt"].as_str().filter(|p| !p.is_empty()) {
        println!("\n{p}");
    }
    let acc: Vec<&str> = t["acceptance"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    if !acc.is_empty() {
        println!("\nacceptance:");
        for a in acc {
            println!("  - {a}");
        }
    }
    let subs = d["subtasks"].as_array().cloned().unwrap_or_default();
    if !subs.is_empty() {
        println!("\nsubtasks:");
        for s in subs {
            let by = s["done_by"]
                .as_str()
                .map(|b| format!(" (by {b})"))
                .unwrap_or_default();
            println!(
                "  {}. [{}] {}{}",
                s["position"],
                if s["done"].as_bool() == Some(true) {
                    "x"
                } else {
                    " "
                },
                s["text"].as_str().unwrap_or(""),
                by
            );
        }
    }
    let comments = d["comments"].as_array().cloned().unwrap_or_default();
    if !comments.is_empty() {
        println!("\ncomments:");
        for c in comments {
            let delivered = if c["kind"] == "instruction" && c["author"] == "human" {
                if c["delivered_at"].is_null() {
                    " (pending)"
                } else {
                    " (delivered)"
                }
            } else {
                ""
            };
            println!(
                "  [{}] {} by {}{}",
                c["kind"].as_str().unwrap_or(""),
                c["body"]["gist"].as_str().unwrap_or(""),
                c["author"].as_str().unwrap_or(""),
                delivered
            );
        }
    }
}

/// One `tok watch` line for a journal event.
pub fn event(e: &Value) -> String {
    let p = &e["payload"];
    let seq = e["seq"].as_i64().unwrap_or(0);
    let what = match (e["entity"].as_str(), e["kind"].as_str()) {
        (Some("ticket"), Some("status_changed")) => {
            let r = reason(&p["reason"]);
            format!(
                "{} {} -> {}{}",
                p["key"].as_str().unwrap_or(""),
                p["from"].as_str().unwrap_or(""),
                p["to"].as_str().unwrap_or(""),
                if r.is_empty() {
                    String::new()
                } else {
                    format!(" ({r})")
                }
            )
        }
        (Some("ticket"), Some(kind)) => {
            format!("{} {kind}", p["ticket"]["key"].as_str().unwrap_or(""))
        }
        (Some("release"), Some("state_changed")) => format!(
            "release {} {} -> {}",
            p["version"].as_str().unwrap_or(""),
            p["from"].as_str().unwrap_or(""),
            p["to"].as_str().unwrap_or("")
        ),
        (Some("comment"), Some(kind)) => format!(
            "{} comment {kind}: [{}] {}",
            p["ticket"].as_str().unwrap_or(""),
            p["comment"]["kind"].as_str().unwrap_or(""),
            p["comment"]["body"]["gist"].as_str().unwrap_or("")
        ),
        (Some(entity), Some(kind)) => format!("{entity} {kind}"),
        _ => "?".to_owned(),
    };
    format!("#{seq} {} {what}", e["actor"].as_str().unwrap_or(""))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

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
        assert_eq!(
            reason(&json!({"status":"waiting","kind":"question","text":"?"})),
            "question: ?"
        );
        assert_eq!(
            reason(
                &json!({"status":"blocked","reason":{"kind":"manual","text":"key"},"resume_to":"running"})
            ),
            "manual: key"
        );
        assert_eq!(reason(&Value::Null), "");
    }
}
