import { describe, expect, it } from "vitest";
import type { EventRecord, Project } from "../api/types";
import { followTarget } from "./follow";
import { reduce } from "./reduce";
import type { Route } from "./route";
import { type State, initialState } from "./state";

const projects = [
  { id: "p1", key: "DEMO", name: "Demo" },
  { id: "p2", key: "OTHER", name: "Other" },
] as Project[];

/// A window that has loaded two projects and looks at `route`; the journal is at seq 10.
function at(route: Route): State {
  const [s] = reduce(initialState(route), { type: "loaded", projects, chains: [] });
  return { ...s, lastSeq: 10 };
}

const show = (seq: number, project = "p1", ticket = "DEMO-2"): EventRecord => ({
  seq, at: 0, actor: "agent", entity: "window", entity_id: project, kind: "show_card",
  payload: { project, project_key: project === "p1" ? "DEMO" : "OTHER", chain: "plan", ticket },
});

const card: Route = { name: "card", project: "DEMO", chain: "plan", key: "DEMO-2" };
const pushed = (fx: { type: string }[]) => fx.find((e) => e.type === "pushUrl");

describe("the assistant opens a card", () => {
  it("shows the card on the assistant's screen, with a notice that leads Back", () => {
    const here: Route = { name: "planner", project: "DEMO" };
    const [s, fx] = reduce(at(here), { type: "event", event: show(11) });
    expect(s.route).toEqual(card);
    expect(s.opened).toEqual({ key: "DEMO-2", seq: 11, back: here });
    expect(pushed(fx)).toEqual({ type: "pushUrl", path: "/projects/DEMO/chains/plan/cards/DEMO-2" });
    expect(fx).toContainEqual({ type: "loadDetail", key: "DEMO-2" });
    expect(fx).toContainEqual({ type: "expireOpened", seq: 11 });
    const [back, fx2] = reduce(s, { type: "backFromOpened" });
    expect(back.route).toEqual(here);
    expect(back.opened).toBeUndefined();
    expect(pushed(fx2)).toEqual({ type: "pushUrl", path: "/projects/DEMO/planner" });
  });

  it("follows from an open chain too", () => {
    const here: Route = { name: "projectChain", project: "DEMO", chain: "plan" };
    const [s] = reduce(at(here), { type: "event", event: show(11) });
    expect(s.route).toEqual(card);
    expect(s.opened?.back).toEqual(here);
  });

  it("ignores it in a window that looks at another project, but still records it", () => {
    for (const here of [{ name: "planner", project: "OTHER" }, { name: "projectChain", project: "OTHER", chain: "x" }] as Route[]) {
      const start = at(here);
      const [s, fx] = reduce(start, { type: "event", event: show(11) });
      expect(s.route).toEqual(here);
      expect(s.opened).toBeUndefined();
      expect(fx).toEqual([]);
      expect(s.lastSeq).toBe(11);
      expect(s.events[s.events.length - 1]?.kind).toBe("show_card");
    }
    // The control: the same event moves a window that does look at that project.
    expect(reduce(at({ name: "planner", project: "DEMO" }), { type: "event", event: show(11) })[0].route).toEqual(card);
  });

  it("ignores it on screens that are not the assistant's or a chain's", () => {
    for (const here of [{ name: "inbox" }, { name: "usage" }, { name: "settings" }, { name: "chains" }, { name: "project", project: "DEMO" }] as Route[]) {
      expect(followTarget(at(here), show(11)), here.name).toBeUndefined();
    }
  });

  it("follows once per event: a replayed seq changes nothing", () => {
    const [s] = reduce(at({ name: "planner", project: "DEMO" }), { type: "event", event: show(11) });
    const [back] = reduce(s, { type: "backFromOpened" });
    const [again, fx] = reduce(back, { type: "event", event: show(11) });
    expect(again).toBe(back);
    expect(fx).toEqual([]);
  });

  it("does not follow the events of the journal tail read at load", () => {
    const [s, fx] = reduce(initialState({ name: "planner", project: "DEMO" }), { type: "journalTail", seq: 11, events: [show(11)] });
    expect(s.route).toEqual({ name: "planner", project: "DEMO" });
    expect(s.opened).toBeUndefined();
    expect(fx).toEqual([{ type: "connect", since: 11, delayMs: 0 }]);
  });

  it("a second card keeps the first Back; the same card again does nothing", () => {
    const here: Route = { name: "planner", project: "DEMO" };
    const [one] = reduce(at(here), { type: "event", event: show(11) });
    const [two] = reduce(one, { type: "event", event: show(12, "p1", "DEMO-3") });
    expect(two.route).toMatchObject({ name: "card", key: "DEMO-3" });
    expect(two.opened).toEqual({ key: "DEMO-3", seq: 12, back: here });
    const [same, fx] = reduce(two, { type: "event", event: show(13, "p1", "DEMO-3") });
    expect(same.route).toEqual(two.route);
    expect(same.opened).toEqual(two.opened);
    expect(pushed(fx)).toBeUndefined();
  });

  it("the person going elsewhere ends the notice; a late timer of an older one does not end a newer", () => {
    const [s] = reduce(at({ name: "planner", project: "DEMO" }), { type: "event", event: show(11) });
    expect(reduce(s, { type: "navigate", route: { name: "inbox" } })[0].opened).toBeUndefined();
    expect(reduce(s, { type: "urlChanged", route: { name: "inbox" } })[0].opened).toBeUndefined();
    expect(reduce(s, { type: "openedExpired", seq: 5 })[0].opened).toBeDefined();
    expect(reduce(s, { type: "openedExpired", seq: 11 })[0].opened).toBeUndefined();
    expect(reduce(s, { type: "dismissOpened" })[0].opened).toBeUndefined();
  });

  it("a gap in the journal still reloads, and the card is still opened", () => {
    const [s, fx] = reduce(at({ name: "planner", project: "DEMO" }), { type: "event", event: show(15) });
    expect(s.route).toEqual(card);
    expect(fx).toContainEqual({ type: "loadAll" });
  });
});
