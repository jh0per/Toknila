//! The daemon is killed hard (`kill -9` / `TerminateProcess`) at 20 pseudo-random points
//! of the gate scenario. After each restart `tok ls --json` must equal the state after the last
//! command that committed, never a mix, and the finished run must equal a run without kills.

use std::path::Path;
use std::process::{Child, Command, Output, Stdio};
use std::time::Duration;

use serde_json::Value;

mod common;

const DEMO: &str = include_str!("../../../examples/demo.toml");

fn tok_cmd(home: &Path, args: &[&str]) -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_tok"));
    c.args(args).env("TOKNILA_HOME", home);
    common::isolate(&mut c, home);
    c
}

fn tok_ok(home: &Path, args: &[&str]) -> Output {
    let out = if args == ["daemon", "start"] {
        common::start_daemon(home)
    } else {
        tok_cmd(home, args).output().expect("run tok")
    };
    assert!(
        out.status.success(),
        "tok {args:?}: {}\n{}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    out
}

fn json(home: &Path, args: &[&str]) -> Value {
    let mut args = args.to_vec();
    args.push("--json");
    let out = tok_ok(home, &args);
    let mut v: Value = serde_json::from_slice(&out.stdout).expect("tok prints JSON");
    strip(&mut v);
    v
}

/// Everything a person can see, without ids and clocks, which differ between runs: projects,
/// chains, `tok ls --json`, and each ticket with its subtasks and comments. Every step of the
/// scenario changes it, so "before" and "after" a step are never equal.
fn state(home: &Path) -> Value {
    let ls = json(home, &["ls"]);
    let shown: Vec<Value> = ls
        .as_array()
        .expect("ls is a list")
        .iter()
        .map(|t| json(home, &["show", t["ticket"]["key"].as_str().expect("key")]))
        .collect();
    serde_json::json!({
        "projects": json(home, &["project", "ls"]),
        "chains": json(home, &["chain", "ls"]),
        "ls": ls,
        "tickets": shown,
    })
}

fn strip(v: &mut Value) {
    match v {
        Value::Object(m) => {
            m.retain(|k, _| {
                k != "id"
                    && !k.ends_with("_id")
                    && !k.ends_with("_since")
                    && !k.ends_with("_at")
                    && !k.ends_with("_ms")
            });
            m.values_mut().for_each(strip);
        }
        Value::Array(a) => a.iter_mut().for_each(strip),
        // Ids inside references (`"release": "01M6..."`): any ULID-shaped string.
        Value::String(t)
            if t.len() == 26
                && t.bytes()
                    .all(|b| b.is_ascii_digit() || b.is_ascii_uppercase()) =>
        {
            *t = "<id>".to_owned();
        }
        _ => {}
    }
}

fn steps(file: &str) -> Vec<Vec<String>> {
    let s = |a: &[&str]| a.iter().map(|x| (*x).to_owned()).collect::<Vec<_>>();
    vec![
        s(&["project", "new", "DEMO"]),
        s(&["chain", "import", file, "--project", "DEMO"]),
        s(&["sim", "DEMO-1", "subtask", "1", "done"]),
        s(&["sim", "DEMO-1", "subtask", "2", "done"]),
        s(&["sim", "DEMO-1", "finish", "A is done"]),
        s(&["accept", "DEMO-1"]),
        s(&["sim", "DEMO-2", "ask", "which way?"]),
        s(&["comment", "DEMO-2", "left"]),
        s(&["sim", "DEMO-2", "finish", "B is done"]),
        s(&["sim", "DEMO-3", "crash", "out of memory"]),
        s(&["unblock", "DEMO-3"]),
        s(&["sim", "DEMO-3", "finish", "D is done"]),
        s(&["return", "DEMO-3", "add a test"]),
        s(&["sim", "DEMO-3", "finish", "D with a test"]),
        s(&["accept", "DEMO-2"]),
        s(&["accept", "DEMO-3"]),
        s(&["release", "start", "R1"]),
        s(&["sim", "R1", "finish"]),
        s(&["sim", "DEMO-4", "finish", "C is done"]),
        s(&["accept", "DEMO-4"]),
    ]
}

fn daemon_pid(home: &Path) -> Option<u32> {
    let text = std::fs::read_to_string(home.join("daemon.json")).ok()?;
    let v: Value = serde_json::from_str(&text).ok()?;
    v["pid"].as_u64().and_then(|p| u32::try_from(p).ok())
}

/// Small deterministic generator, so a failure replays with the same kill points.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self, n: u64) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 33) % n
    }
}

