//! Token usage through the full router: a hook ties a transcript to a run, the card shows the
//! numbers, and the summary and breakdowns follow. Synthetic transcripts in the test's folder.

use std::io::Write;
use std::path::PathBuf;

use axum::http::{Method, StatusCode};
use serde_json::{Value, json};
use toknila_core::AgentRole;
use toknila_core::usage::{day_label, day_of};

use super::Api;
use crate::server::now;

const OPUS: &str = "claude-opus-4-8";

/// One assistant line of a Claude Code transcript, today, with invented numbers.
fn call(id: &str, model: &str, tokens: [u64; 4]) -> String {
    let day = day_label(day_of(now()));
    format!(
        r#"{{"type":"assistant","uuid":"u-{id}","requestId":"r-{id}","timestamp":"{day}T12:00:00.000Z","cwd":"/work/demo","message":{{"id":"{id}","model":"{model}","usage":{{"input_tokens":{},"cache_creation_input_tokens":{},"cache_read_input_tokens":{},"output_tokens":{}}}}}}}"#,
        tokens[0], tokens[1], tokens[2], tokens[3]
    ) + "\n"
}

impl Api {
    fn transcript(&self, project: &str, session: &str) -> PathBuf {
        let dir = self._dir.path().join("claude/projects").join(project);
        std::fs::create_dir_all(&dir).expect("project folder");
        dir.join(format!("{session}.jsonl"))
    }

    /// A hook of the run `token` fired in `session`, whose transcript is `path`.
    async fn hook(&self, token: &str, signal: &str, session: &str, path: &std::path::Path) {
        let (status, body) = self
            .call_with(
                token,
                Method::POST,
                "/v1/tickets/DEMO-1/activity",
                Some(json!({ "signal": signal, "session_id": session, "transcript_path": path })),
            )
            .await;
        assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    }
}

fn append(path: &std::path::Path, text: &str) {
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .expect("open transcript");
    f.write_all(text.as_bytes()).expect("write transcript");
}

#[tokio::test]
async fn a_hook_ties_the_transcript_to_the_run_and_the_card_shows_its_usage() {
    let api = Api::new();
    api.demo_with(&["A"]).await;
    let (worker, _) = api.agent_token("DEMO-1").await;
    let (reviewer, _) = api.agent_token_as("DEMO-1", AgentRole::Reviewer).await;

    let before = api.get("/v1/tickets/DEMO-1").await;
    assert_eq!(before["usage"]["messages"], 0);

    let path = api.transcript("-work-demo", "sess-w");
    // A call is written twice (streaming); it counts once.
    append(
        &path,
        &(call("m1", OPUS, [1_000_000, 0, 0, 10])
            + &call("m1", OPUS, [1_000_000, 0, 0, 100_000])
            + &call("m2", OPUS, [0, 0, 2_000_000, 0])),
    );
    api.hook(&worker, "hook:stop", "sess-w", &path).await;

    let ticket = api.get("/v1/tickets/DEMO-1").await;
    let usage = &ticket["usage"];
    assert_eq!(usage["messages"], 2, "{usage}");
    assert_eq!(usage["tokens"]["input"], 1_000_000);
    assert_eq!(usage["tokens"]["output"], 100_000);
    assert_eq!(usage["tokens"]["cache_read"], 2_000_000);
    assert_eq!(usage["processed"], 3_100_000);
    // 1M input at $5, 100k output at $25 per million, 2M cached at $0.50: 5 + 2.5 + 1.
    assert!(
        (usage["cost_usd"].as_f64().expect("cost") - 8.5).abs() < 1e-6,
        "{usage}"
    );
    // The cache saved 2M x (5 - 0.5) / 1M.
    assert!((usage["cache_savings_usd"].as_f64().expect("savings") - 9.0).abs() < 1e-6);

    // The next hook reads only what was appended.
    append(&path, &call("m3", OPUS, [0, 0, 0, 1_000_000]));
    api.hook(&worker, "hook:stop", "sess-w", &path).await;
    let ticket = api.get("/v1/tickets/DEMO-1").await;
    assert_eq!(ticket["usage"]["messages"], 3);
    assert_eq!(ticket["usage"]["tokens"]["output"], 1_100_000);

    // The reviewer's session counts for the same card.
    let rpath = api.transcript("-work-demo", "sess-r");
    append(&rpath, &call("m9", OPUS, [100, 0, 0, 100]));
    api.hook(&reviewer, "hook:stop", "sess-r", &rpath).await;
    let ticket = api.get("/v1/tickets/DEMO-1").await;
    assert_eq!(ticket["usage"]["messages"], 4);

    // The chain's cards carry their usage for the chip.
    let chain = api.get("/v1/chains/demo").await;
    assert_eq!(chain["usage"]["DEMO-1"]["messages"], 4);
}

#[tokio::test]
async fn a_transcript_outside_the_agents_folder_is_never_read() {
    let api = Api::new();
    api.demo_with(&["A"]).await;
    let (worker, _) = api.agent_token("DEMO-1").await;
    let outside = api._dir.path().join("elsewhere.jsonl");
    append(&outside, &call("m1", OPUS, [1, 1, 1, 1]));
    api.hook(&worker, "hook:stop", "s", &outside).await;
    let dotted = api
        .transcript("-work-demo", "s")
        .join("../../../../elsewhere.jsonl");
    api.hook(&worker, "hook:stop", "s2", &dotted).await;
    assert_eq!(api.get("/v1/tickets/DEMO-1").await["usage"]["messages"], 0);
    let view = api.get("/v1/usage?days=7").await;
    assert_eq!(view["summary"]["totals"]["messages"], 0);
}

