//! `tok hook <name>` in a planner's run: the real `tok` binary with the environment the runner
//! gives a planner (`TOKNILA_ROLE=planner`, no ticket), against a stand-in daemon on a local port
//! that records the bytes of every request. The hook posts its turn signal to
//! `POST /v1/planner/activity`, prints nothing on stdout and exits 0 whatever the daemon does;
//! it claims no instructions, holds no permission dialog and asks no pre-tool decision.

use std::io::{ErrorKind, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

const TOKEN: &str = "planner-run-token-0123456789";
/// How long a test lets `tok hook` run before it calls it hung.
const HUNG: Duration = Duration::from_secs(20);
/// The hook's deadline (two seconds) and room for starting a process on a loaded machine.
const WITHIN: Duration = Duration::from_secs(4);
/// What the daemon answers a recorded signal.
const NO_CONTENT: &str = "HTTP/1.1 204 No Content\r\nConnection: close\r\n\r\n";

/// A stand-in daemon: records each request's bytes and answers it with `answer`, or holds the
/// connection without a word when `answer` is `None`.
struct FakeDaemon {
    url: String,
    got: mpsc::Receiver<String>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

/// The bytes of one request: its head, then as much body as its `Content-Length` says, read with
/// a deadline.
fn read_request(stream: &mut TcpStream) -> String {
    let _ = stream.set_nonblocking(false);
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let mut bytes = Vec::new();
    let mut buf = [0u8; 4096];
    loop {
        let text = String::from_utf8_lossy(&bytes).into_owned();
        if let Some((head, body)) = text.split_once("\r\n\r\n") {
            let length = head
                .lines()
                .find_map(|l| {
                    let (name, value) = l.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().ok())
                        .flatten()
                })
                .unwrap_or(0);
            if body.len() >= length {
                return text;
            }
        }
        match stream.read(&mut buf) {
            Ok(0) | Err(_) => return String::from_utf8_lossy(&bytes).into_owned(),
            Ok(n) => bytes.extend_from_slice(&buf[..n]),
        }
    }
}

impl FakeDaemon {
    fn start(answer: Option<String>) -> FakeDaemon {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        listener.set_nonblocking(true).expect("non-blocking");
        let url = format!("http://{}", listener.local_addr().expect("address"));
        let (tx, got) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let thread = std::thread::spawn(move || {
            let mut held = Vec::new();
            loop {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let _ = tx.send(read_request(&mut stream));
                        match &answer {
                            Some(a) => {
                                let _ = stream.write_all(a.as_bytes());
                            }
                            None => held.push(stream),
                        }
                    }
                    // Asked to stop, and nothing is left in the queue: every connection a hook
                    // that has exited made is in the queue by now.
                    Err(e) if e.kind() == ErrorKind::WouldBlock && flag.load(Ordering::SeqCst) => {
                        return;
                    }
                    Err(e) if e.kind() == ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(10));
                    }
                    Err(_) => return,
                }
            }
        });
        FakeDaemon {
            url,
            got,
            stop,
            thread: Some(thread),
        }
    }

    /// Every request that arrived, once the hook that sent them has exited.
    fn requests(mut self) -> Vec<String> {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            t.join().expect("the stand-in daemon ends");
        }
        self.got.try_iter().collect()
    }
}

