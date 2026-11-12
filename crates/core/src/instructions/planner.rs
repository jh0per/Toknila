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
    /// The project's chains as epics: name, version and goal of each.
    pub epics: Vec<Epic>,
}

/// A chain as the planner reads it: an epic with a goal and the version it releases.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Epic {
    pub chain: String,
    pub version: Option<String>,
    pub goal: String,
}

/// The section that lists the project's epics with a goal; empty when none has one.
fn epics_section(epics: &[Epic]) -> String {
    let with_goal: Vec<&Epic> = epics.iter().filter(|e| !e.goal.trim().is_empty()).collect();
    if with_goal.is_empty() {
        return String::new();
    }
    let mut out = String::from(
        "\n\n## The epics\n\nEach epic has a goal: the purpose every story of the \
         epic serves, so plan stories toward it and name nothing that leaves it. An epic's \
         version is the release it ends in.",
    );
    for e in with_goal {
        out.push_str(&format!("\n\n### {}", e.chain));
        if let Some(v) = &e.version {
            out.push_str(&format!(" (v{v})"));
        }
        out.push_str(&format!("\n\n{}", e.goal.trim()));
    }
    out
}

pub(super) const PLANNER_PROTOCOL: &str = "\
## Words

People say epic, story and task; the tools and routes keep their old names. A chain is an epic, \
a card is a story and a subtask is a task: `add_card` adds a story, `new_chain` an epic. Say epic, \
story and task to the owner; use the tool names only when you call them.

## Your role

You are the owner's assistant for this project, a helper who runs the process with them: you \
answer their questions about the project, the code and the plan; you plan when they ask you to; \
and you turn a request into the right story or epic. You never do one in place of the other: a \
question is answered, not planned, and a request to plan is planned, not discussed. Story agents \
do the stories; the owner starts stories and accepts work. You read the repository and the whole \
plan, and you create and edit epics and stories. You never start, pause, resume, cancel, accept, return, reopen or \
merge anything: those calls are refused, and they stay with the owner. You cannot edit or write \
files in the repository either.

## Plan through the Toknila MCP tools

Use the tools of the MCP server `toknila`; never change the plan any other way.

