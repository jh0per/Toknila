import { afterEach, describe, expect, it, vi } from "vitest";
import type { ConversationMessage, EventRecord, PlannerState, Project } from "../api/types";
import { ANSWER_WAIT_MS } from "./discussion";
import type { Effect, Msg } from "./msg";
import { STEP_POLL_MS, plannerOf } from "./planner";
import { runPlannerPoll } from "./plannerEffects";
import { lastTurnEnded, turnView, workingTitle } from "./plannerTurn";
import { reduce } from "./reduce";
import { type State, initialState } from "./state";

const project = { id: "P1", key: "WEB", name: "Web" } as unknown as Project;
const start = (): State => ({ ...initialState({ name: "project", project: "WEB" } as never), projects: [project], lastSeq: 10 });

const run = (s: State, ...msgs: Msg[]): [State, Effect[]] =>
  msgs.reduce<[State, Effect[]]>(([at], m) => reduce(at, m), [s, []]);

const planner = (state: Partial<PlannerState> = {}): PlannerState => ({ profile: "fake", running: true, working: false, step: null, since: null, ...state });
const loaded = (state: Partial<PlannerState> = {}): Msg => ({ type: "plannerLoaded", project: "WEB", state: planner(state) });
const shown = (on: boolean): Msg => ({ type: "plannerShown", project: "WEB", on });
const turn = (seq: number, working: boolean, at: number): Msg => ({
  type: "event",
  event: { seq, entity: "planner", kind: "turn", entity_id: "P1", at, actor: "agent", payload: { project: "P1", working } } as unknown as EventRecord,
});
const polls = (fx: Effect[]) => fx.filter((e) => e.type === "plannerPoll");

describe("the planner slice follows the assistant's turn", () => {
  it("a planner.turn entry turns working on at once, and its end drops the step and is remembered", () => {
    let [s] = run(start(), loaded());
    [s] = run(s, turn(11, true, 5_000));
    expect(plannerOf(s, "WEB")).toMatchObject({ working: true, since: 5_000 });
    // A poll's answer names the step.
    [s] = run(s, loaded({ working: true, step: "Reading README.md", since: 5_000 }));
    expect(plannerOf(s, "WEB")?.step).toBe("Reading README.md");
    expect(lastTurnEnded(s, "WEB")).toBeUndefined();
    let fx: Effect[];
    [s, fx] = run(s, turn(12, false, 9_000));
    expect(plannerOf(s, "WEB")).toMatchObject({ working: false, step: null, since: null, running: true });
    expect(lastTurnEnded(s, "WEB")).toEqual({ start: 5_000, end: 9_000 });
    // The daemon is asked as well: which answers close their messages depends on the turn.
    expect(fx).toContainEqual({ type: "plannerCall", project: "WEB", call: "load" });
  });

  it("an entry for a project whose assistant the window has not loaded asks the daemon for it", () => {
    const [s, fx] = run(start(), turn(11, true, 5_000));
    expect(plannerOf(s, "WEB")).toBeUndefined();
    expect(fx).toContainEqual({ type: "plannerCall", project: "WEB", call: "load" });
  });

  it("starts the poll once a turn is on in a project a chat or an ask box shows, and only once", () => {
    let [s, fx] = run(start(), loaded(), shown(true));
    expect(polls(fx)).toEqual([]);
    [s, fx] = run(s, turn(11, true, 5_000));
    expect(polls(fx)).toEqual([{ type: "plannerPoll", project: "WEB", on: true }]);
    [s, fx] = run(s, loaded({ working: true, step: "Reading README.md" }));
    expect(polls(fx)).toEqual([]);
    expect(s.planner.polling).toEqual(["WEB"]);
  });

  it("does not poll a turn nothing on the screen shows", () => {
    const [s, fx] = run(start(), loaded(), turn(11, true, 5_000));
    expect(polls(fx)).toEqual([]);
    expect(s.planner.polling).toEqual([]);
  });

  it("stops the poll when the turn ends", () => {
    let [s] = run(start(), loaded(), shown(true), turn(11, true, 5_000));
    let fx: Effect[];
    [s, fx] = run(s, turn(12, false, 9_000));
    expect(polls(fx)).toEqual([{ type: "plannerPoll", project: "WEB", on: false }]);
    expect(s.planner.polling).toEqual([]);
  });

  it("stops the poll when nothing on the screen shows the project, and drops the step nobody follows", () => {
    let [s] = run(start(), loaded(), shown(true), shown(true), turn(11, true, 5_000), loaded({ working: true, step: "Adding a story" }));
    let fx: Effect[];
    // Two places showed it: one leaving keeps the poll.
    [s, fx] = run(s, shown(false));
    expect(polls(fx)).toEqual([]);
    [s, fx] = run(s, shown(false));
    expect(polls(fx)).toEqual([{ type: "plannerPoll", project: "WEB", on: false }]);
    expect(plannerOf(s, "WEB")).toMatchObject({ working: true, step: null });
    // Back on the screen while the turn is still on: the poll starts again.
    [s, fx] = run(s, shown(true));
    expect(polls(fx)).toEqual([{ type: "plannerPoll", project: "WEB", on: true }]);
  });

  it("remembers the box a message was sent from, and forgets it when the chat sends", () => {
    let [s] = run(start(), { type: "editPlannerDraft", where: "WEB", text: "plan" }, { type: "askPlanner", project: "WEB", from: "chain:hide" });
    expect(s.planner.from).toEqual({ WEB: "chain:hide" });
    [s] = run(s, { type: "plannerAsked", project: "WEB" }, { type: "editPlannerDraft", where: "WEB", text: "more" }, { type: "askPlanner", project: "WEB" });
    expect(s.planner.from).toEqual({});
  });

  it("names the step in the title of the Assistant button and row while a turn is on", () => {
    expect(workingTitle(planner({ working: true, step: "Reading README.md" }))).toBe("The assistant is working: Reading README.md");
    expect(workingTitle(planner({ working: true }))).toBe("The assistant is working");
    expect(workingTitle(planner())).toBeUndefined();
    expect(workingTitle(planner({ running: false, working: true }))).toBeUndefined();
    expect(workingTitle(undefined)).toBeUndefined();
  });
});

