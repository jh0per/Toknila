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

mod designs;
mod gate;
mod pending;
mod planner;
mod protocol;
mod record;
mod setup;
pub mod terminal;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_gate;
#[cfg(test)]
mod tests_record;
#[cfg(test)]
mod tests_return;
mod waiting;

pub use designs::BriefDesign;
use designs::{design_section, designs_section};
pub use pending::{PendingReason, PendingView, pending_reason};
pub use planner::{Epic, ProjectFacts, generate_planner};
pub use record::BriefThread;
pub use waiting::{Returned, Waiting, returned_by, short_commit, waiting};

use crate::{Comment, CommentKind, template};

/// The part of the planner's instructions that is the same for every project; the MCP server
/// sends it as its `instructions`.
pub fn planner_protocol() -> &'static str {
    planner::PLANNER_PROTOCOL
}

/// What a run that replaces one the app's restart lost is told first.
pub const RESTARTED: &str = "The app restarted while the run before you worked; its commits are on this branch, read them before you continue.";

/// What the generated file says about one run.
#[derive(Debug, Clone, Copy, Default)]
pub struct RunBrief<'a> {
    pub key: &'a str,
    pub title: &'a str,
    pub prompt: &'a str,
    pub acceptance: &'a [String],
    /// The project's check command, named in both roles' instructions.
    pub check_command: Option<&'a str>,
    /// The project's setup command, which the daemon ran in the workspace before the worker
    /// started (a worker only).
    pub setup_command: Option<&'a str>,
    /// Instructions a person gave to an earlier run of this card ([`earlier`]), as quoted text.
    pub earlier: &'a [String],
    /// This run replaces one the app's restart lost ([`Effect::ResumeRunner`] with `lost`).
    ///
    /// [`Effect::ResumeRunner`]: crate::machine::Effect::ResumeRunner
    pub restarted: bool,
    /// Instructions no run has acted on yet ([`waiting`]), as quoted text: the run reads them
    /// first.
    pub waiting: &'a [String],
    /// The Return this run answers: the card came back from review with changes to make.
    pub returned: Option<&'a Returned>,
    /// The person wants to approve a design before any code (`Ticket::design_first`).
    pub design_first: bool,
    /// The designs the card references, with the file each is kept in.
    pub designs: &'a [BriefDesign],
    /// A planner's project has designs the planner must look at first.
    pub has_designs: bool,
    /// What the person and the project's assistant said about this card
    /// ([`crate::discussion::transcript`]), oldest first.
    pub discussion: &'a [String],
    /// The chain the card belongs to, as its epic: the chain's name and its goal, which is what
    /// every card of the chain works toward. `None` or an empty goal says nothing.
    pub epic: Option<(&'a str, &'a str)>,
    /// What the daemon's check command said about the hand-in under review (a reviewer only);
    /// `None` when it did not run, e.g. the project has no check command.
    pub check: Option<&'a crate::CheckOutcome>,
    /// The card's threads, which the reviewer cannot open (a reviewer only).
    pub threads: &'a [BriefThread],
    /// The whole hand-in report under review (a reviewer only).
    pub hand_in: Option<&'a str>,
}

/// The section that carries the discussion of the card with the assistant; empty when there is
/// none.
fn discussion_section(brief: &RunBrief<'_>) -> String {
    if brief.discussion.is_empty() {
        return String::new();
    }
    let mut out = String::from(
        "\n## Discussion with the assistant\n\n\
         The person talked this story over with the project's assistant. The Task and Acceptance \
         above already hold every edit the person accepted; a proposal that is rejected or still \
         waiting is not part of the story. Oldest first:\n\n",
    );
    for line in brief.discussion {
        out.push_str(&format!("- {line}\n"));
    }
    out
}

/// The section of the instructions file that names the epic (the chain's goal); empty without one.
fn epic_section(brief: &RunBrief<'_>) -> String {
    match brief.epic {
        Some((name, goal)) if !goal.trim().is_empty() => format!(
            "\n## Epic: {name}\n\nThis story belongs to the epic {name}. The epic's goal is \
             the purpose of every story in it; keep the work toward it:\n\n{}\n",
            goal.trim()
        ),
        _ => String::new(),
    }
}

/// How many earlier instructions a replacement run is shown, the newest ones.
const EARLIER_MAX: usize = 10;

/// The instructions an earlier run of this card was given, for the run that replaces it: every
/// instruction comment that was delivered (a person's, a Return's, the reviewer's), oldest first,
/// the last [`EARLIER_MAX`] of them, each rendered in the comment template. A comment that is still
/// pending is not here: it reaches the agent the usual way.
pub fn earlier(comments: &[Comment]) -> Vec<String> {
    let mut sent: Vec<&Comment> = comments
        .iter()
        .filter(|c| c.kind == CommentKind::Instruction && c.delivered_at.is_some())
        .collect();
    sent.sort_by_key(|c| c.created_at);
    let skip = sent.len().saturating_sub(EARLIER_MAX);
    sent.into_iter()
        .skip(skip)
        .map(|c| template::render(c.kind, &c.body).trim_end().to_owned())
        .collect()
}

