# Glossary: the words people see and the words the code uses

The window and `tok`'s help speak to a person who is not a programmer and is guided by the
project's planner. The code, the API and the journal keep their precise names. This table is the
bridge: use the left word wherever a person reads, the right word wherever code, API or logs are
read. A change that adds a screen or a help line checks it against this table.

| Say (window, `tok` help) | Internal word | Notes |
| --- | --- | --- |
| workspace | ship (git worktree) | The copy of the project a story's agent works in. Path and branch only under "Technical details" of a story. |
| main branch | base branch, `work_branch` | Agents start from it and approved work lands on it. |
| test command (optional) | check command, `check_command` | Hint: "agents run it before they hand in". |
| When a story is approved: merge into the main branch / open a pull request on GitHub | merge mode `local` / `github` | Words for the two values; the stored values do not change. |
| pull request (tab) | PR | The tab shows only for a project that opens pull requests, or a story that has one. |
| start stories automatically | autostart | Epic switch. `tok chain set --autostart` keeps its flag. |
| agent | profile | In Settings, "Agents" lists profiles; "default agent" is the project's default profile. |
| simulated agent | `sim` profile | Named in the window only in a development build, or where a project uses it. |
| When the agent wants to run a command: Let Claude decide / Ask me each time / Never ask | command policy `auto` / `ask` / `trust` (`command_policy`), Claude Code permission mode | Under a project's Agents, one line of meaning per choice. New projects start on "Let Claude decide"; projects that existed before keep "Ask me each time". `tok project set --commands`. |
| Sites the reviewer may read | the project's `reviewer_web_domains`: plain host names that become `WebFetch(domain:<host>)` allow rules of the reviewer (a `dontAsk` run) and of nobody else; empty by default | Settings, Review and merge; `tok project show` lists them as `reviewer reads`. |
| Allow always, Commands you allowed always | remembered allow rules (`allowed_commands`, `Bash(cargo test *)`) | A button next to Allow on a question about a command; the list with Remove is under Agents. `tok project show` lists them, `--forget-command` removes one. |
| model | model alias | Under a project's Advanced; "leave empty to use your Claude Code default". |
| assistant | planner | The project's main agent: "Plan with the assistant" is the first entry of a project in the sidebar. `tok planner` and the `/v1/planner` routes keep their name. |
| Chat, Events (tabs of a story) | comments, questions and answers; the journal | Chat shows what the person and the story's agent said to each other; Events prints one sentence per journal entry with the raw event folded under "Raw event". |
| design first ("Show me a design before the code", "Ask for a design") | `design_first` on a story (card), the `ask_design` action, `tok design`, `tok ticket new --design-first` | The agent opens a design thread (a diagram or a mockup) first and writes code only after the person approved a version; `finished` is refused until then. A story that has not started gets the flag; a running story also gets an instruction. |
| hand-in ("What changed", "Checks", "What is left") | the `finished` summary, a `report` comment | The template an agent hands in with: `**What changed:**`, `**Checks:**` (one line each: PASS, FAIL, NOT_RUN or BLOCKED and one line of evidence), `**Left:**`. `finished` without a checks section, or under 40 characters, is refused with the template. The Review story and the inbox row show the three blocks apart, above Accept and Return. |
| epic (was chain) | chain; `/chains`, `tok chain`, `[chain]` in a TOML file | One phase of the roadmap: a goal, its stories and a release at its end. The window, the Guide and the agents' instructions say epic; the code, the API and `tok chain` keep chain. |
| story (was card) | ticket, card; `/cards`, `tok ticket`, `[[ticket]]`, `add_card` | One piece of work an agent does, keyed `TOKNILA-12`. The window and the instructions say story; keys, routes, TOML keys and tool names are unchanged. |
| task (was subtask) | subtask; `subtask_done` | One line of a story's checklist. |
| release, inbox, agent, assistant, roadmap | same | Unchanged on both sides. |
| Back to work | (settings screen) | The settings replace the sidebar; Ctrl+, (Cmd+, on a Mac) opens them. |
| Technical details | worktree path, ship branch, profile name | A collapsed part of a story's Details. |
| Where things live | home, ships and profiles folders | Under a project's Advanced, folded. |

Rules of thumb:

- A path, a branch name or a profile name is shown only where the person needs to copy it, and
  then behind a "Technical details" or "Where things live" fold.
- A setting that most people never change goes under Advanced.
- A placeholder is never a default: a detected value is shown as a real value, and the field says
  where it came from.
- Core identifiers (`ship`, `base_branch`, event and status names) and the API do not change to
  match this table; only what a person reads does.
