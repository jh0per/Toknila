import { describe, expect, it } from "vitest";
import type { ChainDetail, EventRecord } from "../api/types";
import { reduce } from "./reduce";
import { COLLECT_MS, LIVE_MS, hear, soundFor } from "./sound";
import { type State, initialState } from "./state";

const NOW = 1_000_000;
let seq = 0;

const status = (id: string, to: string, extra: { from?: string; reason?: unknown; chain?: string } = {}): EventRecord =>
  ({
    seq: ++seq, at: NOW, actor: "system", entity: "ticket", entity_id: id, kind: "status_changed",
    payload: { to, from: extra.from ?? "review", reason: extra.reason ?? null, ticket: { id, chain_id: extra.chain ?? "c1", status: to } },
  }) as unknown as EventRecord;
const blocked = (id: string, kind: string) =>
  status(id, "blocked", { from: "running", reason: { status: "blocked", reason: { kind }, resume_to: "running" } });
const run = (kind: string): EventRecord =>
  ({ seq: ++seq, at: NOW, actor: "system", entity: "release_run", entity_id: "r1", kind, payload: { kind } }) as unknown as EventRecord;

const chain = (tickets: Record<string, string>): ChainDetail =>
  ({ chain: { id: "c1" }, tickets: Object.entries(tickets).map(([id, st]) => ({ id, chain_id: "c1", status: st })) }) as unknown as ChainDetail;

const on = { story_done: true, epic_done: true, problem: true };
function window(tickets: Record<string, string> = { t1: "review", t2: "running" }, switches = on): State {
  const s = initialState({ name: "chains" });
  return { ...s, chains: { c1: chain(tickets) }, sound: { switches, due: undefined } };
}
/// What the reducer makes of one live event that arrived at `NOW + age`.
const live = (s: State, e: EventRecord, age = 0) => reduce(s, { type: "event", event: e, received: NOW + age });
const kinds = (fx: { type: string }[]) => fx.filter((f) => f.type === "soundWindow").length;

describe("which sound a journal event calls for", () => {
  it("a story that reaches Merged is a story done", () => {
    expect(soundFor(window(), status("t1", "merged"))).toBe("story_done");
  });
  it("a story that goes Released, Review or Running is silent", () => {
    for (const to of ["released", "review", "running", "waiting", "ready", "cancelled"]) {
      expect(soundFor(window(), status("t1", to)), to).toBeUndefined();
    }
  });
  it("work imported as already done is not a story closing", () => {
    expect(soundFor(window(), status("t1", "merged", { from: "backlog" }))).toBeUndefined();
  });
  it("the last open story of an epic is an epic finished, not a story done", () => {
    expect(soundFor(window({ t1: "review", t2: "merged", t3: "cancelled" }), status("t1", "merged"))).toBe("epic_done");
  });
  it("a story finished while another is still open is only a story done", () => {
    expect(soundFor(window({ t1: "review", t2: "waiting" }), status("t1", "merged"))).toBe("story_done");
  });
  it("a status change the chain has not reloaded yet still counts (the journal is read too)", () => {
    const s = window({ t1: "review", t2: "review" });
    const first = status("t2", "merged");
    const withFirst = { ...s, events: [first] };
    expect(soundFor(withFirst, status("t1", "merged"))).toBe("epic_done");
  });
  it("a proposed story is not part of the epic", () => {
    expect(soundFor(window({ t1: "review", t2: "proposed" }), status("t1", "merged"))).toBe("epic_done");
  });
  it("a release run that passed every step is an epic finished", () => {
    expect(soundFor(window(), run("run_done"))).toBe("epic_done");
  });
  it("a release step that failed is something wrong; its other steps are silent", () => {
    expect(soundFor(window(), run("step_failed"))).toBe("problem");
    for (const k of ["run_started", "step_started", "step_passed", "turn_taken", "run_interrupted"]) {
      expect(soundFor(window(), run(k)), k).toBeUndefined();
    }
  });
  it("a story blocked for a reason of its own is something wrong", () => {
    for (const k of ["crashed", "merge_conflict", "ci_failed", "setup_failed", "ship_failed", "terminated"]) {
      expect(soundFor(window(), blocked("t1", k)), k).toBe("problem");
    }
  });
  it("a story the person blocked by hand is silent", () => {
    expect(soundFor(window(), blocked("t1", "manual"))).toBeUndefined();
  });
  it("the restart that heals itself (the runner lost) is silent", () => {
    expect(soundFor(window(), blocked("t1", "runner_lost"))).toBeUndefined();
  });
  it("a check round that failed and went back to its agent is silent", () => {
    expect(soundFor(window(), status("t1", "running", { from: "review" }))).toBeUndefined();
  });
});

