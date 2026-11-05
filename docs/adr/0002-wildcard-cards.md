# [decision] Unplanned work becomes a wildcard card that a person approves

**What:** Work nobody planned that shows up during a ticket ("Login needs a token refresh
endpoint first") becomes a wildcard card: a ticket in the new status `proposed` with a mark
(`wildcard`: `from`, `proposed_by`, pending `after` / `before`, `approved`). An agent proposes one
with the agent event `propose`, a person with `POST /v1/chains/{chain}/wildcards`. Until a person
decides, the card has no edges, so it never starts, changes no other ticket's readiness and does
not count in progress; it is listed under "needs a human" as "awaiting approval". Only a person
approves or rejects (`apply` refuses `Approve`/`Reject` from any other actor). Approval moves it
to Backlog and turns its pending dependencies into edges, and the settle rules do the rest; a
cycle, an edge to a card still proposed, or a `before` node that has started refuses it with a
typed error. Rejection cancels it with the reason. The mark stays.

**Why:** A chain is the plan a person agreed to. If agents could add tickets or edges directly,
the plan would grow silently, and readiness of planned work would depend on things nobody
approved. Keeping proposals out of the graph until approval makes "nothing changes until a
person says so" structural rather than a check in every rule.

**How to verify:** core transition tests (`machine/tests.rs`, `graph/tests/wildcard.rs`), use
cases over the in-memory ports (`app/src/tests/wildcards.rs`), the daemon route test, and the
end-to-end CLI test `crates/cli/tests/wildcard_cards.rs`.

**Alternatives:**

- Store proposed edges in the edge table and make the graph rules skip edges that touch a
  proposed card: rejected, every rule (readiness, order, cuts, cycles) would need the exception.
- Create the card in Backlog with a "draft" flag, like pause: rejected, a flag does not show in
  status, filters or the board, and every transition would have to check it.
- Let agents approve their own small cards: rejected, the point is that the owner sees the plan
  change before it happens.
- Count rejected cards as cancelled work in progress: rejected, they never joined the plan;
  `Ticket::in_plan` (planned, or an approved card) decides what counts and what is exported.

**Consequences / risks:** One more status in every client (`proposed`, category To Do). Chain
status shows `needs_human` while a decision is pending, even if everything else is done. The
actor of a command comes from the route (ticket actions run as a person, `/events` as the
agent); phase 1, where agents call the API with their own credentials, must keep agents off the
person-only routes. Chain files do not carry the mark: an exported approved card comes back as a
planned ticket.
