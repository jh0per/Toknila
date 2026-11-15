//! The planner's turn and its step. A turn is on from the moment the planner takes a message
//! until it stops; a step is one line a person can read about what it does now ("Reading the
//! plan", "Reading apps/web/src/main.tsx", "Adding a story"). Pure: the daemon keeps the turn of
//! each project's live planner in memory and journals only its start and its end.

use std::path::{Component, Path};

use serde::{Deserialize, Serialize};

use crate::Timestamp;

/// The longest step, in characters.
pub const STEP_MAX: usize = 120;

/// What the planner's process reports, named as its hook is (`tok hook <name>`).
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema), schema(as = PlannerSignal))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Signal {
    /// It took a message: its turn starts.
    UserPromptSubmit,
    /// A tool is about to run.
    PreToolUse,
    /// It stopped: its turn ends.
    Stop,
}

/// The planner's turn as a window reads it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Turn {
    /// A turn is on.
    pub working: bool,
    /// What it does now; `None` outside a turn.
    pub step: Option<String>,
    /// When the turn started; `None` outside a turn.
    pub since: Option<Timestamp>,
}

impl Turn {
    /// The turn after `signal` came at `at` with `step` as its line, and `Some(working)` when
    /// the turn started or ended (what a `planner.turn` journal entry carries).
    pub fn after(
        &self,
        signal: Signal,
        step: Option<String>,
        at: Timestamp,
    ) -> (Turn, Option<bool>) {
        match signal {
            Signal::Stop => (Turn::default(), self.working.then_some(false)),
            // A tool outside a turn (the first prompt the planner was started with) starts one.
            Signal::UserPromptSubmit | Signal::PreToolUse => {
                let started = !self.working;
                let since = if started { Some(at) } else { self.since };
                let turn = Turn {
                    working: true,
                    step,
                    since,
                };
                (turn, started.then_some(true))
            }
        }
    }
}

const WORKING: &str = "Working";
const SEARCHING: &str = "Searching the code";

/// The step a signal stands for. `tool_name` and `tool` are a `pre_tool_use`'s: the hook's tool
/// name and the summary [`crate::hook::tool_text`] gives (`Read: /repo/src/main.rs`); `root` is
/// the folder the planner works in, and a path inside it is shown relative to it. A command's
/// text and a file's content never reach a step; a path outside `root` shows its file name only.
pub fn step(
    signal: Signal,
    tool_name: Option<&str>,
    tool: Option<&str>,
    root: Option<&Path>,
) -> Option<String> {
    match signal {
        Signal::Stop => None,
        Signal::UserPromptSubmit => Some("Reading your message".to_owned()),
        Signal::PreToolUse => Some(tool_step(tool_name, tool, root)),
    }
}

fn tool_step(tool_name: Option<&str>, tool: Option<&str>, root: Option<&Path>) -> String {
    let tool = tool.map(str::trim).filter(|t| !t.is_empty());
    let name = tool_name
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .or_else(|| tool.map(|t| t.split_once(": ").map_or(t, |(n, _)| n)));
    let Some(name) = name else {
        return WORKING.to_owned();
    };
    // What `tool_text` wrote after the name: the path, the pattern, the command.
    let arg = tool
        .and_then(|t| match t.strip_prefix(name) {
            Some(rest) => rest.strip_prefix(':').map(str::trim),
            None => Some(t),
        })
        .filter(|a| !a.is_empty());
    if let Some(own) = name.strip_prefix("mcp__toknila__") {
        return plan_step(own).unwrap_or(WORKING).to_owned();
    }
    let line = match name {
        "Read" => return file_step("Reading", arg, root),
        "Write" => return file_step("Writing", arg, root),
        "Edit" | "MultiEdit" | "NotebookEdit" => return file_step("Changing", arg, root),
        "Grep" | "Glob" | "LS" => SEARCHING,
        "Bash" if arg.is_some_and(reads_only) => SEARCHING,
        "Bash" => "Running a command",
        "WebFetch" => "Reading a web page",
        "WebSearch" => "Searching the web",
        _ => WORKING,
    };
    line.to_owned()
}

/// The step of a tool of the planner's own MCP server (`toknila`).
fn plan_step(tool: &str) -> Option<&'static str> {
    Some(match tool {
        "show_plan" => "Reading the plan",
        "show_design" => "Looking at a design",
        "design_question" => "Asking about a design",
        "new_chain" => "Adding an epic",
        "edit_chain" => "Changing an epic",
        "add_card" => "Adding a story",
        "edit_card" => "Changing a story",
        "move_card" => "Moving a story",
        "add_dependency" | "remove_dependency" => "Changing the order of stories",
        "show_card" => "Showing you a story",
        "suggest" => "Suggesting your next step",
        "say" => "Writing to you",
        "open_thread" => "Opening a thread",
        "thread_message" | "thread_version" => "Writing in a thread",
        _ => return None,
    })
}