describe("the poll's timer", () => {
  afterEach(() => {
    vi.useRealTimers();
    vi.unstubAllGlobals();
  });

  it("asks the daemon for the step every two seconds until it is told to stop, with one timer per project", async () => {
    vi.useFakeTimers();
    const asked: string[] = [];
    vi.stubGlobal("fetch", (url: string) => {
      asked.push(url);
      return Promise.resolve(new Response(JSON.stringify(planner({ working: true, step: "Reading README.md" })), { status: 200 }));
    });
    vi.stubGlobal("window", { location: { search: "?token=t", origin: "http://x" } });
    const got: Msg[] = [];
    runPlannerPoll({ type: "plannerPoll", project: "WEB", on: true }, (m) => got.push(m));
    runPlannerPoll({ type: "plannerPoll", project: "WEB", on: true }, (m) => got.push(m));
    await vi.advanceTimersByTimeAsync(STEP_POLL_MS - 1);
    expect(asked).toEqual([]);
    await vi.advanceTimersByTimeAsync(1);
    expect(asked).toHaveLength(1);
    expect(asked[0]).toContain("/projects/WEB/planner");
    await vi.advanceTimersByTimeAsync(STEP_POLL_MS);
    expect(asked).toHaveLength(2);
    expect(got).toContainEqual({ type: "plannerLoaded", project: "WEB", state: planner({ working: true, step: "Reading README.md" }) });
    runPlannerPoll({ type: "plannerPoll", project: "WEB", on: false }, (m) => got.push(m));
    await vi.advanceTimersByTimeAsync(STEP_POLL_MS * 3);
    expect(asked).toHaveLength(2);
  });
});

describe("what the chat says about the turn", () => {
  const SENT = 1_000_000;
  const plan: ConversationMessage = { id: "m1", author: "person", text: "plan", at: SENT };
  const MIN = 60_000;

  it("a turn on past 90 seconds: the waiting line with the step, and no Nudge however long", () => {
    for (const after of [ANSWER_WAIT_MS + 1, 4 * MIN, 30 * MIN]) {
      const v = turnView([plan], planner({ working: true, step: "Reading README.md", since: SENT + 500 }), undefined, false, SENT + after);
      expect(v).toMatchObject({ waiting: true, step: "Reading README.md", ended: false, late: false, since: SENT });
    }
  });

  it("a turn that ended without an answer says so at once and offers Nudge, with no waiting line", () => {
    const v = turnView([plan], planner(), { start: SENT + 400, end: SENT + 3_000 }, false, SENT + 3_100);
    expect(v).toMatchObject({ waiting: false, ended: true, late: false });
    expect(v.owed?.id).toBe("m1");
  });

  it("a message no turn started for: the waiting line, and Nudge only after 90 seconds", () => {
    // A turn that ended before the message was sent is not its turn.
    const last = { start: SENT - 9_000, end: SENT - 5_000 };
    const before = turnView([plan], planner(), last, false, SENT + ANSWER_WAIT_MS - 1);
    expect(before).toMatchObject({ waiting: true, ended: false, late: false });
    const after = turnView([plan], planner(), last, false, SENT + ANSWER_WAIT_MS);
    expect(after).toMatchObject({ waiting: true, ended: false, late: true });
  });

  it("a message sent while another turn was on: that turn ending is not its own, so it still waits", () => {
    const v = turnView([plan], planner(), { start: SENT - 2_000, end: SENT + 3_000 }, false, SENT + 3_100);
    expect(v).toMatchObject({ waiting: true, ended: false, late: false });
  });

  it("an answered message: nothing waits, nothing ended", () => {
    const answer: ConversationMessage = { id: "m2", author: "assistant", text: "Two stories.", at: SENT + 2_000 };
    expect(turnView([plan, answer], planner(), { start: SENT + 400, end: SENT + 2_500 }, false, SENT + 3_000)).toMatchObject({ waiting: false, ended: false, late: false });
  });

  it("a turn on with no message of the person's counts from the turn's start", () => {
    const answer: ConversationMessage = { id: "m2", author: "assistant", text: "Two stories.", at: SENT + 2_000 };
    const v = turnView([plan, answer], planner({ working: true, step: "Adding a story", since: SENT + 9_000 }), undefined, false, SENT + 10_000);
    expect(v).toMatchObject({ waiting: true, since: SENT + 9_000, step: "Adding a story", late: false });
  });

  it("a message on its way waits, whatever the last turn did", () => {
    expect(turnView([plan], planner(), { start: SENT + 400, end: SENT + 3_000 }, true, SENT + 3_100)).toMatchObject({ waiting: true, ended: false, late: false });
  });

  it("an assistant that is not running neither waits nor ended (the chat says it is not running)", () => {
    const v = turnView([plan], planner({ running: false }), { start: SENT + 400, end: SENT + 3_000 }, false, SENT + 4 * MIN);
    expect(v).toMatchObject({ waiting: false, ended: false, late: false });
  });
});
