import { describe, expect, it } from "vitest";
import type { PlannerState, Project } from "../api/types";
import type { Msg } from "./msg";
import { ONBOARDING_DRAFT, draftOf, oneLine, plannerOf, showsTerminal } from "./planner";
import { reduce } from "./reduce";
import { type State, initialState } from "./state";

const project = { id: "P1", key: "DEMO", name: "Demo", archived: false } as unknown as Project;
const idle: PlannerState = { running: false, run: null, profile: "claude-code-planner" };
const running: PlannerState = { running: true, run: "R1", profile: "claude-code-planner" };

const start = (): State => ({ ...initialState({ name: "project", project: "DEMO" }), projects: [project], loaded: true });
const apply = (s: State, ...msgs: Msg[]): State => msgs.reduce((state, m) => reduce(state, m)[0], s);

describe("talking to the assistant", () => {
  it("keeps what is typed per place", () => {
    const s = apply(
      start(),
      { type: "editPlannerDraft", where: "DEMO", text: "a shop" },
      { type: "editPlannerDraft", where: ONBOARDING_DRAFT, text: "my first project" },
    );
    expect(draftOf(s, "DEMO")).toBe("a shop");
    expect(draftOf(s, ONBOARDING_DRAFT)).toBe("my first project");
    expect(draftOf(s, "OTHER")).toBe("");
  });

  it("sends the draft: starts the planner, waits for its screen, types the text on one line", () => {
    const typed = apply(start(), { type: "editPlannerDraft", where: "DEMO", text: "  a shop\nwith a cart  " });
    const [s, fx] = reduce(typed, { type: "askPlanner", project: "DEMO" });
    expect(fx).toContainEqual({
      type: "plannerCall", project: "DEMO", call: "ask", profile: "claude-code-planner", text: "a shop with a cart", wait: true,
    });
    expect(s.planner.sending).toBe("DEMO");
    // Asked from the project page: the conversation moves to the assistant's screen.
    expect(s.route).toEqual({ name: "planner", project: "DEMO" });
    expect(fx).toContainEqual({ type: "pushUrl", path: "/projects/DEMO/planner" });
    // The draft stays until the text has reached the planner, so a refusal does not lose it.
    expect(draftOf(s, "DEMO")).toBe("  a shop\nwith a cart  ");
    const [sent] = reduce(s, { type: "plannerAsked", project: "DEMO" });
    expect(sent.planner.sending).toBeUndefined();
    expect(draftOf(sent, "DEMO")).toBe("");
  });

  it("a planner that already runs gets the text at once and the page stays where it is", () => {
    const base = { ...start(), route: { name: "planner", project: "DEMO" } as const };
    const live = apply(base, { type: "plannerLoaded", project: "DEMO", state: running });
    const typed = apply(live, { type: "editPlannerDraft", where: "DEMO", text: "add a login" });
    const [, fx] = reduce(typed, { type: "askPlanner", project: "DEMO" });
    expect(fx).toEqual([
      { type: "plannerCall", project: "DEMO", call: "ask", profile: "claude-code-planner", text: "add a login", wait: false },
    ]);
  });

  it("sends nothing for an empty box, and one message at a time", () => {
    const [none, fx] = reduce(start(), { type: "askPlanner", project: "DEMO" });
    expect(fx).toEqual([]);
    expect(none.planner.sending).toBeUndefined();
    const blank = apply(start(), { type: "editPlannerDraft", where: "DEMO", text: " \n " });
    const [same, fx2] = reduce(blank, { type: "askPlanner", project: "DEMO" });
    expect(fx2).toEqual([]);
    expect(same.planner.sending).toBeUndefined();
    const typed = apply(start(), { type: "editPlannerDraft", where: "DEMO", text: "one" });
    const [busy] = reduce(typed, { type: "askPlanner", project: "DEMO" });
    const [, again] = reduce(busy, { type: "askPlanner", project: "DEMO" });
    expect(again).toEqual([]);
  });

  it("uses the picked planner profile, and a refusal frees the box and says why", () => {
    const typed = apply(
      start(),
      { type: "pickPlannerProfile", profile: "my-planner" },
      { type: "editPlannerDraft", where: "DEMO", text: "hello" },
    );
    const [s, fx] = reduce(typed, { type: "askPlanner", project: "DEMO" });
    expect(fx[0]).toMatchObject({ call: "ask", profile: "my-planner" });
    const [failed] = reduce(s, { type: "plannerFailed", error: "project DEMO has no repository" });
    expect(failed.planner.sending).toBeUndefined();
    expect(failed.planner.error).toBe("project DEMO has no repository");
    expect(draftOf(failed, "DEMO")).toBe("hello");
  });

  it("puts a message on one line", () => {
    expect(oneLine("  a\n\n  b \n c ")).toBe("a b c");
    expect(oneLine("plain")).toBe("plain");
  });

  it("keeps the terminal as the record once it has been seen running", () => {
    const s = start();
    expect(showsTerminal(s, "DEMO")).toBe(false);
    const idleSeen = apply(s, { type: "plannerLoaded", project: "DEMO", state: idle });
    expect(showsTerminal(idleSeen, "DEMO")).toBe(false);
    const live = apply(idleSeen, { type: "plannerLoaded", project: "DEMO", state: running });
    expect(showsTerminal(live, "DEMO")).toBe(true);
    const ended = apply(live, { type: "plannerLoaded", project: "DEMO", state: idle });
    expect(plannerOf(ended, "DEMO")?.running).toBe(false);
    expect(showsTerminal(ended, "DEMO")).toBe(true);
  });
});

describe("the first project leads into the assistant", () => {
  const added = { ...project, id: "P9", key: "NEW" } as unknown as Project;
  const onboarding = (): State => ({ ...initialState({ name: "chains" }), loaded: true });

  it("opens the assistant with what was described", () => {
    const typed = apply(onboarding(), { type: "editPlannerDraft", where: ONBOARDING_DRAFT, text: "a recipe app" });
    const [s, fx] = reduce(typed, { type: "projectAdded", project: added });
    expect(s.route).toEqual({ name: "planner", project: "NEW" });
    expect(fx).toContainEqual({
      type: "plannerCall", project: "NEW", call: "ask", profile: "claude-code-planner", text: "a recipe app", wait: true,
    });
    expect(s.planner.drafts[ONBOARDING_DRAFT]).toBeUndefined();
    expect(draftOf(s, "NEW")).toBe("a recipe app");
  });

  it("opens the assistant's screen even when nothing was described", () => {
    const [s, fx] = reduce(onboarding(), { type: "projectAdded", project: added });
    expect(s.route).toEqual({ name: "planner", project: "NEW" });
    expect(fx.some((e) => e.type === "plannerCall" && e.call === "ask")).toBe(false);
    expect(fx).toContainEqual({ type: "loadPlans", project: "NEW", reason: "added" });
  });

  it("a project added next goes to its settings, as before", () => {
    const [s] = reduce({ ...onboarding(), projects: [project] }, { type: "projectAdded", project: added });
    expect(s.route).toEqual({ name: "settings", project: "NEW" });
  });
});