/// `<verb> <path>`, the path relative to `root` when it lies inside, else its file name.
fn file_step(verb: &str, path: Option<&str>, root: Option<&Path>) -> String {
    let Some(path) = path.map(Path::new) else {
        return format!("{verb} a file");
    };
    let relative = |p: &Path| matches!(p.components().next(), Some(Component::Normal(_)));
    let inside = root
        .and_then(|r| path.strip_prefix(r).ok())
        .or_else(|| relative(path).then_some(path))
        .filter(|p| p.components().all(|c| matches!(c, Component::Normal(_))))
        .filter(|p| p.components().next().is_some());
    let shown = match inside {
        Some(p) => p
            .components()
            .map(|c| c.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/"),
        None => match path.file_name() {
            Some(name) => name.to_string_lossy().into_owned(),
            None => return format!("{verb} a file"),
        },
    };
    fit(verb, &shown)
}

/// `<verb> <shown>` in at most [`STEP_MAX`] characters: a long path loses its start, so the
/// file's name stays.
fn fit(verb: &str, shown: &str) -> String {
    let line = format!("{verb} {shown}");
    if line.chars().count() <= STEP_MAX {
        return line;
    }
    let room = STEP_MAX.saturating_sub(verb.chars().count() + 2);
    let skip = shown.chars().count().saturating_sub(room);
    let tail: String = shown.chars().skip(skip).collect();
    // Start at a folder's boundary when there is one.
    let tail = match tail.find('/') {
        Some(at) if at + 1 < tail.len() => &tail[at..],
        _ => tail.as_str(),
    };
    format!("{verb} …{tail}")
}

/// Programs that only read, and the subcommands of `git` that only read.
const READS: [&str; 16] = [
    "cat", "cd", "cut", "du", "file", "find", "grep", "head", "ls", "pwd", "rg", "sort", "stat",
    "tail", "tree", "wc",
];
const GIT_READS: [&str; 11] = [
    "blame",
    "cat-file",
    "diff",
    "grep",
    "log",
    "ls-files",
    "ls-tree",
    "merge-base",
    "rev-parse",
    "show",
    "status",
];

/// Whether every part of a shell command only reads (a search, `git log`, `cat`); a redirect
/// or a substitution writes or runs more, so it is not.
fn reads_only(command: &str) -> bool {
    if command.contains(['>', '`']) || command.contains("$(") {
        return false;
    }
    command.split(['|', ';', '&', '\n']).all(|part| {
        let mut words = part.split_whitespace();
        match words.next() {
            None => true,
            Some("git") => words.next().is_some_and(|sub| GIT_READS.contains(&sub)),
            Some(program) => READS.contains(&program),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const ROOT: &str = "/w/repo";

    fn tool(name: &str, text: Option<&str>) -> Option<String> {
        step(Signal::PreToolUse, Some(name), text, Some(Path::new(ROOT)))
    }

    #[test]
    fn the_step_table() {
        let plan = |t: &str| tool(&format!("mcp__toknila__{t}"), None);
        let rows: Vec<(Option<String>, &str)> = vec![
            (
                step(Signal::UserPromptSubmit, None, None, None),
                "Reading your message",
            ),
            (plan("show_plan"), "Reading the plan"),
            (plan("show_design"), "Looking at a design"),
            (plan("design_question"), "Asking about a design"),
            (plan("new_chain"), "Adding an epic"),
            (plan("edit_chain"), "Changing an epic"),
            (plan("add_card"), "Adding a story"),
            (plan("edit_card"), "Changing a story"),
            (plan("move_card"), "Moving a story"),
            (plan("add_dependency"), "Changing the order of stories"),
            (plan("remove_dependency"), "Changing the order of stories"),
            (plan("show_card"), "Showing you a story"),
            (plan("suggest"), "Suggesting your next step"),
            (plan("say"), "Writing to you"),
            (plan("open_thread"), "Opening a thread"),
            (plan("thread_message"), "Writing in a thread"),
            (plan("thread_version"), "Writing in a thread"),
            (
                tool(
                    "Read",
                    Some("Read: /w/repo/apps/web/src/views/ChatPane.tsx"),
                ),
                "Reading apps/web/src/views/ChatPane.tsx",
            ),
            // Outside the repository only the file's name is shown, never where it lies.
            (
                tool("Read", Some("Read: /home/someone/notes/secret-plan.md")),
                "Reading secret-plan.md",
            ),
            (tool("Read", None), "Reading a file"),
            (tool("Grep", Some("Grep: fn step")), "Searching the code"),
            (tool("Glob", Some("Glob: **/*.rs")), "Searching the code"),
            (
                tool("Bash", Some("Bash: git log --oneline -20")),
                "Searching the code",
            ),
            (
                tool("Bash", Some("Bash: cd /w/repo && git show HEAD | head -50")),
                "Searching the code",
            ),
            (
                tool("Bash", Some("Bash: rm -rf target && cargo build")),
                "Running a command",
            ),
            (
                tool("Bash", Some("Bash: git log > /tmp/out.txt")),
                "Running a command",
            ),
            (
                tool("WebFetch", Some("WebFetch: https://example.com")),
                "Reading a web page",
            ),
            (tool("WebSearch", None), "Searching the web"),
            (tool("TodoWrite", None), "Working"),
            (tool("mcp__other__thing", None), "Working"),
            (plan("not_a_tool"), "Working"),
            (step(Signal::PreToolUse, None, None, None), "Working"),
        ];
        for (got, want) in &rows {
            assert_eq!(got.as_deref(), Some(*want), "row {want:?}");
        }
        assert_eq!(
            step(Signal::Stop, None, None, None),
            None,
            "a turn that ended has no step"
        );
    }

    #[test]
    fn the_tool_name_is_read_from_the_summary_when_the_hook_gave_none() {
        let got = step(
            Signal::PreToolUse,
            None,
            Some("Read: /w/repo/README.md"),
            Some(Path::new(ROOT)),
        );
        assert_eq!(got.as_deref(), Some("Reading README.md"));
    }

    #[test]
    fn no_step_is_longer_than_120_characters_or_carries_a_commands_text() {
        let deep = format!("Read: {ROOT}/{}/component.tsx", "nested/".repeat(40));
        let got = tool("Read", Some(&deep)).expect("a step");
        assert!(
            got.chars().count() <= STEP_MAX,
            "{} chars: {got}",
            got.chars().count()
        );
        assert!(got.starts_with("Reading …"), "{got}");
        assert!(
            got.ends_with("/component.tsx"),
            "the file's name stays: {got}"
        );
        let long_name = format!("Read: {ROOT}/{}.rs", "x".repeat(300));
        let got = tool("Read", Some(&long_name)).expect("a step");
        assert!(got.chars().count() <= STEP_MAX, "{got}");

        let secret = "Bash: git log --grep=hunter2-password";
        let got = tool("Bash", Some(secret)).expect("a step");
        assert!(!got.contains("hunter2") && !got.contains("git"), "{got}");
        let got = tool("Bash", Some("Bash: curl -H 'token: abc123' x")).expect("a step");
        assert!(!got.contains("abc123") && !got.contains("curl"), "{got}");
    }

    #[test]
    fn a_message_starts_the_turn_a_tool_keeps_it_and_stop_ends_it() {
        let (t1, t2, t3) = (Timestamp(10), Timestamp(20), Timestamp(30));
        let idle = Turn::default();
        let (on, change) = idle.after(
            Signal::UserPromptSubmit,
            Some("Reading your message".into()),
            t1,
        );
        assert_eq!(change, Some(true), "the start is journaled");
        assert_eq!(
            on,
            Turn {
                working: true,
                step: Some("Reading your message".into()),
                since: Some(t1)
            }
        );
        let (busy, change) = on.after(Signal::PreToolUse, Some("Reading the plan".into()), t2);
        assert_eq!(change, None, "a step is no journal entry");
        assert_eq!(busy.step.as_deref(), Some("Reading the plan"));
        assert_eq!(busy.since, Some(t1), "the turn keeps its start");
        // A second message inside the turn is the same turn.
        let (again, change) = busy.after(
            Signal::UserPromptSubmit,
            Some("Reading your message".into()),
            t2,
        );
        assert_eq!((again.since, change), (Some(t1), None));
        let (off, change) = again.after(Signal::Stop, None, t3);
        assert_eq!(change, Some(false), "the end is journaled");
        assert_eq!(off, Turn::default());
        // A stop outside a turn ends nothing and journals nothing.
        assert_eq!(off.after(Signal::Stop, None, t3), (Turn::default(), None));
        // A tool outside a turn (the first prompt the planner was started with) starts one.
        let (on, change) =
            Turn::default().after(Signal::PreToolUse, Some("Reading the plan".into()), t3);
        assert_eq!((on.working, on.since, change), (true, Some(t3), Some(true)));
    }

    #[test]
    fn signals_are_named_as_their_hooks() {
        for (signal, name) in [
            (Signal::UserPromptSubmit, "user_prompt_submit"),
            (Signal::PreToolUse, "pre_tool_use"),
            (Signal::Stop, "stop"),
        ] {
            assert_eq!(serde_json::to_value(signal).expect("json"), name);
            assert!(
                crate::hook::is_known(name),
                "{name} is a hook of crate::hook"
            );
        }
    }
}
