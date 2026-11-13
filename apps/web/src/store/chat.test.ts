import { describe, expect, it } from "vitest";
import type { Comment, EventRecord, TicketDetail } from "../api/types";
import { agentLive, canTalk, chatItems, composerPlaceholder, defaultMode, pendingText, whenText } from "./chat";

const at = (h: number, m = 0) => new Date(2026, 10, 8, h, m).getTime();
const comment = (id: string, kind: Comment["kind"], author: Comment["author"], gist: string, when: number, extra: object = {}): Comment =>
  ({ id, ticket_id: "t", kind, author, body: { gist, ...extra }, created_at: when, delivered_at: null }) as unknown as Comment;
const detail = (comments: Comment[]) => ({ comments }) as unknown as TicketDetail;
const event = (seq: number, when: number, kind: string, payload: object, actor: EventRecord["actor"] = "system"): EventRecord =>
  ({ seq, at: when, actor, entity: "ticket", entity_id: "t", kind, payload }) as EventRecord;

describe("chatItems", () => {
  it("makes one bubble per comment, oldest first, with who and when", () => {
    const d = detail([
      comment("c2", "report", "agent", "Login form done", at(10, 30), { what: "Added the form", why: "It was asked" }),
      comment("c1", "instruction", "human", "Use the blue button", at(10, 0)),
      comment("c3", "decision", "human", "Ship it", at(11, 0)),
    ]);
    const items = chatItems(d, [], false);
    expect(items.map((i) => [i.side, i.kind, i.text])).toEqual([
      ["you", "instruction", "Use the blue button"],
      ["agent", "report", "Login form done"],
      ["you", "decision", "Ship it"],
    ]);
    expect(items[1].more).toEqual(["Added the form", "It was asked"]);
    expect(items[0].who).toBe("You");
    expect(items[1].who).toBe("Agent");
    expect(items[0].at).toBe(at(10, 0));
  });

  it("marks an instruction the agent has not received yet", () => {
    const sent = { ...comment("c1", "instruction", "human", "Wait", at(9)), delivered_at: at(9, 1) } as Comment;
    const items = chatItems(detail([sent, comment("c2", "instruction", "human", "Stop", at(9, 5))]), [], false);
    expect(items.map((i) => i.pending)).toEqual([false, true]);
  });

  it("leaves out the old permission reports that the Inbox now carries", () => {
    const d = detail([
      comment("c1", "report", "agent", "Claude Code asks for permission: Bash", at(9)),
      comment("c2", "report", "agent", "Done", at(10)),
    ]);
    expect(chatItems(d, [], false).map((i) => i.text)).toEqual(["Done"]);
  });

  it("adds the agent's questions and the person's answers from the journal", () => {
    const events = [
      event(1, at(10, 0), "status_changed", {
        from: "running", to: "waiting", reason: { status: "waiting", kind: "question", text: "Archive or delete?" },
      }),
      event(2, at(10, 5), "question_answered", { answer: "allow", role: "worker" }, "human"),
      event(3, at(10, 6), "status_changed", { from: "waiting", to: "running", reason: null }),
      event(4, at(10, 7), "question_raised", { role: "reviewer", kind: "permission" }),
      event(5, at(10, 8), "ship_created", {}),
    ];
    const items = chatItems(detail([]), events, false);
    expect(items.map((i) => [i.side, i.kind, i.who, i.text])).toEqual([
      ["agent", "question", "Agent", "Archive or delete?"],
      ["you", "answer", "You", "Allowed"],
      ["agent", "question", "Reviewer", "Asks for permission."],
    ]);
  });

  it("does not repeat the open question the answer box shows", () => {
    const events = [
      event(1, at(9, 0), "status_changed", { from: "running", to: "waiting", reason: { status: "waiting", kind: "question", text: "Old one?" } }),
      event(2, at(9, 5), "question_answered", { answer: "text" }, "human"),
      event(3, at(10, 0), "status_changed", { from: "running", to: "waiting", reason: { status: "waiting", kind: "question", text: "Open one?" } }),
    ];
    expect(chatItems(detail([]), events, true).map((i) => i.text)).toEqual(["Old one?", "Typed an answer on its screen"]);
    expect(chatItems(detail([]), events, false).map((i) => i.text)).toContain("Open one?");
  });

  it("shows a question once, though the daemon also writes it down as a report", () => {
    const d = detail([
      comment("c1", "report", "agent", "Login form drafted", at(9, 0)),
      comment("c2", "report", "agent", "Archive or delete?", at(10, 0)),
    ]);
    const events = [event(1, at(10, 0) + 20, "status_changed", {
      from: "running", to: "waiting", reason: { status: "waiting", kind: "question", text: "Archive or delete?" },
    })];
    expect(chatItems(d, events, false).map((i) => [i.kind, i.text])).toEqual([
      ["report", "Login form drafted"],
      ["question", "Archive or delete?"],
    ]);
    // Open: the answer box shows it, so no bubble repeats it.
    expect(chatItems(d, events, true).map((i) => i.text)).toEqual(["Login form drafted"]);
    // The same words long after are two different reports, and both stay.
    const later = detail([comment("c2", "report", "agent", "Archive or delete?", at(10, 0) + 60_000)]);
    expect(chatItems(later, events, false).map((i) => i.kind)).toEqual(["question", "report"]);
  });

  it("an agent that went silent is a note", () => {
    const events = [event(1, at(9), "status_changed", { reason: { status: "waiting", kind: "silent" } })];
    expect(chatItems(undefined, events, false)).toMatchObject([{ kind: "note", text: "The agent stopped responding." }]);
  });
});

