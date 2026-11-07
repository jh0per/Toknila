# [decision] Web trackers exchange new work with Toknila; the plan stays local

**Status:** decided 2026-11-08, not built yet. Toknila is a tool for local development first; the
cards for this are in `plans/trackers.toml`, after the move-in work.

**What:** A project may connect one web tracker (Jira, Cloud or Data Center, or a Trello board).
The plan lives in Toknila; the tracker exchanges *new work* with it, in two independent
directions, each switched on per project:

- **Pull, new tasks from the web.** Tasks that appear in the tracker and match the project's
  filter (a JQL query or an epic in Jira; a board and its lists in Trello) arrive as wildcard cards
  ([0002](0002-wildcard-cards.md)): proposed, without edges, listed under "needs a human". A person
  approves (the card joins a chain at the end, or where they put it) or rejects. Nothing from the
  web ever starts work by itself.
- **Push, new cards to the web.** A card created in Toknila (by a person, an agent's approved
  wildcard, an import or the planner) is created in the tracker: title, prompt as the description,
  acceptance and subtasks as a checklist (Jira sub-tasks, a Trello checklist). The card shows the
  tracker key and link.

Every exchanged item has one origin, recorded in a link table (`card ↔ tracker, remote id, key,
url, origin, last seen`). An item never travels back to where it came from, so pull and push cannot
loop. After creation each side owns its own copy: later edits are not synchronised (see Later).

**How it works:**

- Toknila runs on the person's machine, so tracker webhooks (which need a public address) are not
  used. The daemon polls on an interval (default one minute) for items changed since the last poll,
  and keeps working offline; pushes wait in an outbox and go out when the tracker is reachable.
- Pushes are after-commit external effects of the use case that created the card, as ships and
  merges are today. Each push first looks for its own marker in the tracker (a label or property
  holding the card id) so a retry after a crash never creates a duplicate.
- Credentials (Jira: email and API token, or a personal access token on Data Center; Trello: key and
  token) are kept in the operating system's credential store, never in the data home, the
  repository, logs or the journal.
- Layers: core holds the pure rules (what a remote item becomes as a card draft and back, what is
  new, what must not travel back); the app gets a `Tracker` port and the outbox; the daemon holds
  one adapter per tracker behind that port, the poller and the link table. A fake tracker adapter
  drives the tests.
- The window: project settings get "Tracker" (connect, filter, pull on/off, push on/off, last
  poll, errors); a card shows its tracker key; the Inbox shows pulled tasks to approve.

**Later, not decided now:** mirroring Toknila's statuses (in progress, in review with the PR, done)
and agent reports to the tracker; following edits of exchanged items in either direction, with a
three-way comparison against the last synced state and conflicts asked in the Inbox; release
milestones as Jira fix versions. The link table and the outbox are what these need, so they can be
added without changing this design.

**Why:** The owner plans in Toknila and uses it for local development now, with the prospect of
taking tasks from the web and showing new cards there. Exchanging only new work keeps one owner per
item and needs no conflict handling; it is useful on its own and is the base that two-way
synchronisation would need anyway. Reusing wildcard cards means pulled work gets the same approval
step as any other unplanned work.

**How to verify (when built):** core tests for the mapping and the no-echo rule; app tests with the
fake tracker for pull, push, outbox retry and the duplicate check; adapter tests against recorded
responses; an e2e test for the settings page and an approved pulled task. Check each tracker call
against its official API documentation when building, not against this note.

**Alternatives:**

- Full two-way synchronisation from the start: rejected for now, it needs field ownership and
  conflict handling before it gives anything, and the owner does not plan in a tracker today.
- The tracker as the source of the plan, Toknila only executing: rejected, the chain, its order and
  dependencies, threads and wildcards are Toknila's model; trackers have no dependency graph
  (Trello) or one that does not drive work (Jira).
- Let the planner agent copy work between Toknila and the tracker through a tracker MCP server:
  rejected as the mechanism, it is not deterministic and spends tokens on bookkeeping. The planner
  may still read a tracker to help plan.
- Webhooks: rejected, they need a public address for a local app; polling is enough at this scale.
