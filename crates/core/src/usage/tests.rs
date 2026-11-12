//! Synthetic transcripts only: the shape of Claude Code's JSONL with invented ids, numbers and
//! no message content.

use super::*;
use crate::Timestamp;

const T0: &str = "2026-11-05T10:00:00.000Z";
const T1: &str = "2026-11-06T10:00:00.000Z";
const NOW: &str = "2026-11-06T12:00:00.000Z";

/// One assistant line: message id, request id, model, [input, cache write, cache read, output].
fn line(id: &str, req: &str, model: &str, t: [u64; 4], at: &str) -> String {
    format!(
        r#"{{"type":"assistant","uuid":"u-{id}","requestId":"{req}","timestamp":"{at}","cwd":"/work/demo","message":{{"id":"{id}","model":"{model}","role":"assistant","content":[{{"type":"text","text":"x"}}],"usage":{{"input_tokens":{},"cache_creation_input_tokens":{},"cache_read_input_tokens":{},"output_tokens":{},"service_tier":"standard"}}}}}}"#,
        t[0], t[1], t[2], t[3]
    ) + "\n"
}

fn ts(s: &str) -> Timestamp {
    parse_rfc3339(s).expect("time")
}

fn parse(text: &str) -> Chunk {
    ClaudeCode.parse(text.as_bytes())
}

fn counts(input: u64, cache_write: u64, cache_read: u64, output: u64) -> TokenCounts {
    TokenCounts {
        input,
        cache_write,
        cache_read,
        output,
    }
}

fn row(model: &str, at: &str, tokens: TokenCounts) -> Row {
    Row {
        at: ts(at),
        model: model.to_owned(),
        tokens,
        card: None,
        chain: None,
        project: None,
        workspace: "/work/demo".to_owned(),
    }
}

fn rows(chunk: &Chunk) -> Vec<Row> {
    chunk
        .records
        .iter()
        .map(|r| row(&r.model, T0, r.tokens))
        .collect()
}

fn prices() -> PriceTable {
    PriceTable::parse(
        r#"
source = "test"
retrieved = "2026-01-01"
[[model]]
id = "m-big"
input = 10
cache_write = 12.5
cache_read = 0.25
output = 50
[[model]]
id = "m-small"
input = 1
cache_write = 1.25
cache_read = 0.1
output = 5
"#,
    )
    .expect("prices")
}

#[test]
fn the_four_counts_are_summed_per_model() {
    let text = line("m1", "r1", "m-big", [10, 20, 300, 4], T0)
        + &line("m2", "r2", "m-big", [1, 2, 3, 4], T0)
        + &line("m3", "r3", "m-small", [5, 0, 0, 7], T0);
    let chunk = parse(&text);
    assert_eq!(chunk.records.len(), 3);
    let s = summarize(&rows(&chunk), &prices(), ts(NOW), 7);
    let big = s.by_model.iter().find(|i| i.label == "m-big").expect("big");
    assert_eq!(big.figures.tokens, counts(11, 22, 303, 8));
    assert_eq!(big.figures.messages, 2);
    let small = s
        .by_model
        .iter()
        .find(|i| i.label == "m-small")
        .expect("small");
    assert_eq!(small.figures.tokens, counts(5, 0, 0, 7));
    assert_eq!(s.totals.tokens, counts(16, 22, 303, 15));
    assert_eq!(s.totals.processed, 356);
}

#[test]
fn a_call_written_many_times_counts_once_with_its_largest_numbers() {
    // Streaming writes one line per content block with the same ids; the output grows.
    let text = line("m1", "r1", "m-big", [10, 0, 100, 5], T0)
        + &line("m1", "r1", "m-big", [10, 0, 100, 40], T0)
        + &line("m1", "r1", "m-big", [10, 0, 100, 40], T0)
        + &line("m1", "r2", "m-big", [10, 0, 100, 9], T0)
        + r#"{"type":"user","message":{"role":"user","content":"hello"}}"#;
    let chunk = parse(&text);
    assert_eq!(
        chunk.records.len(),
        2,
        "same message id, other request, is another call"
    );
    let first = chunk
        .records
        .iter()
        .find(|r| r.key.ends_with("r1"))
        .expect("r1");
    assert_eq!(first.tokens, counts(10, 0, 100, 40));
}

#[test]
fn synthetic_models_errors_and_garbage_are_not_usage() {
    let synthetic = line("s", "s", "<synthetic>", [1, 1, 1, 1], T0);
    let error = line("e", "e", "m-big", [1, 1, 1, 1], T0).replace(
        r#""type":"assistant""#,
        r#""type":"assistant","isApiErrorMessage":true"#,
    );
    let text = synthetic + &error + "not json at all\n{\"type\":\"assistant\"}\n";
    let chunk = parse(&text);
    assert_eq!(chunk.records, vec![]);
    assert!(
        chunk.consumed > 0,
        "bad lines are consumed, not retried forever"
    );
}

