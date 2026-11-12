import { afterEach, describe, expect, it, vi } from "vitest";
import type { ConversationMessage, EventRecord, Project } from "../api/types";
import { aboutLabel, aboutRoute, mergeMessages, msUntilOverdue, unanswered, wakeWhenLate } from "./conversation";
import { ANSWER_WAIT_MS } from "./discussion";
import type { Msg } from "./msg";
import { reduce } from "./reduce";
import { type State, initialState } from "./state";

const project = { id: "P1", key: "DEMO", name: "Demo", archived: false } as unknown as Project;
const msg = (id: string, at: number, author: ConversationMessage["author"] = "person", extra: Partial<ConversationMessage> = {}): ConversationMessage =>
  ({ id, at, author, text: `text ${id}`, ...extra }) as ConversationMessage;
const start = (): State => ({ ...initialState({ name: "planner", project: "DEMO" }), projects: [project], loaded: true, lastSeq: 5 });
const apply = (s: State, ...msgs: Msg[]): State => msgs.reduce((state, m) => reduce(state, m)[0], s);
const loaded = (messages: ConversationMessage[], more = false) =>
  apply(start(), { type: "conversationLoaded", project: "DEMO", page: { messages, more } });
const event = (seq: number, entity: string, kind: string, payload: object): EventRecord =>
  ({ seq, entity, kind, entity_id: "P1", actor: "agent", at: 1, payload }) as unknown as EventRecord;

describe("the conversation as loaded", () => {
  it("opening the chat asks for the newest page, and the answer is the chat, oldest first", () => {
    const [, fx] = reduce(start(), { type: "loadConversation", project: "DEMO" });
    expect(fx).toEqual([{ type: "fetchConversation", project: "DEMO" }]);
    const s = loaded([msg("a", 1), msg("b", 2, "assistant")], true);
    expect(s.conversation.DEMO.messages.map((m) => m.id)).toEqual(["a", "b"]);
    expect(s.conversation.DEMO.more).toBe(true);
  });

  it("scrolling up asks for the page before the first message, once, and it joins above", () => {
    const s = loaded([msg("c", 3), msg("d", 4)], true);
    const [asked, fx] = reduce(s, { type: "loadOlder", project: "DEMO" });
    expect(fx).toEqual([{ type: "fetchConversation", project: "DEMO", before: "c" }]);
    expect(asked.conversation.DEMO.older).toBe(true);
    expect(reduce(asked, { type: "loadOlder", project: "DEMO" })[1]).toEqual([]);
    const done = apply(asked, { type: "conversationLoaded", project: "DEMO", before: "c", page: { messages: [msg("a", 1), msg("b", 2)], more: false } });
    expect(done.conversation.DEMO.messages.map((m) => m.id)).toEqual(["a", "b", "c", "d"]);
    expect(done.conversation.DEMO).toMatchObject({ more: false, older: false });
    // Nothing older: scrolling asks for nothing.
    expect(reduce(done, { type: "loadOlder", project: "DEMO" })[1]).toEqual([]);
  });

  it("a reload of the newest page keeps the older pages and a message that came in meanwhile", () => {
    const s = loaded([msg("c", 3), msg("d", 4)], true);
    const older = apply(s, { type: "conversationLoaded", project: "DEMO", before: "c", page: { messages: [msg("a", 1), msg("b", 2)], more: true } });
    const again = apply(older, { type: "conversationLoaded", project: "DEMO", page: { messages: [msg("c", 3), msg("d", 4), msg("e", 5, "assistant")], more: true } });
    expect(again.conversation.DEMO.messages.map((m) => m.id)).toEqual(["a", "b", "c", "d", "e"]);
    expect(again.conversation.DEMO.more).toBe(true);
  });

  it("merges by id, so a message from the send and the same one from the stream are one", () => {
    expect(mergeMessages([msg("a", 1)], [msg("a", 1), msg("b", 1)]).map((m) => m.id)).toEqual(["a", "b"]);
  });
});

