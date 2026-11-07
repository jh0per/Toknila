import { describe, expect, it } from "vitest";
import type { ChainDetail, PlannerState, Project } from "../api/types";
import { plannerOf } from "./planner";
import { reduce } from "./reduce";
import { parseRoute, routePath } from "./route";
import { currentChain, initialState } from "./state";

const project = { id: "P1", key: "DEMO", name: "Demo", archived: false } as unknown as Project;
const chain = (id: string, name: string): ChainDetail =>
  ({ chain: { id, project_id: "P1", name }, tickets: [], edges: [], releases: [], order: [] }) as unknown as ChainDetail;
const idle: PlannerState = { running: false, run: null, profile: "claude-code-planner" };

const start = () => ({ ...initialState({ name: "chains" }), projects: [project], loaded: true });

describe("the planner entry", () => {
  it("has its own address under the project", () => {
    const route = { name: "planner", project: "DEMO" } as const;
    expect(routePath(route)).toBe("/projects/DEMO/planner");
    expect(parseRoute("/projects/DEMO/planner")).toEqual(route);
    // The other project routes are untouched.
    expect(parseRoute("/projects/DEMO")).toEqual({ name: "project", project: "DEMO" });
    expect(parseRoute("/projects/DEMO/settings")).toEqual({ name: "settings", project: "DEMO" });
  });

  it("asks whether the planner runs when it opens, and pushes the address", () => {
    const [s, fx] = reduce(start(), { type: "navigate", route: { name: "planner", project: "DEMO" } });
    expect(s.route).toEqual({ name: "planner", project: "DEMO" });
    expect(fx).toContainEqual({ type: "plannerCall", project: "DEMO", call: "load" });
    expect(fx).toContainEqual({ type: "pushUrl", path: "/projects/DEMO/planner" });
  });

  it("a deep link loads it too", () => {
    const [, fx] = reduce(start(), { type: "urlChanged", route: { name: "planner", project: "DEMO" } });
    expect(fx).toEqual([
      { type: "plannerCall", project: "DEMO", call: "load" },
      { type: "loadProfiles" },
    ]);
  });

  it("Start uses the picked planner profile", () => {
    const [picked] = reduce(start(), { type: "pickPlannerProfile", profile: "my-planner" });
    const [, fx] = reduce(picked, { type: "startPlanner", project: "DEMO" });
    expect(fx).toEqual([{ type: "plannerCall", project: "DEMO", call: "start", profile: "my-planner" }]);
  });

  it("Start and Stop are commands for the daemon, and its answer is kept", () => {
    const [s, fx] = reduce(start(), { type: "startPlanner", project: "DEMO" });
    expect(fx).toEqual([
      { type: "plannerCall", project: "DEMO", call: "start", profile: "claude-code-planner" },
    ]);
    const [running] = reduce(s, { type: "plannerLoaded", project: "DEMO", state: { ...idle, running: true } });
    expect(plannerOf(running, "DEMO")?.running).toBe(true);
    const [, stop] = reduce(running, { type: "stopPlanner", project: "DEMO" });
    expect(stop).toEqual([{ type: "plannerCall", project: "DEMO", call: "stop" }]);
    expect(plannerOf(start(), "DEMO")).toBeUndefined();
  });

  it("shows the daemon's refusal until the next command, and a finished run is asked about again", () => {
    const [refused] = reduce(start(), { type: "plannerFailed", error: "project DEMO has no repository" });
    expect(refused.planner.error).toBe("project DEMO has no repository");
    const [again] = reduce(refused, { type: "startPlanner", project: "DEMO" });
    expect(again.planner.error).toBe("");
    const [, fx] = reduce(again, { type: "refreshPlanner", project: "DEMO" });
    expect(fx).toEqual([{ type: "plannerCall", project: "DEMO", call: "load" }]);
  });

  it("shows the project's most recently active chain above, so the diagram follows the planner", () => {
    let s = { ...start(), route: { name: "planner", project: "DEMO" } as const };
    s = { ...s, chains: { A: chain("A", "old"), B: chain("B", "new") }, chainOrder: ["A", "B"] };
    expect(currentChain(s)?.chain.name).toBe("old");
    // The planner edits chain B: its journal event makes it the active one.
    s = {
      ...s,
      events: [{ seq: 1, entity: "chain", kind: "updated", entity_id: "B", payload: { chain: { id: "B" } } }] as never,
    };
    expect(currentChain(s)?.chain.name).toBe("new");
  });
});