#[test]
fn a_read_continues_from_the_offset_and_waits_for_a_half_written_line() {
    let a = line("a", "r", "m-big", [1, 0, 0, 1], T0);
    let b = line("b", "r", "m-big", [2, 0, 0, 2], T0);
    let c = line("c", "r", "m-big", [3, 0, 0, 3], T0);
    let (c_head, c_tail) = c.split_at(c.len() / 2);

    let first_read = format!("{a}{b}{c_head}");
    let one = parse(&first_read);
    assert_eq!(one.records.len(), 2);
    assert_eq!(
        one.consumed,
        a.len() + b.len(),
        "the unfinished line stays unread"
    );

    let second_read = format!(
        "{c_head}{c_tail}{}",
        line("d", "r", "m-big", [4, 0, 0, 4], T0)
    );
    let two = parse(&second_read);
    let keys: Vec<&str> = two.records.iter().map(|r| r.key.as_str()).collect();
    assert_eq!(keys, vec!["c/r", "d/r"]);
    assert_eq!(parse(c_head).consumed, 0);
}

#[test]
fn the_workspace_is_the_first_cwd_seen() {
    assert_eq!(
        parse(&line("a", "r", "m-big", [1, 0, 0, 1], T0))
            .cwd
            .as_deref(),
        Some("/work/demo")
    );
    assert_eq!(parse("").cwd, None);
}

#[test]
fn cost_and_cache_savings_follow_the_price_table() {
    let p = prices();
    let big = p.lookup("m-big").expect("price");
    let million = counts(1_000_000, 1_000_000, 1_000_000, 1_000_000);
    // 10 + 12.5 + 0.25 + 50
    assert!((big.cost(&million) - 72.75).abs() < 1e-9);
    // The cached million would have cost 10 uncached and cost 0.25.
    assert!((big.cache_savings(&million) - 9.75).abs() < 1e-9);

    let s = summarize(&[row("m-big", T1, million)], &p, ts(NOW), 7);
    assert!((s.totals.cost_usd - 72.75).abs() < 1e-9);
    assert!((s.totals.cache_savings_usd - 9.75).abs() < 1e-9);
    // 1M of 3M input tokens came from the cache.
    assert!((s.totals.cached_share - 1.0 / 3.0).abs() < 1e-9);
    assert_eq!(s.totals.uncached_input, 2_000_000);
}

#[test]
fn a_call_over_the_long_prompt_threshold_is_priced_by_the_long_tier() {
    let p = PriceTable::defaults();
    let h = p.lookup("claude-haiku-5-5").expect("haiku 5.5");
    // At the threshold (prompt counts cache reads and writes): base row.
    let at = counts(50_000, 20_000, 30_000, 1_000_000);
    // 5000*0.1 + 20000*0.125 + 30000*0.01 + 1M*0.5 per 1M... in USD:
    let base = (50_000.0 * 0.1 + 20_000.0 * 0.125 + 30_000.0 * 0.01 + 1_000_000.0 * 0.5) / 1e6;
    assert!((h.cost(&at) - base).abs() < 1e-9);
    // One token over: the whole call, output included, at the long row.
    let over = counts(50_001, 20_000, 30_000, 1_000_000);
    let long = (50_001.0 * 0.5 + 20_000.0 * 0.625 + 30_000.0 * 0.05 + 1_000_000.0 * 2.5) / 1e6;
    assert!((h.cost(&over) - long).abs() < 1e-9);
    assert!((h.cache_savings(&over) - 30_000.0 * (0.5 - 0.05) / 1e6).abs() < 1e-9);
    // Output alone does not count toward the prompt.
    assert!((h.cost(&counts(0, 0, 0, 200_000)) - 0.1).abs() < 1e-9);

    // Priced per call: two calls of 60k are base each, not 120k together.
    let call = counts(60_000, 0, 0, 0);
    let s = summarize(
        &[
            row("claude-haiku-5-5", T1, call),
            row("claude-haiku-5-5", T1, call),
        ],
        &p,
        ts(NOW),
        7,
    );
    assert!((s.totals.cost_usd - 2.0 * 60_000.0 * 0.1 / 1e6).abs() < 1e-12);
    // A model without a tier ignores size.
    let o = p.lookup("claude-haiku-4-5").expect("haiku 4.5");
    assert!((o.cost(&counts(500_000, 0, 0, 0)) - 0.5).abs() < 1e-9);
}

