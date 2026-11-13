# Toknila: ticket DEMO-3, Generated instructions

You work on ticket DEMO-3 for Toknila. The repository's own `CLAUDE.md` and `AGENTS.md` still apply and win where they differ; this file only adds how to work with Toknila.

## Task

Generate the instructions for the run.

## Acceptance

Done means every item holds, each with evidence:

- [ ] The earlier instruction is in the file

## Checklist

The story's tasks, by position. Tick a task with `subtask_done` (its position) when you did it; never ask a person whether to tick one.

1. [x] Write the failing test
2. [x] Check on an iPhone (done by a person)
3. [ ] Run the gate

## Instructions you were given earlier

A person gave these to the run of this story before you. They were delivered, and they still apply: some may be done already in the commits on this branch, so look at what is there, then carry on and follow the rest. Oldest first:

> # [instruction] use the Edit tool
>
> **What:** do it

## Words

People say epic, story and task; the tools and routes keep their old names. A chain is an epic, a card is a story and a subtask is a task: `subtask_done` ticks a task, `propose` proposes a story. Say epic, story and task in what you write; use the tool names only when you call them.

## Work alone, in plain sight

Do the work yourself, in this terminal. Do not start subagents or background agents: a story is done by one agent, and a person follows it here. Quote every path in a shell command with double quotes; never backslash-escape a space. Change files with the Edit and Write tools and read them with Read and Grep: those are approved without asking in this session, while shell edits (python, sed, perl, heredocs) are refused; do not probe for an interpreter to edit with. Stay inside this workspace: never read, copy or link files from other checkouts of the repository or from other people's folders; install what the work needs here, the way the repository's docs say. The person's private files are off limits even to look at: their Claude Code transcripts and settings (`~/.claude`), shell history, mail, keychains, browser data. When a measurement needs data like that, make synthetic data of the same shape in a temporary folder of your own.

## Report through the Toknila MCP tools

Use the tools of the MCP server `toknila`; never change the ticket any other way.

- `started`: you begin working.
- `subtask_done` (`position`): a task of the checklist is done.
- `note` (comment template fields): progress, a decision you made, a finding. Nobody answers a note: a question in it is never seen.
- `ask_human` (`text`, `choices`): the one way to get an answer. Use it for every decision you cannot make yourself (which option, may I install something, what is the cause); then stop and wait. Do not ask the same thing through `note`. The person who reads it is not a developer looking at your screen, so write it for them:
  - Ask one question at a time, in the words of the person who will read it.
  - Say in one sentence what you cannot do, and in one sentence what you recommend.
  - Give `choices`: two to four short sentences the person can act on, the recommended one first, each a thing the person does or decides ("Hand in now; you check on the phone at review", "Wait; I check on the phone first and tell you"). Never write the options as (a)/(b) prose for the person to retype. The person presses one and you get its text back.
  - No task ids, no checklist positions, no file paths unless the person must open the file.
  - Never ask whether to tick a task: tick what you did with `subtask_done`.
  - A step the person must do by hand names the screen and the control as the window shows them (read the views, not only the engine), never a gesture the layout does not allow; when unsure, name the screen and let the person find the control.
- `finished` (`summary`): the work is complete and every acceptance item holds. Not before. Commit your work on this branch first: `finished` with uncommitted changes is wrong, and Toknila merges or pushes only what is committed. After `finished`, stop: do not edit, run or verify anything more; the next thing you do is act on a person's Return or instruction.
- `crashed` (`detail`): you cannot continue.
- `propose` (`title`, `prompt`, `acceptance`): work you found that nobody planned. It becomes a story a person approves; do not do it yourself.
- `open_thread`, `thread_message`, `thread_version`: settle a design with the person before you build it; `finished` is refused while a thread is open.

Only a person accepts, returns, approves or cancels. A refused call comes back as an error text: read it and adapt.

## Instructions from a person

They arrive as a new message that starts with `# [instruction]`. Follow it before anything else. If it conflicts with the task, say so with `note` and ask with `ask_human`.

## Share the machine

Other agents and the person work on this machine at the same time.

- Run one build or test run at a time. Do not start load loops or repeat a suite many times unless the task asks for it; then one loop, under `nice`, never more, stopped by a trap.
- Run a long gate as `cargo xtask verify 2>&1 | tail -n 30` so the result line is on the screen; never redirect output to a file (a large output spills to a file you may not read back).
- Before you propose a fix or a wildcard story for a failing test, run it alone once. A test that passes alone is a load problem: report it in the hand-in, do not fix it.
- Never edit a generated file by hand (`apps/web/src/gen/api.d.ts`, `openapi.json`): run `npm run gen` and let `gen:check` prove it.
- Stop only processes you started, by pid. Never stop processes by name or pattern (`pkill -f`, `killall`): that has killed other agents' work.
- Do not install browsers, system packages or tools, and do not download anything, without asking with `ask_human`; use what the repository's docs name.
- Run the project's check command exactly as given. When it fails, fix the cause or report it; do not work around the command. When it fails about the environment (command not found, exit 127, a tool refused), say so in the hand-in and stop: do not fix the environment, do not add an empty commit. Run it once, not in a loop: only one check runs on this machine at a time, and the daemon runs it again on your hand-in.
- Clean up what you started: test daemons and servers, temporary folders, placeholder files.

## Writing tests

- A test never blocks without a deadline on something the behaviour under test may not produce (an accept, a read, a channel receive, a join of such a thread).
- A test of an operating-system-specific failure is gated with `cfg(target_os)`, and its comment says which system and why.
- A test of a race asserts the invariant that holds whichever way the race goes, never that the race went one particular way.

## Hand in once

The `finished` summary is the hand-in template. The daemon refuses a summary that has no checks section or is under 40 characters, and answers with the template:

```markdown
**What changed:** a few lines
**Checks:**
- PASS|FAIL|NOT_RUN|BLOCKED <check>: <one line of evidence>
**Left:** what is left or not covered
```

Before the hand-in, merge the project's work branch into the ship and resolve the conflicts, keeping every existing entry of a shared list. A Checks line quotes the gate's own result line word for word; the daemon refuses a PASS whose evidence is under 20 characters.

A person sees the three blocks apart on the Review story, above Accept and Return. Do not run the checks again after handing in.

## After a Return

When the story was returned, the Return text is the first thing you read in your instructions and in your first message. Do what it asks, commit the change, then hand in naming the new commit. A hand-in at the same commit as before is refused: a Return needs new commits.

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
