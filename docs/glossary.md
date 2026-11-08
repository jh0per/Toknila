# Glossary: the words people see and the words the code uses

The window and `tok`'s help speak to a person who is not a programmer and is guided by the
project's planner. The code, the API and the journal keep their precise names. This table is the
bridge: use the left word wherever a person reads, the right word wherever code, API or logs are
read. A change that adds a screen or a help line checks it against this table.

| Say (window, `tok` help) | Internal word | Notes |
| --- | --- | --- |
| workspace | ship (git worktree) | The copy of the project a card's agent works in. Path and branch only under "Technical details" of a card. |
| main branch | base branch, `work_branch` | Agents start from it and approved work lands on it. |
| test command (optional) | check command, `check_command` | Hint: "agents run it before they hand in". |
| When a card is approved: merge into the main branch / open a pull request on GitHub | merge mode `local` / `github` | Words for the two values; the stored values do not change. |
| pull request (tab) | PR | The tab shows only for a project that opens pull requests, or a card that has one. |
| start cards automatically | autostart | Chain switch. `tok chain set --autostart` keeps its flag. |
| agent | profile | In Settings, "Agents" lists profiles; "default agent" is the project's default profile. |
| simulated agent | `sim` profile | Named in the window only in a development build, or where a project uses it. |
| model | model alias | Under a project's Advanced; "leave empty to use your Claude Code default". |
| assistant | planner | The project's main agent: "Plan with the assistant" is the first entry of a project in the sidebar. `tok planner` and the `/v1/planner` routes keep their name. |
| Chat, Events (tabs of a card) | comments, questions and answers; the journal | Chat shows what the person and the card's agent said to each other; Events prints one sentence per journal entry with the raw event folded under "Raw event". |
| card | ticket | Already the word in the window. |
| chain | chain | Same word both sides. |
| Back to work | (settings screen) | The settings replace the sidebar; Ctrl+, (Cmd+, on a Mac) opens them. |
| Technical details | worktree path, ship branch, profile name | A collapsed part of a card's Details. |
| Where things live | home, ships and profiles folders | Under a project's Advanced, folded. |

Rules of thumb:

- A path, a branch name or a profile name is shown only where the person needs to copy it, and
  then behind a "Technical details" or "Where things live" fold.
- A setting that most people never change goes under Advanced.
- A placeholder is never a default: a detected value is shown as a real value, and the field says
  where it came from.
- Core identifiers (`ship`, `base_branch`, event and status names) and the API do not change to
  match this table; only what a person reads does.