#[test]
fn prices_match_by_id_with_a_date_suffix_and_overrides_win() {
    let p = prices();
    assert!(p.lookup("m-big-20251001").is_some());
    assert!(p.lookup("m-big[1m]").is_some());
    assert!(p.lookup("m-bigger").is_none());
    assert!(p.lookup("<synthetic>").is_none());
    let over = PriceTable::parse(
        "[[model]]\nid = \"m-big\"\ninput = 1\ncache_write = 1\ncache_read = 1\noutput = 1\n",
    )
    .expect("override");
    let merged = p.with_overrides(over);
    assert_eq!(merged.lookup("m-big").expect("big").input, 1.0);
    assert_eq!(merged.lookup("m-small").expect("small").input, 1.0);
    assert_eq!(merged.source, "test", "the shipped source stays");
}

#[test]
fn the_shipped_prices_parse_and_name_their_source() {
    let p = PriceTable::defaults();
    assert!(p.source.starts_with("https://platform.claude.com/"));
    assert_eq!(p.retrieved.len(), 10);
    assert!(p.lookup("claude-opus-4-8").is_some());
    assert!(p.lookup("claude-haiku-4-5-20251001").is_some());
}

#[test]
fn a_bad_price_file_is_a_typed_error() {
    assert!(matches!(
        PriceTable::parse("[[model]]\nid = 3"),
        Err(PriceError::Invalid(_))
    ));
    let negative =
        "[[model]]\nid = \"x\"\ninput = -1\ncache_write = 0\ncache_read = 0\noutput = 0\n";
    assert!(matches!(
        PriceTable::parse(negative),
        Err(PriceError::Negative(_))
    ));
}

#[test]
fn the_series_has_one_entry_per_day_and_drops_rows_outside_the_window() {
    let old = row("m-big", "2026-10-01T10:00:00Z", counts(1, 0, 0, 1));
    let a = row("m-big", T0, counts(10, 0, 0, 10));
    let b = row("m-small", T1, counts(5, 0, 0, 5));
    let s = summarize(&[old, a, b], &prices(), ts(NOW), 7);
    assert_eq!(s.series.len(), 7);
    assert_eq!(s.series[0].day, "2026-10-31");
    assert_eq!(s.series[6].day, "2026-11-06");
    assert_eq!(s.series[5].figures.processed, 20);
    assert_eq!(s.series[6].figures.processed, 10);
    assert_eq!(s.series[0].figures.processed, 0);
    assert_eq!(
        s.totals.processed, 30,
        "the October row is outside seven days"
    );
}

#[test]
fn breakdowns_group_by_project_chain_card_and_workspace_and_sort_by_cost() {
    let mut a = row("m-big", T0, counts(1_000_000, 0, 0, 0));
    a.card = Some("DEMO-1".into());
    a.chain = Some("alpha".into());
    a.project = Some("DEMO".into());
    let mut b = row("m-small", T0, counts(1_000_000, 0, 0, 0));
    b.card = Some("DEMO-2".into());
    b.chain = Some("alpha".into());
    b.project = Some("DEMO".into());
    b.workspace = "/work/other".into();
    let c = row("m-big", T0, counts(1_000_000, 0, 0, 0));
    let s = summarize(&[a, b, c], &prices(), ts(NOW), 30);
    let labels = |v: &[UsageItem]| v.iter().map(|i| i.label.clone()).collect::<Vec<_>>();
    assert_eq!(labels(&s.by_card), vec!["DEMO-1", "DEMO-2"]);
    assert_eq!(labels(&s.by_chain), vec!["alpha"]);
    assert_eq!(s.by_chain[0].figures.messages, 2);
    assert_eq!(labels(&s.by_project), vec!["DEMO"]);
    assert_eq!(labels(&s.by_workspace), vec!["/work/demo", "/work/other"]);
    assert_eq!(s.by_workspace[0].figures.messages, 2);
    assert_eq!(labels(&s.by_model), vec!["m-big", "m-small"]);
}

#[test]
fn figures_per_card_sum_each_cards_calls_and_skip_calls_of_no_card() {
    let mut a = row("m-big", T0, counts(1_000_000, 0, 0, 0));
    a.card = Some("DEMO-1".into());
    let mut b = row("m-big", T1, counts(0, 0, 0, 1_000_000));
    b.card = Some("DEMO-1".into());
    let loose = row("m-big", T0, counts(5, 5, 5, 5));
    let per = figures_per_card(&[a, b, loose], &prices());
    assert_eq!(per.len(), 1);
    let one = &per["DEMO-1"];
    assert_eq!(one.messages, 2);
    assert!(
        (one.cost_usd - 60.0).abs() < 1e-9,
        "10 for input + 50 for output"
    );
}

