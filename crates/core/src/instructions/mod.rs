//! What the run's and a person's words look like to the agent. Pure.
//!
//! - [`generate`]: the instructions file of one run (the ticket's prompt and acceptance, the
//!   comment template, short-output rules, how to report through the MCP tools). The runner
//!   writes it under the data home and passes it by a flag; the project's own `CLAUDE.md` and
//!   `AGENTS.md` are never touched. [`protocol`] is the part that does not depend on the ticket;
//!   the MCP server sends it as its `instructions`.
//! - [`generate_review`]: the same for the reviewer agent, whose one tool is its verdict.
//! - [`route`]: how a person's pending instruction reaches a live agent.
//! - [`typed`] and [`for_hook`]: the text of that delivery.
//! - [`terminal`]: the bounds of a terminal size.

mod planner;
mod protocol;
pub mod terminal;
#[cfg(test)]
mod tests;

pub use planner::{ProjectFacts, generate_planner};

use crate::{Comment, CommentKind, template};

/// The part of the planner's instructions that is the same for every project; the MCP server
/// sends it as its `instructions`.
pub fn planner_protocol() -> &'static str {
    planner::PLANNER_PROTOCOL
}

/// What the generated file says about one run.
#[derive(Debug, Clone, Copy)]
pub struct RunBrief<'a> {
    pub key: &'a str,
    pub title: &'a str,
    pub prompt: &'a str,
    pub acceptance: &'a [String],
    /// The project's check command, named in both roles' instructions.
    pub check_command: Option<&'a str>,
}

/// The first message of a run, passed to the agent when it starts (the profile's `{kickoff}`
/// placeholder), because an agent that starts at an empty prompt waits for someone to tell it to
/// begin. Short and one line: the details are in the instructions file the agent already has.
pub fn kickoff(role: crate::AgentRole, brief: &RunBrief<'_>) -> String {
    let checks = brief
        .check_command
        .map(|c| format!(" Run `{c}` before you hand in."))
        .unwrap_or_default();
    match role {
        crate::AgentRole::Planner => format!(
            "You are the planner of project {key}. Read your Toknila instructions, call \
             `show_plan`, then tell the owner in a line or two how the plan stands and wait for \
             what they want planned. Do not start any card.",
            key = brief.key
        ),
        crate::AgentRole::Reviewer => format!(
            "Review ticket {key} now, as your Toknila instructions say: read what changed on this \
             branch, run the checks, then call `review_verdict` once. You do not change the code.",
            key = brief.key
        ),
        _ => format!(
            "Start ticket {key} now. Read your Toknila instructions, work the card, commit your \
             work on this branch, then call `finished`.{checks}",
            key = brief.key
        ),
    }
}

/// The section that names the project's check command, if it has one.
fn checks_section(brief: &RunBrief<'_>, who: &str) -> String {
    match brief.check_command {
        Some(c) => format!(
            "\n## Checks\n\nThe project's check command is `{c}`. {who} You may run it without \
             asking.\n"
        ),
        None => String::new(),
    }
}

/// The part of the instructions that is the same for every ticket.
pub fn protocol() -> &'static str {
    protocol::PROTOCOL
}

/// The instructions file of one run.
pub fn generate(brief: &RunBrief<'_>) -> String {
    let mut out = format!(
        "# Toknila: ticket {key}, {title}\n\n\
         You work on ticket {key} for Toknila. The repository's own `CLAUDE.md` and `AGENTS.md` \
         still apply and win where they differ; this file only adds how to work with Toknila.\n\n\
         ## Task\n\n{prompt}\n",
        key = brief.key,
        title = brief.title,
        prompt = brief.prompt.trim(),
    );
    if !brief.acceptance.is_empty() {
        out.push_str("\n## Acceptance\n\nDone means every item holds, each with evidence:\n\n");
        for item in brief.acceptance {
            out.push_str(&format!("- [ ] {}\n", item.trim()));
        }
    }
    out.push_str(&checks_section(
        brief,
        "Run it before you call `finished`; the work is not done while it fails.",
    ));
    out.push('\n');
    out.push_str(protocol::PROTOCOL);
    out
}

/// The part of the reviewer's instructions that is the same for every ticket.
pub fn review_protocol() -> &'static str {
    protocol::REVIEW_PROTOCOL
}

/// The instructions file of a review run: what to check, and how to give the verdict.
pub fn generate_review(brief: &RunBrief<'_>) -> String {
    let mut out = format!(
        "# Toknila: review of ticket {key}, {title}\n\n\
         You review the finished work on ticket {key} for Toknila. You do not change the code. \
         The repository's own `CLAUDE.md` and `AGENTS.md` still apply to what good work is.\n\n\
         ## The task that was given\n\n{prompt}\n",
        key = brief.key,
        title = brief.title,
        prompt = brief.prompt.trim(),
    );
    if !brief.acceptance.is_empty() {
        out.push_str("\n## Acceptance\n\nCheck that every item holds, with evidence:\n\n");
        for item in brief.acceptance {
            out.push_str(&format!("- [ ] {}\n", item.trim()));
        }
    }
    out.push_str(&checks_section(
        brief,
        "Run it to see the result for yourself, and report it as a check in your verdict.",
    ));
    out.push('\n');
    out.push_str(protocol::REVIEW_PROTOCOL);
    out
}

/// How a pending instruction reaches the live agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    /// Type it into the agent's terminal now.
    Type,
    /// Hold it; the agent's next Stop hook picks it up.
    WaitForHook,
}

/// The Stop hook is the way in while the agent works, because typing then would land in the
/// middle of a turn. Without hooks, or with the agent idle at its prompt, typing is the way.
pub fn route(hooks: bool, idle: bool) -> Route {
    if !hooks || idle {
        Route::Type
    } else {
        Route::WaitForHook
    }
}

/// The instruction as one line of typed input: a terminal prompt takes a newline as "send".
pub fn typed(c: &Comment) -> String {
    let text = template::render(c.kind, &c.body);
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The reason a Stop hook gives the agent to continue: the instructions in the comment template,
/// oldest first.
pub fn for_hook(comments: &[Comment]) -> String {
    let mut texts: Vec<String> = comments
        .iter()
        .filter(|c| c.kind == CommentKind::Instruction)
        .map(|c| template::render(c.kind, &c.body).trim_end().to_owned())
        .collect();
    if texts.len() > 1 {
        let head = format!("{} new instructions from a person, in order:", texts.len());
        texts.insert(0, head);
    }
    texts.join("\n\n")
}
