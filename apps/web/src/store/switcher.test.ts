import { describe, expect, it } from "vitest";
import type { Project } from "../api/types";
import { reduce } from "./reduce";
import { initialState } from "./state";
import { SIDEBAR_OTHERS, asksForSwitcher, filterProjects, sidebarOthers, switchRoute } from "./switcher";

const p = (key: string, name: string) => ({ id: `id-${key}`, key, name, archived: false }) as unknown as Project;
const press = (key: string, over: Partial<{ ctrlKey: boolean; metaKey: boolean; altKey: boolean; shiftKey: boolean }> = {}) => ({
  key, ctrlKey: false, metaKey: false, altKey: false, shiftKey: false, ...over,
});

describe("switchRoute: the screen kind is kept across projects", () => {
  it("keeps the roadmap, the assistant, the inbox and the settings", () => {
    expect(switchRoute({ name: "project", project: "AAA" }, "BBB")).toEqual({ name: "project", project: "BBB" });
    expect(switchRoute({ name: "planner", project: "AAA" }, "BBB")).toEqual({ name: "planner", project: "BBB" });
    expect(switchRoute({ name: "inbox", project: "AAA" }, "BBB")).toEqual({ name: "inbox", project: "BBB" });
    expect(switchRoute({ name: "inbox" }, "BBB")).toEqual({ name: "inbox", project: "BBB" });
    expect(switchRoute({ name: "settings", project: "AAA" }, "BBB")).toEqual({ name: "settings", project: "BBB" });
  });
  it("keeps a project's settings section, drops an app one", () => {
    expect(switchRoute({ name: "settings", project: "AAA", section: "designs" }, "BBB")).toEqual({
      name: "settings", project: "BBB", section: "designs",
    });
    expect(switchRoute({ name: "settings", section: "about" }, "BBB")).toEqual({ name: "settings", project: "BBB" });
  });
  it("opens the roadmap where the screen is one project's own", () => {
    expect(switchRoute({ name: "chain", chain: "x" }, "BBB")).toEqual({ name: "project", project: "BBB" });
    expect(switchRoute({ name: "card", project: "AAA", chain: "x", key: "AAA-1" }, "BBB")).toEqual({ name: "project", project: "BBB" });
    expect(switchRoute({ name: "usage" }, "BBB")).toEqual({ name: "project", project: "BBB" });
  });
});

describe("filterProjects", () => {
  const all = [p("BEA", "Beatster"), p("TOK", "Toknila"), p("LAB", "Lab notes")];
  it("keeps all for a blank text and matches name or key, ignoring case", () => {
    expect(filterProjects(all, "  ")).toEqual(all);
    expect(filterProjects(all, "toK").map((x) => x.key)).toEqual(["TOK"]);
    expect(filterProjects(all, "lab").map((x) => x.key)).toEqual(["LAB"]);
    expect(filterProjects(all, "t").map((x) => x.key)).toEqual(["BEA", "TOK", "LAB"]);
    expect(filterProjects(all, "zzz")).toEqual([]);
  });
});

describe("the sidebar's other projects", () => {
  const many = (n: number) => Array.from({ length: n }, (_, i) => p(`P${i}`, `Project ${i}`));
  it("lists up to three, then folds the rest into a count", () => {
    expect(sidebarOthers(many(3))).toEqual({ shown: many(3), more: 0 });
    const four = sidebarOthers(many(4));
    expect(four.shown.map((x) => x.key)).toEqual(["P0", "P1", "P2"]);
    expect(four.more).toBe(1);
    expect(sidebarOthers(many(SIDEBAR_OTHERS + 5)).more).toBe(5);
  });
});

describe("the key and the state", () => {
  it("is Cmd or Ctrl plus K, and nothing else", () => {
    expect(asksForSwitcher(press("k", { metaKey: true }))).toBe(true);
    expect(asksForSwitcher(press("K", { ctrlKey: true }))).toBe(true);
    expect(asksForSwitcher(press("k"))).toBe(false);
    expect(asksForSwitcher(press("k", { ctrlKey: true, shiftKey: true }))).toBe(false);
    expect(asksForSwitcher(press("p", { metaKey: true }))).toBe(false);
  });
  it("opens, toggles and closes", () => {
    const s0 = initialState({ name: "chains" });
    expect(s0.switcherOpen).toBe(false);
    const [open] = reduce(s0, { type: "toggleSwitcher" });
    expect(open.switcherOpen).toBe(true);
    expect(reduce(open, { type: "toggleSwitcher" })[0].switcherOpen).toBe(false);
    expect(reduce(open, { type: "closeSwitcher" })[0].switcherOpen).toBe(false);
  });
});
