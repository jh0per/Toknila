import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { agentsOf, inboxOf, runRouteIn, projectOpen, readProjectOpen, stepIndex, storeProjectOpen, treeMove, waitingIn } from "./tree";

const memory = () => {
  const data = new Map<string, string>();
  return { getItem: (k: string) => data.get(k) ?? null, setItem: (k: string, v: string) => void data.set(k, v) };
};

describe("the open state of a project", () => {
  beforeEach(() => vi.stubGlobal("window", { localStorage: memory() }));
  afterEach(() => vi.unstubAllGlobals());

  it("is undefined until the person opens or closes it, then remembered per project", () => {
    expect(readProjectOpen("A")).toBeUndefined();
    storeProjectOpen("A", true);
    storeProjectOpen("B", false);
    expect(readProjectOpen("A")).toBe(true);
    expect(readProjectOpen("B")).toBe(false);
    expect(readProjectOpen("C")).toBeUndefined();
  });
  it("survives a reload: a new window over the same storage reads the same", () => {
    const store = memory();
    vi.stubGlobal("window", { localStorage: store });
    storeProjectOpen("A", true);
    vi.stubGlobal("window", { localStorage: store });
    expect(readProjectOpen("A")).toBe(true);
  });
  it("reads as untouched and does not throw when storage is blocked", () => {
    vi.stubGlobal("window", { get localStorage(): never { throw new Error("blocked"); } });
    expect(readProjectOpen("A")).toBeUndefined();
    expect(() => storeProjectOpen("A", true)).not.toThrow();
  });
});

describe("projectOpen", () => {
  it("is open for the project on the screen whatever was stored", () => {
    expect(projectOpen(false, true)).toBe(true);
    expect(projectOpen(undefined, true)).toBe(true);
  });
  it("follows the stored state for any other, closed when none", () => {
    expect(projectOpen(true, false)).toBe(true);
    expect(projectOpen(false, false)).toBe(false);
    expect(projectOpen(undefined, false)).toBe(false);
  });
});

describe("waitingIn", () => {
  const s = {
    inbox: [{ project: "pa" }, { chain: "cb" }, { chain: "cb" }, { chain: "unknown" }, {}],
    chains: { cb: { chain: { project_id: "pb" } } },
  } as never;
  it("counts the items of the project, by its own field or by the chain's project", () => {
    expect(waitingIn(s, { id: "pa" })).toBe(1);
    expect(waitingIn(s, { id: "pb" })).toBe(2);
    expect(waitingIn(s, { id: "pc" })).toBe(0);
  });
});

describe("inboxOf and agentsOf", () => {
  const s = {
    inbox: [{ kind: "question", project: "pa" }, { kind: "blocked", chain: "cb" }],
    chains: { cb: { chain: { project_id: "pb", name: "cb" }, tickets: [{ id: "t1" }] } },
    agents: [{ project: "pb", used: 1, limit: 3, runs: [{ card: { id: "t1", key: "B-1", title: "x" }, role: "worker" }, { card: null, role: "planner" }] }],
  } as never;
  it("lists the items of one project", () => {
    expect(inboxOf(s, { id: "pb" }).map((i) => i.kind)).toEqual(["blocked"]);
    expect(inboxOf(s, { id: "pa" }).map((i) => i.kind)).toEqual(["question"]);
  });
  it("reports runs, used and limit of a project; a project without a report runs nothing", () => {
    expect(agentsOf(s, { id: "pb" })).toMatchObject({ used: 1, limit: 3 });
    expect(agentsOf(s, { id: "pb" }).runs).toHaveLength(2);
    expect(agentsOf(s, { id: "pa", max_parallel: 2 })).toEqual({ runs: [], used: 0, limit: 2 });
  });
  it("a run's route is its card in its own project, or the assistant", () => {
    const [card, assistant] = agentsOf(s, { id: "pb" }).runs;
    expect(runRouteIn(s, { key: "BBB" }, card)).toEqual({ name: "card", project: "BBB", chain: "cb", key: "B-1" });
    expect(runRouteIn(s, { key: "BBB" }, assistant)).toEqual({ name: "planner", project: "BBB" });
  });
});

describe("the keys of the tree", () => {
  it("Up and Down step, Home and End jump", () => {
    expect(treeMove("ArrowDown", undefined)).toBe("next");
    expect(treeMove("ArrowUp", true)).toBe("prev");
    expect(treeMove("Home", false)).toBe("first");
    expect(treeMove("End", false)).toBe("last");
  });
  it("Right opens a closed node and enters an open one; a leaf does nothing", () => {
    expect(treeMove("ArrowRight", false)).toBe("open");
    expect(treeMove("ArrowRight", true)).toBe("child");
    expect(treeMove("ArrowRight", undefined)).toBeUndefined();
  });
  it("Left closes an open node, else goes to the parent", () => {
    expect(treeMove("ArrowLeft", true)).toBe("close");
    expect(treeMove("ArrowLeft", false)).toBe("parent");
    expect(treeMove("ArrowLeft", undefined)).toBe("parent");
  });
  it("ignores other keys", () => expect(treeMove("a", true)).toBeUndefined());
  it("steps inside the rows and stops at the ends", () => {
    expect(stepIndex("next", 0, 3)).toBe(1);
    expect(stepIndex("next", 2, 3)).toBe(2);
    expect(stepIndex("prev", 0, 3)).toBe(0);
    expect(stepIndex("prev", 2, 3)).toBe(1);
    expect(stepIndex("first", 2, 3)).toBe(0);
    expect(stepIndex("last", 0, 3)).toBe(2);
    expect(stepIndex("open", 0, 3)).toBeUndefined();
    expect(stepIndex("next", 0, 0)).toBeUndefined();
  });
});