impl Drop for FakeDaemon {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// What a run of `tok hook` came to.
struct Ran {
    code: Option<i32>,
    stdout: String,
    stderr: String,
    took: Duration,
}

/// `tok hook <name>` with a planner's environment pointing at `url`, `stdin` as the hook's JSON.
fn planner_hook(url: &str, name: &str, stdin: &str) -> Ran {
    let started = Instant::now();
    let mut child = Command::new(env!("CARGO_BIN_EXE_tok"))
        .args(["hook", name])
        .env("TOKNILA_URL", url)
        .env("TOKNILA_RUN_TOKEN", TOKEN)
        .env("TOKNILA_ROLE", "planner")
        .env("TOKNILA_RUN", "01JBQ7ZJ0000000000000000R1")
        .env_remove("TOKNILA_TICKET")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("run tok");
    let mut input = child.stdin.take().expect("stdin");
    input
        .write_all(stdin.as_bytes())
        .expect("write the hook's JSON");
    drop(input);
    let status = loop {
        if let Some(status) = child.try_wait().expect("wait") {
            break status;
        }
        if started.elapsed() > HUNG {
            let _ = child.kill();
            let _ = child.wait();
            panic!("tok hook {name} still runs after {HUNG:?}");
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    let took = started.elapsed();
    let mut stdout = String::new();
    let mut stderr = String::new();
    let _ = child
        .stdout
        .take()
        .expect("stdout")
        .read_to_string(&mut stdout);
    let _ = child
        .stderr
        .take()
        .expect("stderr")
        .read_to_string(&mut stderr);
    Ran {
        code: status.code(),
        stdout,
        stderr,
        took,
    }
}

/// The request line and the body of a recorded request, and whether it carried `TOKEN`.
fn parts(request: &str) -> (&str, &str, bool) {
    let (head, body) = request.split_once("\r\n\r\n").expect("a whole request");
    let line = head.lines().next().unwrap_or_default();
    let bearer = head.lines().any(|l| {
        l.split_once(':').is_some_and(|(n, v)| {
            n.eq_ignore_ascii_case("authorization") && v.trim() == format!("Bearer {TOKEN}")
        })
    });
    (line, body, bearer)
}

#[test]
fn a_planners_pre_tool_use_posts_its_step_and_prints_nothing() {
    let daemon = FakeDaemon::start(Some(NO_CONTENT.to_owned()));
    let read = r#"{"session_id":"s1","transcript_path":"/h/s1.jsonl","cwd":"/repo","hook_event_name":"PreToolUse","tool_name":"Read","tool_input":{"file_path":"/repo/src/main.rs"}}"#;
    let ran = planner_hook(&daemon.url, "pre_tool_use", read);
    let requests = daemon.requests();
    for r in &requests {
        println!("--- the bytes that crossed ---\n{r}\n---");
    }
    assert_eq!(ran.code, Some(0), "{}", ran.stderr);
    assert_eq!(ran.stdout, "", "a hook's stdout is read by the agent");
    assert_eq!(ran.stderr, "");
    assert_eq!(
        requests.len(),
        1,
        "one signal and nothing else: {requests:?}"
    );
    let (line, body, bearer) = parts(&requests[0]);
    assert_eq!(line, "POST /v1/planner/activity HTTP/1.1");
    assert!(bearer, "the planner's own token: {}", requests[0]);
    assert_eq!(
        body,
        r#"{"signal":"pre_tool_use","tool":"Read: /repo/src/main.rs","tool_name":"Read"}"#
    );
}

#[test]
fn a_planners_prompt_and_stop_post_their_signal_and_the_stop_claims_nothing() {
    for (name, want) in [
        ("user_prompt_submit", r#"{"signal":"user_prompt_submit"}"#),
        ("stop", r#"{"signal":"stop"}"#),
    ] {
        let daemon = FakeDaemon::start(Some(NO_CONTENT.to_owned()));
        let input = r#"{"session_id":"s1","hook_event_name":"Stop","stop_hook_active":false,"prompt":"plan"}"#;
        let ran = planner_hook(&daemon.url, name, input);
        let requests = daemon.requests();
        assert_eq!(ran.code, Some(0), "{name}: {}", ran.stderr);
        assert_eq!(ran.stdout, "", "{name}: no block decision, no output");
        assert_eq!(requests.len(), 1, "{name}: no claim: {requests:?}");
        let (line, body, bearer) = parts(&requests[0]);
        assert_eq!(line, "POST /v1/planner/activity HTTP/1.1", "{name}");
        assert!(bearer, "{name}");
        assert_eq!(body, want);
    }
}

#[test]
fn a_planners_shell_command_asks_no_pre_tool_decision_and_its_permission_hook_does_nothing() {
    let daemon = FakeDaemon::start(Some(NO_CONTENT.to_owned()));
    let bash = r#"{"hook_event_name":"PreToolUse","tool_name":"Bash","tool_input":{"command":"git log -1"}}"#;
    let ran = planner_hook(&daemon.url, "pre_tool_use", bash);
    let requests = daemon.requests();
    assert_eq!(ran.code, Some(0), "{}", ran.stderr);
    assert_eq!(ran.stdout, "");
    let lines: Vec<&str> = requests.iter().map(|r| parts(r).0).collect();
    assert_eq!(
        lines,
        ["POST /v1/planner/activity HTTP/1.1"],
        "no /pre-tool"
    );

    // Hooks a planner is not subscribed to send nothing and hold nothing.
    for name in ["permission_request", "notification", "post_tool_use"] {
        let daemon = FakeDaemon::start(Some(NO_CONTENT.to_owned()));
        let ran = planner_hook(&daemon.url, name, bash);
        assert_eq!(ran.code, Some(0), "{name}: {}", ran.stderr);
        assert_eq!(ran.stdout, "", "{name}");
        assert!(ran.took < WITHIN, "{name} took {:?}", ran.took);
        assert_eq!(daemon.requests(), Vec::<String>::new(), "{name}");
    }
}

#[test]
fn with_no_daemon_listening_the_planners_hook_exits_0_within_its_deadline() {
    let port = {
        let l = TcpListener::bind("127.0.0.1:0").expect("bind");
        l.local_addr().expect("address").port()
    };
    let ran = planner_hook(&format!("http://127.0.0.1:{port}"), "stop", "{}");
    assert_eq!(ran.code, Some(0), "{}", ran.stderr);
    assert_eq!(ran.stdout, "");
    assert!(ran.took < WITHIN, "took {:?}", ran.took);
    assert_eq!(ran.stderr.lines().count(), 1, "one line: {:?}", ran.stderr);
    assert!(
        ran.stderr.starts_with("toknila hook stop: "),
        "{}",
        ran.stderr
    );
}

#[test]
fn a_daemon_that_does_not_answer_holds_the_planners_hook_two_seconds_at_most() {
    let daemon = FakeDaemon::start(None);
    let ran = planner_hook(&daemon.url, "pre_tool_use", r#"{"tool_name":"Read"}"#);
    assert_eq!(ran.code, Some(0), "{}", ran.stderr);
    assert_eq!(ran.stdout, "");
    assert!(
        ran.took >= Duration::from_secs(2),
        "it waited its deadline: {:?}",
        ran.took
    );
    assert!(ran.took < WITHIN, "took {:?}", ran.took);
    assert_eq!(ran.stderr.lines().count(), 1, "one line: {:?}", ran.stderr);
    assert!(ran.stderr.contains("2 s"), "{}", ran.stderr);
    assert_eq!(daemon.requests().len(), 1, "the signal was sent");
}

#[test]
fn a_refusal_or_an_answer_it_does_not_understand_still_exits_0() {
    let refusal = r#"{"error":"only the planner reports its turn"}"#;
    for answer in [
        format!(
            "HTTP/1.1 403 Forbidden\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\
             Connection: close\r\n\r\n{refusal}",
            refusal.len()
        ),
        "HTTP/1.1 200 OK\r\nContent-Length: 9\r\nConnection: close\r\n\r\nnot json!".to_owned(),
    ] {
        let daemon = FakeDaemon::start(Some(answer.clone()));
        let ran = planner_hook(&daemon.url, "user_prompt_submit", "{}");
        assert_eq!(ran.code, Some(0), "{answer}: {}", ran.stderr);
        assert_eq!(ran.stdout, "", "{answer}");
        assert!(ran.took < WITHIN, "took {:?}", ran.took);
        assert_eq!(ran.stderr.lines().count(), 1, "{answer}: {:?}", ran.stderr);
        assert_eq!(daemon.requests().len(), 1, "{answer}");
    }
}
