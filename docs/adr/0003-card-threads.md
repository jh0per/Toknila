# [decision] A card's design is settled in threads that a person approves before the hand-in

**What:** A ticket can hold threads: a screen design, a flowchart, a process, an architecture
diagram, or anything else to settle before code. A thread has a kind (`design`, `flow`,
`architecture`, `other`), a title, a number on its ticket (`#1`, how people address it) and a
state, `open` or `approved`. A person or the ticket's agent opens one. A thread holds messages
(from a person or the agent) and artifact versions: a format (`mermaid`, `markdown`, `html`) and a
text body of at most 512 KiB, numbered per thread and never changed. Only a person approves, and
approval pins the latest version (a thread without an artifact may be approved too, as a decision
in words). Posting a new version to an approved thread reopens it and drops the pin; a person's
"request changes" is a message that keeps the thread open or puts it back to open. While any
thread of a ticket is open, the agent's hand-in (`finished`, Running to Review) is refused with a
`409` that names the open threads, and the ticket does not move. The ticket's phase, `design`
while any thread is open and `implement` otherwise, is computed in the read model and never
stored.

**Why:** Nothing else makes the owner see and agree to a design before the agent spends a run on
it. A gate on the hand-in, not on starting, lets the agent work on the thread (and read the code
to draw it) without waiting, while still guaranteeing that no implementation reaches review
before the design is settled. The rules are pure functions in `toknila_core::thread`; the app
orders them into commands; an agent event can open a thread and post to it but there is no event
that approves, and core refuses `Approve` from any actor but a person.

**How to verify:** core tests (`thread/tests.rs`), use cases over the in-memory ports
(`app/src/tests/threads.rs`), the daemon route and migration tests, and the end-to-end CLI test
`crates/cli/tests/card_threads.rs`.

**Alternatives:**

- Store the phase on the ticket: rejected, it would have to be kept in step with every thread
  change; computing it from the threads cannot drift.
- Gate on starting the ticket (design first, then Running): rejected, the agent needs a running
  ship to look at the code while it drafts the design; the gate sits where code would be handed
  in.
- Edit artifacts in place with history on the side: rejected, an approval has to point at bytes
  that cannot change; immutable numbered versions make the pin a number.
- Make threads part of the comment stream: rejected, comments have no state, versions or
  approval, and "is every thread approved" would become a scan of free text.
- Journal the artifact body: rejected, up to 512 KiB per event would bloat the journal and the
  stream; the event carries number, format, author and size, the body is one read away.

**Consequences / risks:** One migration (three tables). `TicketDetail` gains `phase` (every
snapshot of a ticket shows it). The hand-in check is in the app use case, so a new path that moves
Running to Review must call it. A person who approves a thread that the agent keeps editing sees
it reopen on every new version; that is intended. The thread routes run as a person: phase 1,
where agents call the API with their own credentials, must keep agents off them (the agent events
are the agent's route). Opening a thread on a finished ticket is allowed and has no effect on its
status. The web Threads tab and the rendering of mermaid and html come next.