describe("when a sound plays", () => {
  it("a live event whose switch is on waits for the burst to end, then plays once", () => {
    const [held, fx] = live(window(), status("t1", "merged"));
    expect(held.sound.due).toBe("story_done");
    expect(fx.filter((f) => f.type === "soundWindow")).toEqual([{ type: "soundWindow" }]);
    const [after, play] = reduce(held, { type: "soundDue" });
    expect(play).toEqual([{ type: "playSound", kind: "story_done" }]);
    expect(after.sound.due).toBeUndefined();
    expect(reduce(after, { type: "soundDue" })[1]).toEqual([]);
  });

  it("nothing plays for the history the window loads at its start", () => {
    const s = window();
    const [loaded, fx] = reduce(s, { type: "journalTail", seq: 9, events: [status("t1", "merged")] });
    expect(loaded.sound.due).toBeUndefined();
    expect(fx.map((f) => f.type)).toEqual(["connect"]);
  });

  it("nothing plays for events the stream catches up on after a reconnect (old by then)", () => {
    const [s, fx] = live(window(), status("t1", "merged"), LIVE_MS + 1);
    expect(s.sound.due).toBeUndefined();
    expect(kinds(fx)).toBe(0);
    expect(live(window(), status("t1", "merged"), LIVE_MS)[0].sound.due).toBe("story_done");
  });

  it("an event with no arrival time is not live", () => {
    const [s] = reduce(window(), { type: "event", event: status("t1", "merged") });
    expect(s.sound.due).toBeUndefined();
  });

  it("an event seen before (same seq) makes no second sound", () => {
    const e = status("t1", "merged");
    const [once] = live(window(), e);
    const [again] = reduce({ ...once, sound: { ...once.sound, due: undefined } }, { type: "event", event: e, received: NOW });
    expect(again.sound.due).toBeUndefined();
  });

  it("several within the window play one: the most important, wrong then epic then story", () => {
    let s = window({ t1: "review", t2: "review", t3: "running" });
    const play = (order: EventRecord[]) => {
      let st = s;
      let timers = 0;
      for (const e of order) {
        const [n, fx] = live(st, e);
        st = n;
        timers += kinds(fx);
      }
      expect(timers, "one wait for the burst").toBe(1);
      return reduce(st, { type: "soundDue" })[1];
    };
    const story = status("t1", "merged");
    const epic = run("run_done");
    const wrong = blocked("t3", "crashed");
    expect(play([story, epic, wrong])).toEqual([{ type: "playSound", kind: "problem" }]);
    expect(play([wrong, story, epic])).toEqual([{ type: "playSound", kind: "problem" }]);
    expect(play([story, epic])).toEqual([{ type: "playSound", kind: "epic_done" }]);
    expect(play([epic, story])).toEqual([{ type: "playSound", kind: "epic_done" }]);
    s = window();
    expect(play([story])).toEqual([{ type: "playSound", kind: "story_done" }]);
  });

  it("when a story's finish is also its epic's finish only the epic's sound plays", () => {
    const s = window({ t1: "review" });
    const [held] = live(s, status("t1", "merged"));
    expect(reduce(held, { type: "soundDue" })[1]).toEqual([{ type: "playSound", kind: "epic_done" }]);
  });

  it("a kind whose switch is off never plays, the others still do", () => {
    const s = window(undefined, { story_done: false, epic_done: true, problem: false });
    const [a, fx] = live(s, status("t1", "merged"));
    expect(a.sound.due).toBeUndefined();
    expect(fx.some((f) => f.type === "soundWindow")).toBe(false);
    expect(live(s, blocked("t2", "crashed"))[0].sound.due).toBeUndefined();
    expect(live(s, run("run_done"))[0].sound.due).toBe("epic_done");
  });

  it("a burst waits COLLECT_MS (1.5 s) and the window is shorter than what a person needs between sounds", () => {
    expect(COLLECT_MS).toBe(1500);
  });
});

describe("the switches", () => {
  it("are all off before the daemon answers", () => {
    expect(initialState({ name: "chains" }).sound.switches).toEqual({ story_done: false, epic_done: false, problem: false });
  });
  it("come from the daemon and a change is saved by an effect", () => {
    const [loaded] = reduce(initialState({ name: "chains" }), { type: "soundsLoaded", sounds: on });
    expect(loaded.sound.switches).toEqual(on);
    const off = { ...on, problem: false };
    const [s, fx] = reduce(loaded, { type: "setSounds", sounds: off });
    expect(s.sound.switches).toEqual(off);
    expect(fx).toEqual([{ type: "saveSounds", sounds: off }]);
  });
  it("Play asks for the sound whatever the switch says", () => {
    const [, fx] = reduce(initialState({ name: "chains" }), { type: "playPreview", kind: "epic_done" });
    expect(fx).toEqual([{ type: "previewSound", kind: "epic_done" }]);
  });
  it("hear ignores an event that calls for no sound", () => {
    const s = window();
    expect(hear(s, status("t1", "running"), NOW)).toEqual([s, []]);
  });
});
