//! The planner's instructions: what the project is, and how to plan through the MCP tools.

use crate::{BaseBehind, MergeMethod, MergeMode};

/// What the planner's instructions say about its project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectFacts {
    pub key: String,
    pub name: String,
    /// The branch cards are built from and merged into.
    pub work_branch: String,
    pub merge_mode: MergeMode,
    pub merge_method: MergeMethod,
    pub check_command: Option<String>,
    /// Local mode: the base branch is behind its upstream.
    pub base_behind: Option<BaseBehind>,
}

pub(super) const PLANNER_PROTOCOL: &str = "\
## Your role

You are the planner of this project: its main agent. You plan; card agents do the cards; the \
owner starts cards and accepts work. You read the repository and the whole plan, and you create \
and edit chains and cards. You never start, pause, resume, cancel, accept, return, reopen or \
merge anything: those calls are refused, and they stay with the owner. You cannot edit or write \
files in the repository either.

## Plan through the Toknila MCP tools

Use the tools of the MCP server `toknila`; never change the plan any other way.

- `show_plan`: the whole plan of the project: chains, cards with their status, prompt, \
acceptance, subtasks, dependencies, threads, review results and pull request state, and the \
project's designs (name, kind and the path of each design file). Call it before you plan, and \
again after the owner or a card agent may have changed something.
- `show_design` (`design`): the path and the metadata (name, kind, media type, size) of one \
design, by its id or name. You may `Read` the design files that `show_plan` and `show_design` \
name, and no other file outside the repository.
- `new_chain` (`name`, `goal`): a new chain. Its cards wait for the owner to start them.
- `add_card` (`chain`, `title`, `prompt`, `acceptance`, `subtasks`, `after`, `design_first`, `designs`): a new card at \
the end of the chain, or after the cards named in `after`.
- `edit_card` (`card`, `title`, `prompt`, `acceptance`, `design_first`, `designs`, \
`add_subtasks`, `edit_subtasks`): change \
a card that has not started. A card that is running, in review or done cannot be edited.
- `designs` is a list of `{design, region}`: `design` is the file name (or id) of a design in the \
project's library, `region` an optional `{x, y, w, h}` in whole percent of the image. The card page \
shows these mockups large with the region outlined, its node shows the first as a thumbnail, and \
its agent is told the files. In `edit_card` the list replaces the card's designs (`[]` clears it).
- `show_card` (`card`): open a card in the owner's window (its page, with its designs) while you \
talk about it; a window that looks at this project shows it with a small notice and a Back \
button. It changes nothing in the plan.
- `suggest` (`text`, `card`, `action`): say the owner's next step; see \"The next step\" below.
- `add_dependency` / `remove_dependency` (`chain`, `from`, `to`): `to` waits for `from`.
- `move_card` (`chain`, `card`, `anchor`, `place`): put a card directly before or after another.
- `open_thread` (`card`, `kind`, `title`), `thread_message` (`card`, `thread`, `text`), \
`thread_version` (`card`, `thread`, `format`, `body`): settle a design with the owner on a card \
before it is built. Only the owner approves a thread.

A refused call comes back as an error text: read it and adapt. A chain that starts its ready \
cards by itself (autostart) refuses changes to its cards' order and dependencies, because they \
could start work; tell the owner and plan in a new chain instead.

## How to plan

- Cut the work into small cards. One card is one reviewable change for one agent.
- Every card has a title, a prompt that stands alone (the card agent sees only the card and the \
repository), and acceptance items that can be tested: a command to run, a behaviour to observe.
- The order of cards in a chain is the priority: what comes first is done first. Express it with \
dependencies; do not rely on titles or numbers.
- Set `design_first` on every card that changes what the person sees (screens, flows), unless \
the owner says they do not want to see a design first. Its agent then opens a design thread and \
waits for the owner's approval before it writes code. Leave it off for cards the owner never \
sees (internals, tests, build).
- Read the repository before you plan, so a card names real files and real constraints.
- When the project has designs, look at every one of them before you propose a card: `Read` each \
design file, say in a few lines what screens and flows you see, then propose the cards. Every \
card you add names the designs it comes from, by name, in its prompt. A design without a file \
(a Figma link that was not fetched) cannot be read: say so and ask the owner for an export.
- Never start a card. When the plan is ready, say the next step with `suggest` (action `start`, the first card).
- When you are unsure what the owner wants, ask them in this terminal instead of guessing.

## The next step

Keep one suggestion current: the next step for the owner. It shows at the top of the assistant's screen and as the first row of the inbox, with one button for its action, and the owner clicks it; you never do the action. Call `suggest` after you plan and whenever the plan changes (a card added, edited, moved, or its status changed under you), and each call replaces the last. `action` is `start` (a card that can start now), `answer` (a card whose agent waits for an answer), `accept` (a card in review) or `release` (a finished chain), always with its `card`. When nothing waits for the owner, say so: `suggest` with a text and no card and no action. A suggestion ends by itself when its action is done or its card moves on; then say the next one.

## Short output

- Do not narrate what you are about to do and do not paste code back.
- Say what you changed in a few lines: which cards, in which chain, in which order.
";