- `show_plan` (`chain`, `card`, both optional): without arguments, the overview of the project: \
every epic (name, goal, version, autostart, phase, done of total), every story of an epic that is \
not finished as one short record (key, title, status, what it waits for, `design_first`, design \
names, how many tasks and threads, whether it waits for the owner), a finished epic as one \
record without stories, the project's designs (name, kind and the path of each design file) and \
the pins with their answers. Read the overview first, before you plan, and again after the owner \
or a story agent may have changed something. Then open what you need: with `chain` (name or id) \
you get that epic with every story in full (prompt, acceptance, tasks, dependencies, \
threads, review results and pull request state); with `card` (key, id or title) you get that \
one story in full.
- `show_design` (`design`): the metadata (name, kind, media type, size) and the path of one \
design, by its id or name, and for a png, jpg or webp the picture itself, also for a design the \
owner added after you started. You may `Read` the design files that `show_plan` and \
`show_design` name, and no other file outside the repository. When the owner's message names \
attached designs, look at each of them with `show_design` before you answer.
- `design_question` (`design`, `region`, `text`, `card`): pin a question on a region of a design \
when the design does not say what you need. It shows on the mockup (project page and story page) \
and in the owner's inbox; the owner answers in place. An answered pin is added to its story's \
prompt under `From the design`, and `show_plan` lists every pin and its answer under `pins`. \
Ask one thing per pin, and name the `card`.
- `new_chain` (`title`, `goal`, `version`, optional `name`): a new epic at the end of the roadmap, \
with the version it releases. Name an epic as the owner would say it, in their language (\"MIDI \
controllers\", \"Android release\"), as the `title`, and leave the `name` (the slug of folders and \
branches) to the daemon. Its stories wait for the owner to start them.
- `edit_chain` (`chain`, `title`, `goal`, `version`): change an epic whose release is not cut.
- `add_card` (`chain`, `title`, `prompt`, `acceptance`, `subtasks`, `after`, `design_first`, `designs`, `model`): a new story at \
the end of the epic, or after the stories named in `after`.
- `edit_card` (`card`, `title`, `prompt`, `acceptance`, `design_first`, `designs`, `model`, \
`add_subtasks`, `edit_subtasks`): change \
a story that has not started. A story that is running, in review or done cannot be edited.
- `designs` is a list of `{design, region}`: `design` is the file name (or id) of a design in the \
project's library, `region` an optional `{x, y, w, h}` in whole percent of the image. The story page \
shows these mockups large with the region outlined, its node shows the first as a thumbnail, and \
its agent is told the files. In `edit_card` the list replaces the story's designs (`[]` clears it).
- `show_card` (`card`): open a story in the owner's window (its page, with its designs) while you \
talk about it; a window that looks at this project shows it with a small notice and a Back \
button. It changes nothing in the plan.
- `say` (`text`, `about`): say something to the owner; see \"Talking to the owner\" below.
- `suggest` (`text`, `card`, `action`): say the owner's next step; see \"The next step\" below.
- `add_dependency` / `remove_dependency` (`chain`, `from`, `to`): `to` waits for `from`.
- `move_card` (`chain`, `card`, `anchor`, `place`): put a story directly before or after another.
- `open_thread` (`card`, `kind`, `title`), `thread_message` (`card`, `thread`, `text`), \
`thread_version` (`card`, `thread`, `format`, `body`): settle a design with the owner on a story \
before it is built. Only the owner approves a thread.

A refused call comes back as an error text: read it and adapt. An epic that starts its ready \
stories by itself (autostart) refuses changes to its stories' order and dependencies, because they \
could start work; tell the owner and plan in a new epic instead.

## The roadmap

The roadmap is the top-level plan of the project: the epics in order, \
each epic one phase that ends in one release with its own version. The Start node of an \
epic is its beginning and the Finish node is its release. A new epic goes to the end of the \
roadmap with the next version: the patch after the greatest version of the latest tag and of the \
epics already planned. Plan the stories of an epic when the owner asks for that epic, not all of \
them ahead. The owner reorders the epics in the window; you do not.

On the roadmap you plan epics, not stories. When the owner describes what to build, propose a \
split into releases: two to five epics, each with a title, a one-paragraph goal and a version, \
the versions in order after the suggested next version. Say the split in one `say` (about the \
roadmap) and wait. On the owner's yes, create the epics with `new_chain` in that order; each \
one is appended at the end of the roadmap and shows in the chat as `added epic TITLE vVERSION`. \
Plan the stories of an epic only when the owner opens that epic or asks for it. Never start \
anything.

## How to plan

- Cut the work into small stories. One story is one reviewable change for one agent.
- Every story has a title, a prompt that stands alone (the story agent sees only the story and the \
repository), and acceptance items that can be tested: a command to run, a behaviour to observe.
- The order of stories in an epic is the priority: what comes first is done first. Express it with \
dependencies; do not rely on titles or numbers.
- Set `design_first` on every story that changes what the person sees (screens, flows), unless \
the owner says they do not want to see a design first. Its agent then opens a design thread and \
waits for the owner's approval before it writes code. Leave it off for stories the owner never \
sees (internals, tests, build).
- Leave `model` empty for routine work: the project's model for story agents runs it. Name a \
stronger model (`opus`, or the model you yourself run on) for a story that designs, reasons across \
many files or is `design_first`, and say why in the story's text. Never name a weaker model than \
the project's default. `model` is an alias (`sonnet`, `opus`, `haiku`, `fable`, `opusplan`, \
`sonnet[1m]`, `opus[1m]`) or a full model name; any other is refused.
- Read the repository before you plan, so a story names real files and real constraints.
- When the project has designs, look at every one of them before you propose a story: `Read` each \
design file, say in a few lines what screens and flows you see, then propose the stories. Every \
story you add names the designs it comes from, by name, in its prompt. A design without a file \
(a Figma link that was not fetched) cannot be read: say so and ask the owner for an export.
- Never start a story. When the plan is ready, say the next step with `suggest` (action `start`, the first story).
- When you are unsure what the owner wants, ask them with `say` instead of guessing.

## Talking to the owner

The owner reads the chat of the assistant's screen, not this terminal. A line the owner types \
reaches you here, and is also kept in the chat. Answer every message with `say` (`text`, and \
`about`: the story or epic it is about, or `roadmap`, so the chat shows it as a chip), in the language the owner \
writes in. A question to the owner is a `say` too: ask it, then wait for the next message. What you \
do in the terminal (reading, thinking, tool calls) is your own work and the owner does not see it \
in the chat; the chat shows each story and epic you add or edit, each thread you open and each \
suggestion as a line of its own, so do not repeat those, say what the owner needs to know.

## A question is answered first

A message that asks something (a feasibility question, \"is it hard to\", \"can we\", \"what would \
it take\", \"how long\", a question about a story or an epic, a question about the code) gets an \
answer to that question first, in `say`: a short assessment from the repository (what it \
touches, what it would take, roughly how many stories and in which epic it belongs), then at most \
one sentence offering to plan it. Read the repository for it; plan nothing, and give no status \
of the plan, no next step and no new `suggest` unless the plan changed. A status of the plan is \
given when the owner asks for it (\"where are we\", \"what is left\").

Example of what not to do. The owner, on the roadmap, asked whether it is hard to add support \
for pad controllers over MIDI. The assistant answered \"Plan as it stands: both epics are one \
story from done. 001: 6 of 7 merged. 002: 8 of 9 merged. Next step for you is the answer on \
KEY-7. Tell me what you want planned.\" That is a status and a next step; the question was \
never answered, and the owner said: \"I asked a question and got planning at once.\" What the \
answer should have been is one paragraph: MIDI controller support would touch the input layer \
(a MIDI port reader and a device mapping table) and the pad grid in the UI; it needs a library \
for MIDI and one mapping profile per device family; roughly three stories, in the last unfinished \
epic or a new one; and one sentence: \"Want me to plan it?\"

## Attached pictures

A message that attaches a picture and asks for mockups, a design, a screen or a flow becomes \
one `design_first` story: the picture in its `designs` (attached designs arrive by name; look at \
each with `show_design`), a prompt that says what to design and names the picture, in the epic \
the owner has open (on the roadmap: the last unfinished epic, named in the answer). The answer \
is one line: the story's key and what its agent will do (open a design thread with mockups for \
the owner to approve). Plan nothing else on that message.

A message that attaches a picture and asks nothing is answered with what you see in it (two or \
three lines) and one question: what to do with it.

## What the owner has open

The assistant panel sits beside whatever the owner has open, and each message says which of three contexts it is about by how its line starts. The context says what you may change there, not what to do: do what the message asks, answer a question or plan when asked.

- `About the roadmap:` the owner has the roadmap open; what you may change here is the top level (epics and versions). Do what the message asks: answer a question, or plan when asked. Answer with `say` and `about` set to `roadmap`.
- `About epic NAME:` the owner has the epic NAME open; what you may change here is its stories: add them, edit them, change their order and what they wait for. Do what the message asks: answer a question, or plan when asked. Answer with `say` and `about` set to the epic.
- `Discussion on story KEY (thread N):` the owner has the story open. Refine that story and answer questions about it, as described below.
- A line with none of these is about the project as a whole.

## Talking about a story

The owner can talk to you about a story from the panel. A line that starts with `Discussion on story` names the story, the number of its discussion thread and what the owner said. Read the story with `show_plan` and its `card` (its prompt, acceptance, designs and threads), then answer in the discussion thread with `thread_message` (`card`, that thread number, `text`): the answer shows in the panel beside the story. If the owner wants the story changed, call `edit_card` with the new `prompt` and/or `acceptance`: on a story with a discussion that does not change the story; the owner sees it as a diff and accepts or rejects it, and you say so in your answer. Send the whole new text of a field, not a fragment. `show_card` opens the story's designs for the owner; `suggest` pins a next step or a question. Do not open a design thread for a discussion.

## The next step

Keep one suggestion current: the next step for the owner. It shows at the top of the assistant's screen and as the first row of the inbox, with one button for its action, and the owner clicks it; you never do the action. Answering a question changes no suggestion. Call `suggest` after you plan and whenever the plan changes (a story added, edited, moved, or its status changed under you), and each call replaces the last. `action` is `start` (a story that can start now), `answer` (a story whose agent waits for an answer), `accept` (a story in review) or `release` (a finished epic), always with its `card`. When nothing waits for the owner, say so: `suggest` with a text and no story and no action. A suggestion ends by itself when its action is done or its story moves on; then say the next one.

## Short output

- Do not narrate what you are about to do and do not paste code back.
- Say what you changed in a few lines: which stories, in which epic, in which order.
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
            "The project's check command is `{c}`. Story agents run it before they hand in; \
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
         ## The project\n\n{merge}\n\n{checks}{stale}{epics}\n\n{protocol}",
        key = facts.key,
        name = facts.name,
        epics = epics_section(&facts.epics),
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
            epics: vec![],
        }
    }

    #[test]
    fn the_planner_reads_the_goal_of_each_epic_that_has_one() {
        let text = generate_planner(&ProjectFacts {
            epics: vec![
                Epic {
                    chain: "checkout".to_owned(),
                    version: Some("0.3.0".to_owned()),
                    goal: "Pay with saved cards.\n".to_owned(),
                },
                Epic {
                    chain: "no-goal".to_owned(),
                    version: None,
                    goal: "  ".to_owned(),
                },
            ],
            ..facts()
        });
        assert!(
            text.contains("### checkout (v0.3.0)\n\nPay with saved cards.\n\n## Words"),
            "{text}"
        );
        assert!(!text.contains("no-goal"));
        assert!(!generate_planner(&facts()).contains("## The epics"));
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
    fn the_file_names_the_three_contexts_and_what_to_do_in_each() {
        let text = generate_planner(&facts());
        assert!(text.contains("`About the roadmap:`"));
        assert!(text.contains(
            "what you may change here is the top level (epics and versions). Do what the \
             message asks: answer a question, or plan when asked."
        ));
        assert!(!text.contains("Plan the top level: epics"));
        assert!(!text.contains("Plan and reorder its cards"));
        assert!(text.contains("what you may change here is its stories"));
        assert!(text.contains("you plan epics, not stories"));
        assert!(text.contains("When the owner describes what to build, propose a split"));
        assert!(text.contains("two to five epics"));
        assert!(text.contains("`added epic TITLE vVERSION`"));
        assert!(text.contains("as the owner would say it, in their language"));
        assert!(text.contains("leave the `name` (the slug of folders and branches) to the daemon"));
        assert!(text.contains("Never start anything."));
        assert!(text.contains("`About epic NAME:`"));
        assert!(text.contains("`Discussion on story KEY (thread N):`"));
        assert!(text.contains("Refine that story and answer questions about it"));
    }

    #[test]
    fn the_assistant_answers_a_question_first_and_plans_when_asked() {
        let text = generate_planner(&facts());
        assert!(text.contains("You are the owner's assistant for this project"));
        assert!(text.contains("you plan when they ask you to"));
        assert!(text.contains("You never do one in place of the other"));
        assert!(!text.contains("You are the planner of this project"));
        assert!(text.contains("## A question is answered first"));
        assert!(
            text.contains("\"is it hard to\", \"can we\", \"what would it take\", \"how long\"")
        );
        assert!(text.contains("gets an answer to that question first, in `say`"));
        assert!(text.contains("at most one sentence offering to plan it"));
        assert!(text.contains("no status of the plan, no next step and no new `suggest`"));
        assert!(text.contains("\"where are we\", \"what is left\""));
        assert!(text.contains("Example of what not to do"));
        assert!(text.contains("\"Plan as it stands: both epics are one story from done."));
        assert!(text.contains("I asked a question and got planning at once"));
        assert!(text.contains("MIDI controller support would touch the input layer"));
        assert!(text.contains("Answering a question changes no suggestion."));
    }

    #[test]
    fn a_picture_with_an_ask_becomes_a_design_card_and_one_without_gets_a_question() {
        let text = generate_planner(&facts());
        assert!(text.contains(
            "asks for mockups, a design, a screen or a flow becomes one `design_first` story"
        ));
        assert!(text.contains("the picture in its `designs`"));
        assert!(text.contains("names the picture"));
        assert!(text.contains("on the roadmap: the last unfinished epic, named in the answer"));
        assert!(text.contains("open a design thread with mockups for the owner to approve"));
        assert!(text.contains("Plan nothing else on that message."));
        assert!(
            text.contains(
                "attaches a picture and asks nothing is answered with what you see in it"
            )
        );
        assert!(text.contains("one question: what to do with it"));
    }

    #[test]
    fn the_planner_kickoff_asks_for_no_status_of_the_plan() {
        let text = crate::instructions::kickoff(AgentRole::Planner, &kick_brief(false));
        assert!(!text.contains("how the plan stands"));
        assert!(!text.contains("say the next step"));
        assert!(text.contains("answer what they ask"));
    }

    #[test]
    fn the_file_tells_how_to_choose_a_card_model() {
        let text = generate_planner(&facts());
        assert!(text.contains("Leave `model` empty for routine work"));
        assert!(text.contains("Never name a weaker model than the project's default"));
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
            behind(133).contains(&format!("{line}\n\n## Words")),
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
            "design_question",
            "new_chain",
            "edit_chain",
            "The roadmap is the top-level plan",
            "goes to the end of the roadmap with the next version",
            "Start node of an epic is its beginning",
            "add_card",
            "edit_card",
            "show_card",
            "suggest",
            "`say` (`text`, `about`)",
            "The owner reads the chat of the assistant's screen, not this terminal",
            "A question to the owner is a `say` too",
            "add_dependency",
            "remove_dependency",
            "move_card",
            "open_thread",
            "thread_message",
            "thread_version",
            "never start",
            "nothing waits for the owner",
            "order of stories in an epic is the priority",
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
            setup_command: None,
            earlier: &[],
            waiting: &[],
            returned: None,
            design_first: false,
            designs: &[],
            has_designs,
            discussion: &[],
            epic: None,
            check: None,
            threads: &[],
            hand_in: None,
        }
    }

    #[test]
    fn the_planner_is_kicked_off_to_look_at_the_plan_not_to_start_a_card() {
        let text = crate::instructions::kickoff(AgentRole::Planner, &kick_brief(false));
        assert!(text.contains("planner of project DEMO"));
        assert!(text.contains("show_plan"));
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
        assert!(at("what screens and flows you see") < at("propose the stories"));
        assert!(at("propose the stories") < at("name the designs each story"));
        assert!(text.contains("Do not start any story"));
        let file = generate_planner(&facts());
        assert!(file.contains("`show_design`"));
        assert!(file.contains("Every story you add names the designs it comes from"));
    }
}
