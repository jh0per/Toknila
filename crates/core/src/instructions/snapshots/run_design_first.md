# Toknila: ticket DEMO-3, Generated instructions

You work on ticket DEMO-3 for Toknila. The repository's own `CLAUDE.md` and `AGENTS.md` still apply and win where they differ; this file only adds how to work with Toknila.

## Task

Generate the instructions for the run.

## Acceptance

Done means every item holds, each with evidence:

- [ ] A design is approved before the code

## Design first

The person asked to see and approve a design before you write any code. Before you touch the code: call `open_thread` with kind `design` (kind `flow` or `architecture` when that fits the card better), then post a first version with `thread_version` that the person can judge: a Mermaid diagram, or the screen described as its states and layout. Then wait for the person's approval or request for changes; answer a request with a new version. Write code only after the person approved a version. `finished` is refused until one is approved.

## Work alone, in plain sight

Do the work yourself, in this terminal. Do not start subagents or background agents: a card is done by one agent, and a person follows it here. Quote every path in a shell command with double quotes; never backslash-escape a space. Change files with the Edit and Write tools and read them with Read and Grep: those are approved without asking in this session, while shell edits (python, sed, perl, heredocs) stop for a permission answer every time; do not probe for an interpreter to edit with. Stay inside this workspace: never read, copy or link files from other checkouts of the repository or from other people's folders; install what the work needs here, the way the repository's docs say. The person's private files are off limits even to look at: their Claude Code transcripts and settings (`~/.claude`), shell history, mail, keychains, browser data. When a measurement needs data like that, make synthetic data of the same shape in a temporary folder of your own.

## Report through the Toknila MCP tools

Use the tools of the MCP server `toknila`; never change the ticket any other way.

- `started`: you begin working.
- `subtask_done` (`position`): a checklist item is done.
- `note` (comment template fields): progress, a decision you made, a finding. Nobody answers a note: a question in it is never seen.
- `ask_human` (`text`): the one way to get an answer. Use it for every decision you cannot make yourself (which option, may I install something, what is the cause); then stop and wait. Do not ask the same thing through `note`.
- `finished` (`summary`): the work is complete and every acceptance item holds. Not before. Commit your work on this branch first: `finished` with uncommitted changes is wrong, and Toknila merges or pushes only what is committed. After `finished`, stop: do not edit, run or verify anything more; the next thing you do is act on a person's Return or instruction.
- `crashed` (`detail`): you cannot continue.
- `propose` (`title`, `prompt`, `acceptance`): work you found that nobody planned. It becomes a card a person approves; do not do it yourself.
- `open_thread`, `thread_message`, `thread_version`: settle a design with the person before you build it; `finished` is refused while a thread is open.

Only a person accepts, returns, approves or cancels. A refused call comes back as an error text: read it and adapt.

## Instructions from a person

They arrive as a new message that starts with `# [instruction]`. Follow it before anything else. If it conflicts with the task, say so with `note` and ask with `ask_human`.

## Share the machine

Other agents and the person work on this machine at the same time.

- Run one build or test run at a time. Do not start load loops or repeat a suite many times unless the task asks for it; then use a few workers under `nice`, with a trap that stops them.
- Stop only processes you started, by pid. Never stop processes by name or pattern (`pkill -f`, `killall`): that has killed other agents' work.
- Do not install browsers, system packages or tools, and do not download anything, without asking with `ask_human`; use what the repository's docs name.
- Run the project's check command exactly as given. When it fails, fix the cause or report it; do not work around the command. Run it once, not in a loop: only one check runs on this machine at a time, and the daemon runs it again on your hand-in.
- Clean up what you started: test daemons and servers, temporary folders, placeholder files.

## Hand in once

The `finished` summary says what changed, gives each check as PASS, FAIL, NOT_RUN or BLOCKED with one line of evidence, and names what is left or not covered. Do not run the checks again after handing in.

## After a Return

When the card was returned, the Return text is the first thing you read in your instructions and in your first message. Do what it asks, commit the change, then hand in naming the new commit. A hand-in at the same commit as before is refused: a Return needs new commits.

## Comment template

Reports, notes and decisions use this shape; leave out empty fields:

```markdown
# [instruction | report | decision] <one-line gist>

**What:** what to do or what was done
**Why:** context and reason, one or two sentences
**How to verify:** criterion or command
**Alternatives:** what was rejected and why (decisions)
**Consequences / risks:** what changes next (decisions)
```

## Short output

- Do not narrate what you are about to do and do not paste code back.
- Report each check as PASS, FAIL, NOT_RUN or BLOCKED, with one line of evidence.
- Say what the tests do not cover, and mark local-only evidence as such.
- Keep every note and summary to the template fields, a few lines each.
