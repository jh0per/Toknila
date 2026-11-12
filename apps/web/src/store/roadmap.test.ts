import { describe, expect, it } from "vitest";
import type { ChainDetail } from "../api/types";
import { dropMove, moveStep, roadmapOrder, roadmapPhase, splitRoadmap } from "./roadmap";
import { reduce } from "./reduce";
import { initialState } from "./state";

const chain = (id: string, position: number, phase = "idle", finish?: { state: string }): ChainDetail =>
  ({
    chain: { id, name: id, goal: "", autostart: true, project_id: "p1", position },
    tickets: [], releases: [], edges: [], release_tickets: [], order: [],
    summary: { status: "idle", phase, finished_at: null, total: 4, done: 1, released: 0, releasable: false, finish },
  }) as unknown as ChainDetail;

const ids = (cs: ChainDetail[]) => cs.map((c) => c.chain.id);

describe("the roadmap order", () => {
  it("follows the position and keeps the list's order for equal ones", () => {
    expect(ids(roadmapOrder([chain("c", 2), chain("a", 0), chain("b", 1)]))).toEqual(["a", "b", "c"]);
    expect(ids(roadmapOrder([chain("x", 0), chain("y", 0)]))).toEqual(["x", "y"]);
  });
  it("folds the finished epics out of the sequence", () => {
    const { finished, rest } = splitRoadmap([chain("a", 0, "finished"), chain("b", 1, "active"), chain("c", 2, "finished"), chain("d", 3)]);
    expect(ids(finished)).toEqual(["a", "c"]);
    expect(ids(rest)).toEqual(["b", "d"]);
  });
  it("names the phase of an epic", () => {
    expect(roadmapPhase({ phase: "idle" })).toBe("planned");
    expect(roadmapPhase({ phase: "empty" })).toBe("planned");
    expect(roadmapPhase({ phase: "waiting" })).toBe("active");
    expect(roadmapPhase({ phase: "active" })).toBe("active");
    expect(roadmapPhase({ phase: "finished" })).toBe("finished");
    expect(roadmapPhase({ phase: "finished", finish: { state: "released" } } as never)).toBe("released");
  });
});

describe("moving an epic", () => {
  const shown = [chain("a", 0), chain("b", 1), chain("c", 2)];
  it("a step goes before the previous or after the next one, and stops at the ends", () => {
    expect(moveStep(shown, "b", -1)).toEqual({ before: "a", after: null });
    expect(moveStep(shown, "b", 1)).toEqual({ before: null, after: "c" });
    expect(moveStep(shown, "a", -1)).toBeUndefined();
    expect(moveStep(shown, "c", 1)).toBeUndefined();
  });
  it("a drop puts the epic where the other one was", () => {
    expect(dropMove(shown, "a", "c")).toEqual({ before: null, after: "c" });
    expect(dropMove(shown, "c", "a")).toEqual({ before: "a", after: null });
    expect(dropMove(shown, "b", "b")).toBeUndefined();
    expect(dropMove(shown, "b", "zzz")).toBeUndefined();
  });
  it("a move is sent, and its answer reloads every chain; a refusal is shown as said", () => {
    const base = { ...initialState({ name: "project", project: "DEMO" }), chains: { a: chain("a", 0) }, chainOrder: ["a"] };
    const move = { type: "moveChain" as const, chain: "a", project: "DEMO", to: { before: null, after: "b" } };
    const [, fx] = reduce(base, move);
    expect(fx).toEqual([move]);
    const [moved, more] = reduce(base, { type: "chainMoved", chain: chain("a", 1) });
    expect(moved.chains.a.chain.position).toBe(1);
    expect(more).toEqual([{ type: "loadAll" }]);
    const [refused] = reduce(base, { type: "chainMoveRefused", message: "the chain is not in this project's roadmap" });
    expect(refused.roadmap.error).toBe("the chain is not in this project's roadmap");
  });
});