describe("whenText and canTalk", () => {
  const now = at(15, 0);
  it("shows the time today, and the day with it before", () => {
    expect(whenText(at(9, 5), now)).toBe("09:05");
    expect(whenText(new Date(2026, 10, 6, 23, 59).getTime(), now)).toBe("Nov 6, 23:59");
  });
  it("says in one sentence what the daemon does with a pending instruction", () => {
    expect(pendingText({ reason: "starts_agent", busy: 1, slots: 2 })).toBe("Starting the agent; it gets this when it is up.");
    expect(pendingText({ reason: "waits_for_slot", busy: 2, slots: 2 })).toBe(
      "2 of 2 agents busy; the agent starts when a slot is free and gets this then.",
    );
    expect(pendingText({ reason: "at_start", busy: 0, slots: 2 })).toBe("The story has not started; the agent gets this when it does.");
    expect(pendingText(undefined)).toBe("The agent gets this when it next stops.");
  });
  it("marks an instruction delivered once it has been", () => {
    const sent = { ...comment("c1", "instruction", "human", "Wait", at(9)), delivered_at: at(9, 1) } as Comment;
    const [item] = chatItems(detail([sent]), [], false);
    expect([item.pending, item.delivered]).toEqual([false, true]);
  });
  it("words the composer by whether an agent is alive", () => {
    const paused = { status: "waiting", agent_paused: { resume: false } } as never;
    const lost = { status: "blocked", status_reason: { status: "blocked", reason: { kind: "runner_lost" } } } as never;
    expect(composerPlaceholder(paused)).toBe("Answer here; it starts the agent");
    expect(composerPlaceholder(lost)).toBe("Answer here; it starts the agent");
    expect(composerPlaceholder({ status: "ready" } as never)).toBe("Leave a note; the agent gets it when the story starts");
    expect(composerPlaceholder({ status: "running" } as never)).toBe("Tell the agent something; Enter sends");
  });
  it("makes Ask the composer's default when no agent is live, and Tell the agent when one is", () => {
    const card = (status: string, extra: object = {}) => ({ status, ...extra }) as never;
    const person = { review: { turn: { by: "person" } } };
    expect(defaultMode(card("review", person))).toBe("ask");
    expect(defaultMode(card("merged"))).toBe("ask");
    expect(defaultMode(card("blocked"))).toBe("ask");
    expect(defaultMode(card("waiting", { agent_paused: { resume: false } }))).toBe("ask");
    expect(defaultMode(card("ready"))).toBe("ask");
    expect(defaultMode(card("running"))).toBe("tell");
    expect(defaultMode(card("waiting"))).toBe("tell");
    expect(agentLive(card("running", { agent_paused: { resume: false } }))).toBe(false);
  });
  it("reads a Review card at the person's turn as an agent that handed in and is gone", () => {
    const review = { status: "review", review: { turn: { by: "person" } } } as never;
    expect(composerPlaceholder(review, "ask")).toBe("Ask what to do, or tell the agent for its next run");
    expect(pendingText(undefined, review)).toBe(
      "The agent handed in and is gone; it gets this if you Return the story. Ask instead?",
    );
    // A running agent keeps the old sentence and offers no link; a stopped one gets the link.
    expect(pendingText(undefined, { status: "running" } as never)).toBe("The agent gets this when it next stops.");
    expect(pendingText(undefined, { status: "blocked" } as never)).toBe("The agent gets this when it next stops. Ask instead?");
  });
  it("shows a question as the person's and the assistant's answer as its own bubble", () => {
    const d = detail([
      comment("c1", "question", "human", "шо треба зробити?", at(9)),
      comment("c2", "report", "assistant", "Відкрийте на телефоні", at(9, 1)),
    ]);
    const items = chatItems(d, [], false);
    expect(items.map((i) => [i.side, i.who, i.kind, i.text, i.pending])).toEqual([
      ["you", "You", "ask", "шо треба зробити?", false],
      ["assistant", "Assistant", "answer", "Відкрийте на телефоні", false],
    ]);
  });
  it("lets nobody be told anything on a finished card", () => {
    expect(canTalk({ status: "running" })).toBe(true);
    expect(canTalk({ status: "backlog" })).toBe(true);
    expect(canTalk({ status: "merged" })).toBe(false);
    expect(canTalk({ status: "released" })).toBe(false);
    expect(canTalk({ status: "cancelled" })).toBe(false);
  });
});
