//! A stand-in for the GitHub CLI, for the tests of pull-request mode: no network, no account.
//! It answers the commands the daemon runs, from a JSON state file named by
//! `FAKE_GH_STATE`; a test edits that file between steps to move checks along.
//!
//! - `pr view <number|url> --json <fields>`: the pull request (`statusCheckRollup` is its checks).
//! - `pr list --head <branch> --state all --json <fields>`: a list; empty (exit 0) for no pull request.
//! - `pr create --head <branch> --base <base> --title <t> --body <b>`: opens one and prints its URL.
//! - `pr checks <selector> --json name,bucket`: the checks; exit 8 while pending, 1 on a failure.
//! - `pr merge <selector> --merge|--squash|--rebase`: refuses unless every check passed; else the
//!   pull request is MERGED and the branch moves in the bare repository `origin` of the state.
//!
//! State: `{"logged_in": true, "origin": "<bare repo>", "new_checks": [{"name", "bucket"}],
//! "prs": [{"number", "url", "branch", "base", "title", "body", "state", "mergeable", "checks",
//! "method"}], "calls": [[args]], "fail": {"<command>": {"code", "text"}}, "auth_text"}`. Not
//! logged in, every command exits 4 as the real one does; `fail` makes one command fail with
//! the given exit code and message (the daemon must go by the code, not the words).
//! Built only to be run by the daemon's tests.

use std::process::{Command, exit};

use serde_json::{Value, json};

fn fail(code: i32, text: &str) -> ! {
    eprintln!("{text}");
    exit(code)
}

fn main() {
    let path = std::env::var("FAKE_GH_STATE").unwrap_or_else(|_| fail(2, "FAKE_GH_STATE not set"));
    let mut state: Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap_or_default())
            .unwrap_or_else(|_| json!({}));
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut calls = state["calls"].as_array().cloned().unwrap_or_default();
    calls.push(json!(args));
    state["calls"] = json!(calls);
    let code = run(&mut state, &args);
    std::fs::write(&path, state.to_string()).unwrap_or_else(|e| fail(2, &e.to_string()));
    exit(code);
}

fn flag<'a>(args: &'a [String], name: &str) -> &'a str {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .map_or("", String::as_str)
}

fn run(state: &mut Value, args: &[String]) -> i32 {
    // Like the real one: exit code 4 is "authentication required", whatever the message says.
    if state["logged_in"] != json!(true) {
        let text = state["auth_text"]
            .as_str()
            .unwrap_or("To get started with GitHub CLI, please run:  gh auth login");
        fail(4, text);
    }
    let (cmd, sel) = (args.get(1).map_or("", String::as_str), args.get(2));
    // A command made to fail: `"fail": {"<command>": {"code": 1, "text": "..."}}`.
    if let Some(f) = state["fail"].get(cmd) {
        fail(
            f["code"]
                .as_i64()
                .and_then(|c| i32::try_from(c).ok())
                .unwrap_or(1),
            f["text"].as_str().unwrap_or("failed"),
        );
    }
    let sel = sel.map_or("", String::as_str);
    let prs = state["prs"].as_array().cloned().unwrap_or_default();
    let found = prs.iter().rposition(|p| {
        p["branch"] == sel || p["number"].as_u64() == sel.parse().ok() || p["url"] == sel
    });
    match (cmd, found) {
        ("create", _) => {
            let number = prs.len() + 1;
            let url = format!("https://github.com/o/r/pull/{number}");
            let pr = json!({
                "number": number, "url": url, "branch": flag(args, "--head"),
                "base": flag(args, "--base"), "title": flag(args, "--title"),
                "body": flag(args, "--body"), "state": "OPEN", "mergeable": "MERGEABLE",
                "checks": state["new_checks"].clone(),
            });
            state["prs"] = json!(prs.into_iter().chain([pr]).collect::<Vec<_>>());
            println!("{url}");
            0
        }
        // Like the real one: a branch without a pull request is an empty list and exit code 0.
        ("list", _) => {
            let head = flag(args, "--head");
            let listed: Vec<Value> = prs
                .iter()
                .filter(|p| p["branch"] == head)
                .map(|p| shown(p, flag(args, "--json")))
                .collect();
            println!("{}", Value::Array(listed));
            0
        }
        ("view" | "checks" | "merge", None) => fail(1, "no pull requests found for branch"),
        ("view", Some(i)) => {
            println!("{}", shown(&prs[i], flag(args, "--json")));
            0
        }
        ("checks", Some(i)) => {
            let checks = prs[i]["checks"].clone();
            // Like the real one: nothing on stdout and exit code 1 when no check was reported.
            if checks.as_array().is_none_or(Vec::is_empty) {
                fail(1, "no checks reported on the 'branch' branch");
            }
            println!("{checks}");
            let buckets: Vec<&str> = checks
                .as_array()
                .map(|c| c.iter().filter_map(|x| x["bucket"].as_str()).collect())
                .unwrap_or_default();
            if buckets.iter().any(|b| matches!(*b, "fail" | "cancel")) {
                1
            } else if buckets.contains(&"pending") {
                8
            } else {
                0
            }
        }
        ("merge", Some(i)) => {
            let pr = &prs[i];
            let ready = pr["checks"].as_array().is_some_and(|c| {
                c.iter()
                    .all(|x| x["bucket"] == "pass" || x["bucket"] == "skipping")
            });
            if !ready || pr["mergeable"] == "CONFLICTING" {
                fail(
                    1,
                    "Pull request is not mergeable: required status checks are not passing",
                );
            }
            let origin = state["origin"].as_str().unwrap_or_default();
            let head = format!("refs/heads/{}", pr["branch"].as_str().unwrap_or_default());
            let base = format!("refs/heads/{}", pr["base"].as_str().unwrap_or_default());
            let sha = git(origin, &["rev-parse", &head]);
            git(origin, &["update-ref", &base, &sha]);
            let method = args.get(3).cloned().unwrap_or_default();
            state["prs"][i]["state"] = json!("MERGED");
            state["prs"][i]["method"] = json!(method);
            0
        }
        _ => fail(2, "fake-gh: unsupported command"),
    }
}

/// The `--json` fields of a pull request; `statusCheckRollup` is its checks.
fn shown(pr: &Value, fields: &str) -> Value {
    let shown: serde_json::Map<String, Value> = fields
        .split(',')
        .filter_map(|f| {
            let key = if f == "statusCheckRollup" {
                "checks"
            } else {
                f
            };
            pr.get(key).map(|v| (f.to_owned(), v.clone()))
        })
        .collect();
    Value::Object(shown)
}

fn git(dir: &str, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("--git-dir")
        .arg(dir)
        .args(args)
        .output()
        .unwrap_or_else(|e| fail(2, &e.to_string()));
    if !out.status.success() {
        fail(2, &String::from_utf8_lossy(&out.stderr));
    }
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}