#[tokio::test]
async fn the_summary_totals_the_window_and_breaks_it_down() {
    let api = Api::new();
    api.demo_with(&["A", "B"]).await;
    let (worker, _) = api.agent_token("DEMO-1").await;
    let path = api.transcript("-work-demo", "sess-w");
    append(
        &path,
        &(call("m1", OPUS, [1_000_000, 0, 0, 0])
            + &call("m2", "claude-haiku-4-5", [0, 0, 0, 1_000_000])),
    );
    api.hook(&worker, "hook:stop", "sess-w", &path).await;
    // A session of the same machine that is not a Toknila run, in another folder.
    let other = api.transcript("-work-other", "sess-x");
    append(
        &other,
        &call("m5", OPUS, [0, 0, 1_000_000, 0]).replace("/work/demo", "/work/other"),
    );

    let view = api.get("/v1/usage?days=30").await;
    assert_eq!(view["machine_scan"], true);
    let s = &view["summary"];
    assert_eq!(s["days"], 30);
    assert_eq!(s["series"].as_array().expect("series").len(), 30);
    assert_eq!(s["totals"]["messages"], 3);
    assert_eq!(s["totals"]["processed"], 3_000_000);
    // 5 (opus input) + 5 (haiku output) + 0.5 (opus cached read)
    assert!(
        (s["totals"]["cost_usd"].as_f64().expect("cost") - 10.5).abs() < 1e-6,
        "{s}"
    );
    let labels = |key: &str| -> Vec<String> {
        s[key]
            .as_array()
            .expect(key)
            .iter()
            .map(|i| i["label"].as_str().expect("label").to_owned())
            .collect()
    };
    assert_eq!(labels("by_model"), [OPUS, "claude-haiku-4-5"]);
    assert_eq!(labels("by_card"), ["DEMO-1"]);
    assert_eq!(labels("by_chain"), ["demo"]);
    assert_eq!(labels("by_project"), ["DEMO"]);
    assert_eq!(labels("by_workspace"), ["/work/demo", "/work/other"]);
    assert!(
        view["prices"]["source"]
            .as_str()
            .expect("source")
            .starts_with("https://")
    );

    // Switched off, only Toknila's runs are counted.
    let (status, set) = api
        .call(
            Method::PUT,
            "/v1/usage/settings",
            Some(json!({ "machine_scan": false })),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{set}");
    let view = api.get("/v1/usage?days=30").await;
    assert_eq!(view["machine_scan"], false);
    assert_eq!(view["summary"]["totals"]["messages"], 2);
    assert_eq!(
        view["summary"]["by_workspace"]
            .as_array()
            .expect("ws")
            .len(),
        1
    );
}

#[tokio::test]
async fn the_window_is_7_30_or_90_days() {
    let api = Api::new();
    assert_eq!(api.get("/v1/usage").await["summary"]["days"], 7);
    for days in [7, 30, 90] {
        let view = api.get(&format!("/v1/usage?days={days}")).await;
        assert_eq!(view["summary"]["days"], days);
    }
    let (status, body) = api.call(Method::GET, "/v1/usage?days=5", None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
}

#[tokio::test]
async fn a_person_can_override_a_price_in_the_data_home() {
    let api = Api::new();
    api.demo_with(&["A"]).await;
    std::fs::write(
        api._dir.path().join("usage-prices.toml"),
        format!("[[model]]\nid = \"{OPUS}\"\ninput = 100\ncache_write = 100\ncache_read = 100\noutput = 100\n"),
    )
    .expect("write prices");
    let (worker, _) = api.agent_token("DEMO-1").await;
    let path = api.transcript("-work-demo", "sess-w");
    append(&path, &call("m1", OPUS, [1_000_000, 0, 0, 0]));
    api.hook(&worker, "hook:stop", "sess-w", &path).await;
    let cost = api.get("/v1/tickets/DEMO-1").await["usage"]["cost_usd"]
        .as_f64()
        .expect("cost");
    assert!((cost - 100.0).abs() < 1e-6, "{cost}");
    let view = api.get("/v1/usage").await;
    assert_eq!(view["prices"]["overridden"], json!([OPUS]));

    std::fs::write(
        api._dir.path().join("usage-prices.toml"),
        "[[model]]\nid = 3",
    )
    .expect("write");
    let view = api.get("/v1/usage").await;
    assert!(view["prices"]["problem"].is_string(), "{view}");
    assert!(
        view["summary"]["totals"]["cost_usd"]
            .as_f64()
            .expect("cost")
            > 0.0
    );
}

#[tokio::test]
async fn the_quota_is_whatever_a_status_line_reported_until_the_window_resets() {
    let api = Api::new();
    api.demo_with(&["A"]).await;
    let none = api.get("/v1/usage").await;
    assert_eq!(none["quota"]["available"], false);
    assert!(!none["quota"]["reason"].as_str().expect("reason").is_empty());

    let (worker, _) = api.agent_token("DEMO-1").await;
    let (status, body) = api
        .call_with(
            &worker,
            Method::POST,
            "/v1/usage/quota",
            Some(json!({ "windows": [
                { "name": "five_hour", "used_percentage": 23.5, "resets_at": 4_102_444_800_i64 },
                { "name": "seven_day", "used_percentage": 41.0, "resets_at": 1 },
            ] })),
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    let quota: Value = api.get("/v1/usage").await["quota"].clone();
    assert_eq!(quota["available"], true);
    let windows = quota["windows"].as_array().expect("windows");
    assert_eq!(
        windows.len(),
        1,
        "the window that already reset is dropped: {quota}"
    );
    assert_eq!(windows[0]["name"], "five_hour");
    assert_eq!(windows[0]["used_percentage"], 23.5);
}