describe("a message arriving", () => {
  it("a conversation event from the journal joins the open chat of its project", () => {
    const s = loaded([msg("a", 1)]);
    const e = event(6, "conversation", "message", { project: "P1", message: msg("b", 2, "assistant") });
    const [next] = reduce(s, { type: "event", event: e });
    expect(next.conversation.DEMO.messages.map((m) => m.id)).toEqual(["a", "b"]);
    expect(next.lastSeq).toBe(6);
  });

  it("a chat nobody opened takes nothing: it loads the page when it opens", () => {
    const e = event(6, "conversation", "message", { project: "P1", message: msg("b", 2) });
    const [next, fx] = reduce(start(), { type: "event", event: e });
    expect(next.conversation).toEqual({});
    expect(fx).toEqual([]);
  });

  it("a gap in the stream reloads the open chats; a card discussion's message does too", () => {
    const s = loaded([msg("a", 1)]);
    const gap = event(9, "conversation", "message", { project: "P1", message: msg("z", 9) });
    expect(reduce(s, { type: "event", event: gap })[1]).toEqual([{ type: "fetchConversation", project: "DEMO" }]);
    const said = event(6, "thread", "message_posted", { ticket: "DEMO-1", message: { id: "m1" } });
    expect(reduce(s, { type: "event", event: said })[1]).toContainEqual({ type: "fetchConversation", project: "DEMO" });
    const opened = event(6, "thread", "opened", { ticket: "DEMO-1" });
    expect(reduce(s, { type: "event", event: opened })[1]).not.toContainEqual({ type: "fetchConversation", project: "DEMO" });
  });
});

describe("the assistant answering", () => {
  it("is owed an answer while the person spoke last, whatever Toknila's lines say after it", () => {
    expect(unanswered([msg("a", 1)])?.id).toBe("a");
    expect(unanswered([msg("a", 1), msg("l", 2, "toknila")])?.id).toBe("a");
    expect(unanswered([msg("a", 1), msg("b", 2, "assistant")])).toBeUndefined();
    expect(unanswered([msg("a", 1), msg("b", 2, "assistant"), msg("c", 3), msg("l", 4, "toknila")])?.id).toBe("c");
    expect(unanswered([])).toBeUndefined();
  });

  it("is overdue after the same 90 seconds as a card's discussion", () => {
    expect(ANSWER_WAIT_MS).toBe(90_000);
    const m = [msg("a", 1000)];
    expect(msUntilOverdue(m, 1000)).toBe(ANSWER_WAIT_MS);
    expect(msUntilOverdue(m, 1000 + ANSWER_WAIT_MS)).toBe(0);
    expect(msUntilOverdue([msg("a", 1), msg("b", 2, "assistant")], 5)).toBeUndefined();
  });

  describe("the wake-up", () => {
    afterEach(() => vi.useRealTimers());
    it("fires when the message becomes overdue and can be stopped", () => {
      vi.useFakeTimers();
      const wake = vi.fn();
      const stop = wakeWhenLate([msg("a", 1000)], 1000, wake);
      vi.advanceTimersByTime(ANSWER_WAIT_MS - 1);
      expect(wake).not.toHaveBeenCalled();
      vi.advanceTimersByTime(1);
      expect(wake).toHaveBeenCalledTimes(1);
      const quiet = vi.fn();
      wakeWhenLate([msg("a", 1000)], 1000, quiet)();
      vi.advanceTimersByTime(ANSWER_WAIT_MS * 2);
      expect(quiet).not.toHaveBeenCalled();
      stop();
    });
  });

  it("a nudge of a message about no card types its text again; about a card it is the card's line", () => {
    const [, plain] = reduce(start(), { type: "nudgeChat", project: "DEMO", text: "a shop\nwith a cart" });
    expect(plain).toEqual([{ type: "plannerCall", project: "DEMO", call: "nudge", text: "a shop with a cart" }]);
    const [, card] = reduce(start(), { type: "nudgeCard", project: "DEMO", key: "DEMO-1", thread: 2, text: "why" });
    expect(card).toEqual([{ type: "plannerCall", project: "DEMO", call: "nudge", card: "DEMO-1", thread: 2, text: "why" }]);
  });
});

describe("where a chip or a Toknila line leads", () => {
  it("to the card's page when the store has the card, to the chain by name, else nowhere", () => {
    const chain = { chain: { id: "C1", name: "plan", project_id: "P1" }, tickets: [{ id: "T1", key: "DEMO-1" }] };
    const s = { ...start(), chains: { C1: chain as never }, chainOrder: ["C1"] };
    expect(aboutRoute(s, "DEMO", { kind: "card", key: "DEMO-1" })).toEqual({ name: "card", project: "DEMO", chain: "plan", key: "DEMO-1" });
    expect(aboutRoute(s, "DEMO", { kind: "card", key: "DEMO-9" })).toBeUndefined();
    expect(aboutRoute(s, "DEMO", { kind: "chain", name: "plan" })).toEqual({ name: "projectChain", project: "DEMO", chain: "plan" });
    expect(aboutRoute(s, "DEMO", null)).toBeUndefined();
    expect(aboutLabel({ kind: "card", key: "DEMO-1" })).toBe("DEMO-1");
    expect(aboutLabel({ kind: "chain", name: "plan" })).toBe("chain plan");
  });
});