#[test]
fn a_model_without_a_price_is_counted_but_not_costed_and_is_named() {
    let s = summarize(
        &[row("mystery", T0, counts(100, 0, 0, 100))],
        &prices(),
        ts(NOW),
        7,
    );
    assert_eq!(s.totals.processed, 200);
    assert_eq!(s.totals.cost_usd, 0.0);
    assert_eq!(s.unpriced_models, vec!["mystery".to_owned()]);
}

#[test]
fn only_a_transcript_under_an_agent_folder_is_accepted() {
    let roots = vec!["/home/me/.claude/projects".to_owned()];
    assert!(accepted_transcript(
        "/home/me/.claude/projects/-work-demo/abc.jsonl",
        &roots
    ));
    assert!(accepted_transcript(
        r"C:\Users\me\.claude\projects\x\abc.jsonl",
        &[r"C:\Users\me\.claude\projects".to_owned()]
    ));
    assert!(!accepted_transcript("/etc/passwd", &roots));
    assert!(!accepted_transcript(
        "/home/me/.claude/projects/x/abc.txt",
        &roots
    ));
    assert!(!accepted_transcript(
        "/home/me/.claude/projects/../../secret.jsonl",
        &roots
    ));
    assert!(!accepted_transcript(
        "/home/me/.claude/projectsX/abc.jsonl",
        &roots
    ));
    assert!(!accepted_transcript("relative/abc.jsonl", &roots));
}

#[test]
fn a_transcript_file_belongs_to_the_session_named_by_its_folder_or_file() {
    let roots = vec!["/h/.claude/projects".to_owned()];
    let of = |p: &str| session_of(p, &roots);
    assert_eq!(
        of("/h/.claude/projects/-w-demo/abc.jsonl").as_deref(),
        Some("abc")
    );
    // Sub-agents and workflows write under a folder named like the session.
    assert_eq!(
        of("/h/.claude/projects/-w-demo/abc/subagents/agent-1.jsonl").as_deref(),
        Some("abc")
    );
    assert_eq!(of("/elsewhere/-w-demo/abc.jsonl"), None);
    assert_eq!(of("/h/.claude/projects/abc.jsonl"), None);
}

mod quota {
    use super::*;
    use crate::usage::quota::{self, QuotaSnapshot, QuotaWindow};

    fn window(name: &str, pct: f64, resets: i64) -> QuotaWindow {
        QuotaWindow {
            name: name.to_owned(),
            used_percentage: pct,
            resets_at: resets,
        }
    }

    #[test]
    fn the_status_line_json_yields_the_documented_windows() {
        let json = r#"{"model":{"display_name":"x"},"rate_limits":{"five_hour":{"used_percentage":23.5,"resets_at":1738425600},"seven_day":{"used_percentage":41.2,"resets_at":1738857600},"spend_limit":{"used_percentage":1,"resets_at":1}}}"#;
        let windows = quota::parse_status_line(json);
        assert_eq!(
            windows,
            vec![
                window("five_hour", 23.5, 1_738_425_600),
                window("seven_day", 41.2, 1_738_857_600)
            ]
        );
        assert_eq!(quota::parse_status_line(r#"{"model":{}}"#), vec![]);
        assert_eq!(quota::parse_status_line("nope"), vec![]);
    }

    #[test]
    fn a_window_that_has_reset_is_dropped_and_nothing_seen_is_not_available() {
        let none = quota::view(None, Timestamp(0));
        assert!(!none.available);
        assert!(!none.reason.is_empty());
        let snap = QuotaSnapshot {
            seen_at: Timestamp(1_000),
            windows: vec![
                window("five_hour", 50.0, 100),
                window("seven_day", 10.0, 10_000),
            ],
        };
        let v = quota::view(Some(&snap), Timestamp(200_000));
        assert!(v.available);
        assert_eq!(v.windows, vec![window("seven_day", 10.0, 10_000)]);
        let gone = quota::view(Some(&snap), Timestamp(20_000_000));
        assert!(!gone.available);
    }
}

#[test]
fn a_workspace_line_carries_its_repository_only_when_it_has_one() {
    let figures = UsageFigures::default();
    let with = UsageItem {
        label: "/Users/x/.paradev/worktrees/app/a".into(),
        figures: figures.clone(),
        repo: Some("/Users/x/Work/app".into()),
        name: Some("app".into()),
    };
    let without = UsageItem {
        label: "/work/loose".into(),
        figures,
        repo: None,
        name: None,
    };
    let json = |i: &UsageItem| {
        let mut v = serde_json::to_value(i).expect("json");
        v.as_object_mut().expect("object").remove("figures");
        v.to_string()
    };
    assert_eq!(
        json(&with),
        r#"{"label":"/Users/x/.paradev/worktrees/app/a","name":"app","repo":"/Users/x/Work/app"}"#
    );
    assert_eq!(json(&without), r#"{"label":"/work/loose"}"#);
}