/// The section of the instructions file that carries `earlier`; empty when there are none.
fn earlier_section(brief: &RunBrief<'_>) -> String {
    if brief.earlier.is_empty() {
        return String::new();
    }
    let mut out = String::from(
        "\n## Instructions you were given earlier\n\n\
         A person gave these to the run of this story before you. They were delivered, and they \
         still apply: some may be done already in the commits on this branch, so look at what is \
         there, then carry on and follow the rest. Oldest first:\n",
    );
    for text in brief.earlier {
        out.push('\n');
        for line in text.lines() {
            out.push_str(line_quoted(line).as_str());
        }
    }
    out
}

/// One line of a quoted block.
fn line_quoted(line: &str) -> String {
    if line.is_empty() {
        ">\n".to_owned()
    } else {
        format!("> {line}\n")
    }
}

/// The first message of a run, passed to the agent when it starts (the profile's `{kickoff}`
/// placeholder), because an agent that starts at an empty prompt waits for someone to tell it to
/// begin. Short and one line: the details are in the instructions file the agent already has.
pub fn kickoff(role: crate::AgentRole, brief: &RunBrief<'_>) -> String {
    let mut checks = brief
        .check_command
        .map(|c| format!(" Run `{c}` before you hand in."))
        .unwrap_or_default();
    if !brief.earlier.is_empty() {
        checks.push_str(
            " The instructions you were given earlier are in your Toknila instructions; follow them too.",
        );
    }
    let first = waiting::kickoff_part(brief.waiting, brief.returned);
    if matches!(role, crate::AgentRole::Worker | crate::AgentRole::Release) {
        checks.push_str(&setup::kickoff_part(brief));
    }
    if brief.design_first {
        checks.push_str(
            " Before any code, open a design thread with a first version and wait for the \
             person to approve it (see \"Design first\" in your Toknila instructions).",
        );
    }
    match role {
        crate::AgentRole::Planner if brief.has_designs => format!(
            "You are the planner of project {key}. Read your Toknila instructions, call \
             `show_plan`. The project has designs: Read every design file `show_plan` lists \
             first, say in a few lines what screens and flows you see, then propose the stories, \
             and name the designs each story you add comes from. Do not start any story.",
            key = brief.key
        ),
        crate::AgentRole::Planner => format!(
            "You are the planner of project {key}. Read your Toknila instructions, call \
             `show_plan` to know the plan, say nothing about it yet, and wait for the owner's \
             message: answer what they ask, and plan only when they ask you to. Only if a story \
             sits in the wrong epic, as \"A story in the wrong epic\" says, point it out in one \
             `say` now, once; never move it without the owner's yes. Do not start any story.",
            key = brief.key
        ),
        crate::AgentRole::Reviewer => format!(
            "Review ticket {key} now, as your Toknila instructions say: read what changed on this \
             branch, run the single tests the hand-in names and whatever the diff makes you doubt \
             and quote their result lines (each command plain, one per call, no `cd` or `&&`), \
             mark NOT_RUN what you could not run (never FAIL it), then call `review_verdict` \
             once. You do not change the code. Do not run the whole gate again.{since}\n\n\
             {gate}",
            key = brief.key,
            since = waiting::reviewer_part(brief.returned),
            gate = gate::text(brief.check),
        ),
        _ => format!(
            "Start ticket {key} now. Read your Toknila instructions, work the story, commit your \
             work on this branch, then call `finished` and stop. Ask with `ask_human` when you \
             need a decision.{checks}{first}",
            key = brief.key
        ),
    }
}

/// The section that names the project's check command, if it has one.
fn checks_section(brief: &RunBrief<'_>, who: &str) -> String {
    match brief.check_command {
        Some(c) => format!(
            "\n## Checks\n\nThe project's check command is `{c}`. {who} Run it exactly as \
             given, with no `cd` or `export` in front, so that it is approved without asking; \
             the ship is already your working directory. Run it once, not in a loop: only one \
             check runs on this machine at a time, so another run makes the others wait. A migration \
             follows AGENTS.md's Adding a migration bullet. When `apps/web/node_modules` is \
             missing, run `npm ci --no-audit --no-fund` in `apps/web` before the web gate; \
             do not ask and do not skip the gate.\n"
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
    out.push_str(&epic_section(brief));
    out.push_str(&designs_section(brief));
    out.push_str(&design_section(brief));
    out.push_str(&discussion_section(brief));
    out.push_str(&waiting::sections(brief.waiting, brief.returned));
    if brief.restarted {
        out.push_str(&format!("\n{RESTARTED}\n"));
    }
    out.push_str(&earlier_section(brief));
    out.push_str(&setup::section(brief));
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
    if let Some(s) = brief.returned.and_then(|r| r.since.as_ref()) {
        out.push_str(&format!(
            "\n## Since the Return\n\nThis hand-in answers a Return. Read what changed since it \
             first, then the rest:\n\n```\n{}```\n",
            s.text()
        ));
    }
    out.push_str(&record::threads_section(brief));
    out.push_str(&record::hand_in_section(brief));
    out.push_str(&format!(
        "\n## The gate's result\n\n{}\n",
        gate::text(brief.check)
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
