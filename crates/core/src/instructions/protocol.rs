//! The ticket-independent text of the generated instructions.

pub(super) const PROTOCOL: &str = "\
## Report through the Toknila MCP tools

Use the tools of the MCP server `toknila`; never change the ticket any other way.

- `started`: you begin working.
- `subtask_done` (`position`): a checklist item is done.
- `note` (comment template fields): progress, a decision, a finding.
- `ask_human` (`text`): you need an answer; then stop and wait.
- `finished` (`summary`): the work is complete and every acceptance item holds. Not before.
- `crashed` (`detail`): you cannot continue.
- `propose` (`title`, `prompt`, `acceptance`): work you found that nobody planned. It becomes a \
card a person approves; do not do it yourself.
- `open_thread`, `thread_message`, `thread_version`: settle a design with the person before you \
build it; `finished` is refused while a thread is open.

Only a person accepts, returns, approves or cancels. A refused call comes back as an error text: \
read it and adapt.

## Instructions from a person

They arrive as a new message that starts with `# [instruction]`. Follow it before anything else. \
If it conflicts with the task, say so with `note` and ask with `ask_human`.

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
";
