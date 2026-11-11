import { afterEach, describe, expect, it, vi } from "vitest";
import type { ThreadDetail } from "../api/types";
import { ANSWER_WAIT_MS, overdue, wakeWhenOverdue } from "./discussion";
import type { Msg } from "./msg";
import { reduce } from "./reduce";
import { type State, initialState } from "./state";

const start = (): State => ({ ...initialState({ name: "card", project: "DEMO", chain: "plan", key: "DEMO-1" }), loaded: true });
const apply = (s: State, ...msgs: Msg[]): State => msgs.reduce((state, m) => reduce(state, m)[0], s);

describe("talking to the assistant about a card", () => {
  it("sends the card's draft through the planner call and clears it when the line reached the terminal", () => {
    const typed = apply(start(), { type: "editPlannerDraft", where: "card:DEMO-1", text: "  stricter errors " });
    const [s, fx] = reduce(typed, { type: "discussCard", project: "DEMO", key: "DEMO-1" });
    expect(fx).toEqual([
      { type: "plannerCall", project: "DEMO", call: "discuss", card: "DEMO-1", draft: "card:DEMO-1", text: "stricter errors", profile: "claude-code-planner" },
    ]);
    expect(s.planner.sending).toBe("card:DEMO-1");
    // While it is on its way a second send does nothing.
    expect(reduce(s, { type: "discussCard", project: "DEMO", key: "DEMO-1" })[1]).toEqual([]);
    const done = apply(s, { type: "plannerAsked", project: "DEMO", draft: "card:DEMO-1" });
    expect(done.planner.sending).toBeUndefined();
    expect(done.planner.drafts["card:DEMO-1"]).toBe("");
  });

  it("sends nothing for an empty draft", () => {
    expect(reduce(start(), { type: "discussCard", project: "DEMO", key: "DEMO-1" })[1]).toEqual([]);
  });

  it("accepts and rejects a proposed edit by its message id", () => {
    const [, fx] = reduce(start(), { type: "threadAction", key: "DEMO-1", thread: "1", action: "accept_edit", text: "m2" });
    expect(fx).toEqual([{ type: "threadAction", key: "DEMO-1", thread: "1", action: "accept_edit", text: "m2" }]);
  });
});

describe("a message the assistant does not answer", () => {
  const said = (at: number, author = "human") => ({ id: `m${at}`, kind: "message", author, text: "stricter", thread_id: "T", created_at: at });
  const thread = (...messages: ReturnType<typeof said>[]) =>
    ({ thread: { id: "T", number: 1, kind: "discussion" }, messages, versions: [] }) as unknown as ThreadDetail;

  afterEach(() => vi.useRealTimers());

  it("is overdue after 90 seconds on a fake clock, and the timer wakes exactly then", () => {
    vi.useFakeTimers();
    vi.setSystemTime(1_000_000);
    const d = thread(said(1_000_000));
    const woke = vi.fn();
    wakeWhenOverdue(d, Date.now(), woke);
    expect(overdue(d, Date.now())).toBe(false);
    vi.advanceTimersByTime(ANSWER_WAIT_MS - 1);
    expect(woke).not.toHaveBeenCalled();
    vi.advanceTimersByTime(1);
    expect(woke).toHaveBeenCalledTimes(1);
    expect(overdue(d, Date.now())).toBe(true);
  });

  it("is not overdue once the assistant has answered, and its timer is cancelled", () => {
    vi.useFakeTimers();
    vi.setSystemTime(2_000_000);
    const answered = thread(said(2_000_000 - 100_000), said(2_000_000 - 50_000, "agent"));
    expect(overdue(answered, Date.now())).toBe(false);
    const stop = wakeWhenOverdue(thread(said(2_000_000)), Date.now(), () => {
      throw new Error("woke after being stopped");
    });
    stop();
    vi.advanceTimersByTime(ANSWER_WAIT_MS * 2);
  });

  it("a nudge sends the same message to the planner's input without touching the draft", () => {
    const typed = apply(start(), { type: "editPlannerDraft", where: "card:DEMO-1", text: "keep me" });
    const [s, fx] = reduce(typed, { type: "nudgeCard", project: "DEMO", key: "DEMO-1", thread: 1, text: "stricter" });
    expect(fx).toEqual([{ type: "plannerCall", project: "DEMO", call: "nudge", card: "DEMO-1", thread: 1, text: "stricter" }]);
    expect(s.planner.drafts["card:DEMO-1"]).toBe("keep me");
  });
});