fn run(home: &Path, args: &[String]) -> Child {
    tok_cmd(home, &args.iter().map(String::as_str).collect::<Vec<_>>())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn tok")
}

#[test]
fn kill_9_at_20_points_loses_nothing_and_mixes_nothing() {
    let work = tempfile::tempdir().expect("tempdir");
    let file = work.path().join("demo.toml");
    std::fs::write(&file, DEMO).expect("write demo");
    let file = file.to_str().expect("utf-8 path").to_owned();
    let steps = steps(&file);

    // Reference: the same steps without kills, the state after each one.
    let clean = work.path().join("clean");
    let _stop_clean = common::DaemonGuard::new(&clean);
    let mut reference = vec![state(&clean)];
    for s in &steps {
        tok_ok(&clean, &s.iter().map(String::as_str).collect::<Vec<_>>());
        reference.push(state(&clean));
    }
    let _ = tok_cmd(&clean, &["daemon", "stop"]).output();
    let final_statuses: Vec<String> = reference
        .last()
        .and_then(|v| v["ls"].as_array())
        .expect("final ls")
        .iter()
        .map(|t| format!("{} {}", t["ticket"]["key"], t["ticket"]["status"]))
        .collect();
    assert_eq!(
        final_statuses,
        [
            r#""DEMO-1" "released""#,
            r#""DEMO-2" "released""#,
            r#""DEMO-3" "released""#,
            r#""DEMO-4" "merged""#
        ],
        "the clean run must reach the end of the gate"
    );

    for (k, w) in reference.windows(2).enumerate() {
        assert_ne!(
            w[0], w[1],
            "step {:?} must change the visible state",
            steps[k]
        );
    }

    let home = work.path().join("killed");
    let _stop = common::DaemonGuard::new(&home);
    tok_ok(&home, &["daemon", "start"]);
    let mut rng = Lcg(0x70_6b_6e_69_6c_61);
    let mut kills = 0;
    let mut lost = 0;
    let mut done = 0; // steps known to be committed
    while done < steps.len() {
        // One kill per attempt until 20: a step lost to a kill is retried and may be killed again.
        let kill_now = kills < 20;
        let child = run(&home, &steps[done]);
        if kill_now {
            std::thread::sleep(Duration::from_micros(rng.next(12_000)));
            if let Some(pid) = daemon_pid(&home) {
                common::kill_hard(pid);
            }
            kills += 1;
            let _ = child.wait_with_output();
            tok_ok(&home, &["daemon", "start"]);
            let now = state(&home);
            if now == reference[done + 1] {
                done += 1;
            } else if now == reference[done] {
                lost += 1; // killed before commit: the step left no trace, so it runs again
            } else {
                let dump = std::env::temp_dir().join("toknila-crash");
                let _ = std::fs::create_dir_all(&dump);
                let _ = std::fs::write(dump.join("now.json"), format!("{now:#}"));
                let _ = std::fs::write(
                    dump.join("after.json"),
                    format!("{:#}", reference[done + 1]),
                );
                panic!(
                    "after kill {kills} at step {done} ({:?}) the state is neither before nor after it:\n{now:#}",
                    steps[done]
                );
            }
        } else {
            let out = child.wait_with_output().expect("wait");
            assert!(
                out.status.success(),
                "step {:?} after {kills} kills: {}",
                steps[done],
                String::from_utf8_lossy(&out.stderr)
            );
            done += 1;
            assert_eq!(
                state(&home),
                reference[done],
                "after step {:?}",
                steps[done - 1]
            );
        }
    }
    assert!(kills >= 20, "only {kills} kills happened");
    println!("{kills} kills, {lost} of them before the command committed");
    assert_eq!(state(&home), *reference.last().expect("final"));

    // No restart found state that disagrees with the journal.
    let logs = home.join("logs");
    let mut checked = 0;
    for e in std::fs::read_dir(&logs).expect("logs dir") {
        let text = std::fs::read_to_string(e.expect("entry").path()).unwrap_or_default();
        checked += text.matches("store checked").count();
        assert!(!text.contains("differs from the journal"), "{text}");
    }
    assert!(
        checked > kills,
        "every start logged its check ({checked} for {kills} kills)"
    );
}