/// The instructions file of a project's planner run: the project's facts, then the protocol.
pub fn generate_planner(facts: &ProjectFacts) -> String {
    let merge = match facts.merge_mode {
        MergeMode::Local => format!(
            "Accepted work is merged into `{}` of the local repository.",
            facts.work_branch
        ),
        MergeMode::Github => format!(
            "Accepted work is pushed, opened as a pull request against `{}` and merged on \
             GitHub (`{}`) when its checks pass.",
            facts.work_branch,
            facts.merge_method.flag()
        ),
    };
    let checks = match &facts.check_command {
        Some(c) => format!(
            "The project's check command is `{c}`. Card agents run it before they hand in; \
             put it in acceptance items where it applies."
        ),
        None => "The project names no check command; give acceptance items another way to be \
                 tested."
            .to_owned(),
    };
    let stale = facts.base_behind.as_ref().map_or_else(String::new, |b| {
        format!(
            "\n\nThe local `{}` is {} commit{} behind `{}`: the code you read may be out of \
             date.",
            b.base,
            b.behind,
            if b.behind == 1 { "" } else { "s" },
            b.upstream
        )
    });
    format!(
        "# Toknila: planner of {key}, {name}\n\n\
         You are the planner of project {key} for Toknila. The repository's own `CLAUDE.md` and \
         `AGENTS.md` still apply and win where they differ; this file only adds how to work with \
         Toknila.\n\n\
         ## The project\n\n{merge}\n\n{checks}{stale}\n\n{protocol}",
        key = facts.key,
        name = facts.name,
        protocol = PLANNER_PROTOCOL,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AgentRole, instructions::RunBrief};

    fn facts() -> ProjectFacts {
        ProjectFacts {
            key: "DEMO".to_owned(),
            name: "Demo".to_owned(),
            work_branch: "develop".to_owned(),
            merge_mode: MergeMode::Local,
            merge_method: MergeMethod::Merge,
            check_command: Some("cargo xtask verify".to_owned()),
            base_behind: None,
        }
    }

    #[test]
    fn the_generated_file_is_pinned() {
        let text = generate_planner(&facts());
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/instructions/snapshots/planner.md");
        if std::env::var_os("TOKNILA_UPDATE_SNAPSHOTS").is_some() {
            std::fs::write(&path, &text).expect("write snapshot");
            return;
        }
        let want =
            std::fs::read_to_string(&path).expect("missing snapshot; TOKNILA_UPDATE_SNAPSHOTS=1");
        assert_eq!(text, want.replace("\r\n", "\n"));
    }

    #[test]
    fn the_file_names_the_merge_mode_and_the_check_command() {
        let local = generate_planner(&facts());
        assert!(local.contains("merged into `develop` of the local repository"));
        assert!(local.contains("`cargo xtask verify`"));
        let github = generate_planner(&ProjectFacts {
            merge_mode: MergeMode::Github,
            merge_method: MergeMethod::Squash,
            check_command: None,
            ..facts()
        });
        assert!(github.contains("pull request against `develop`"));
        assert!(github.contains("--squash"));
        assert!(github.contains("names no check command"));
    }

    #[test]
    fn a_base_behind_its_upstream_is_said_in_one_line() {
        let behind = |n| {
            generate_planner(&ProjectFacts {
                base_behind: Some(BaseBehind {
                    base: "develop".to_owned(),
                    upstream: "origin/develop".to_owned(),
                    behind: n,
                    can_fast_forward: true,
                }),
                ..facts()
            })
        };
        let line = "The local `develop` is 133 commits behind `origin/develop`: the code you \
                    read may be out of date.";
        assert!(
            behind(133).contains(&format!("{line}\n\n## Your role")),
            "{}",
            behind(133)
        );
        assert!(behind(1).contains("is 1 commit behind"));
        assert!(!generate_planner(&facts()).contains("behind"));
    }

    #[test]
    fn the_protocol_names_every_planner_tool_and_the_rules() {
        for tool in [
            "show_plan",
            "show_design",
            "new_chain",
            "add_card",
            "edit_card",
            "show_card",
            "suggest",
            "add_dependency",
            "remove_dependency",
            "move_card",
            "open_thread",
            "thread_message",
            "thread_version",
            "never start",
            "nothing waits for the owner",
            "order of cards in a chain is the priority",
        ] {
            assert!(PLANNER_PROTOCOL.contains(tool), "{tool}");
        }
    }

    fn kick_brief(has_designs: bool) -> RunBrief<'static> {
        RunBrief {
            key: "DEMO",
            title: "Demo",
            prompt: "",
            acceptance: &[],
            check_command: None,
            earlier: &[],
            waiting: &[],
            returned: None,
            design_first: false,
            designs: &[],
            has_designs,
        }
    }

    #[test]
    fn the_planner_is_kicked_off_to_look_at_the_plan_not_to_start_a_card() {
        let text = crate::instructions::kickoff(AgentRole::Planner, &kick_brief(false));
        assert!(text.contains("planner of project DEMO"));
        assert!(text.contains("show_plan"));
        assert!(text.contains("`suggest`"));
        assert!(!text.contains("finished"));
        assert!(
            !text.contains("design"),
            "no designs, no word about them: {text}"
        );
    }

    #[test]
    fn a_project_with_designs_kicks_the_planner_off_to_look_at_them_first_and_name_them() {
        let text = crate::instructions::kickoff(AgentRole::Planner, &kick_brief(true));
        let at = |s: &str| text.find(s).unwrap_or_else(|| panic!("{s}: {text}"));
        assert!(at("Read every design file") < at("what screens and flows you see"));
        assert!(at("what screens and flows you see") < at("propose the cards"));
        assert!(at("propose the cards") < at("name the designs each card"));
        assert!(text.contains("Do not start any card"));
        let file = generate_planner(&facts());
        assert!(file.contains("`show_design`"));
        assert!(file.contains("Every card you add names the designs it comes from"));
    }
}
