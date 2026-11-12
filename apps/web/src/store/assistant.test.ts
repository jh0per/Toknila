import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ConversationMessage } from "../api/types";
import { aboutThis, chipOf, contextLabel, contextOf, lineFor, parseAssistant } from "./assistant";
import { ASSISTANT_KEY, readAssistant, saveAssistant } from "./assistantEffects";
import type { Msg } from "./msg";
import { reduce } from "./reduce";
import type { Route } from "./route";
import { type State, initialState } from "./state";

const at = (route: Route): State => ({ ...initialState(route), loaded: true });
const apply = (s: State, ...msgs: Msg[]): State => msgs.reduce((state, m) => reduce(state, m)[0], s);
const typed = (s: State, text: string): State => apply(s, { type: "editPlannerDraft", where: "DEMO", text });
const msg = (id: string, about?: ConversationMessage["about"]): ConversationMessage =>
  ({ id, author: "person", text: id, at: 1, about: about ?? null }) as unknown as ConversationMessage;

/// A browser in miniature: a storage that keeps what is written.
const kept = new Map<string, string>();
beforeEach(() => {
  kept.clear();
  vi.stubGlobal("window", { localStorage: { getItem: (k: string) => kept.get(k) ?? null, setItem: (k: string, v: string) => kept.set(k, v) } });
});
afterEach(() => vi.unstubAllGlobals());

describe("what is open is the context chip", () => {
  it("is the roadmap on a project's page, the chain on a chain and the card on a card", () => {
    expect(contextOf({ name: "project", project: "DEMO" })).toEqual({ kind: "roadmap" });
    expect(contextOf({ name: "projectChain", project: "DEMO", chain: "plan" })).toEqual({ kind: "chain", name: "plan" });
    expect(contextOf({ name: "card", project: "DEMO", chain: "plan", key: "DEMO-1" })).toEqual({ kind: "card", key: "DEMO-1" });
    expect(contextOf({ name: "planner", project: "DEMO" })).toBeUndefined();
    expect(contextOf({ name: "inbox" })).toBeUndefined();
  });

  it("goes when the person takes it off and comes back when something else opens", () => {
    const card: Route = { name: "card", project: "DEMO", chain: "plan", key: "DEMO-1" };
    const off = apply(at(card), { type: "removeChip" });
    expect(chipOf(off)).toBeUndefined();
    const other = apply(off, { type: "navigate", route: { name: "card", project: "DEMO", chain: "plan", key: "DEMO-2" } });
    expect(chipOf(other)).toEqual({ kind: "card", key: "DEMO-2" });
  });
});

describe("what a message sent with a chip is", () => {
  const open = (route: Route) => apply(at(route), { type: "toggleAssistant" });

  it("about a chain is typed `About epic NAME: …` and stored with the chain", () => {
    const [, fx] = reduce(typed(open({ name: "projectChain", project: "DEMO", chain: "plan" }), "reorder\nthe cards"), { type: "askPlanner", project: "DEMO" });
    expect(fx[0]).toMatchObject({
      call: "ask", text: "About epic plan: reorder the cards", raw: "reorder\nthe cards", about: { kind: "chain", name: "plan" },
    });
  });

  it("about the roadmap is typed `About the roadmap: …` and stored about the roadmap", () => {
    const [, fx] = reduce(typed(open({ name: "project", project: "DEMO" }), "add an epic"), { type: "askPlanner", project: "DEMO" });
    expect(fx[0]).toMatchObject({ call: "ask", text: "About the roadmap: add an epic", raw: "add an epic" });
    expect(fx[0]).toHaveProperty("about", { kind: "roadmap" });
  });

  it("is plain without a chip", () => {
    const off = apply(open({ name: "project", project: "DEMO" }), { type: "removeChip" });
    expect(lineFor({ kind: "chain", name: "x" }, " a  ")).toBe("About epic x: a");
    const [, fx] = reduce(typed(off, "hi"), { type: "askPlanner", project: "DEMO" });
    expect(fx[0]).toMatchObject({ call: "ask", text: "hi" });
  });

  it("asked on the roadmap with the panel shut opens the panel there, and remembers it", () => {
    const [s, fx] = reduce(typed(at({ name: "project", project: "DEMO" }), "hi"), { type: "askPlanner", project: "DEMO" });
    expect(s.route).toEqual({ name: "project", project: "DEMO" });
    expect(s.assistant.open).toBe(true);
    expect(fx).toContainEqual({ type: "saveAssistant", open: true, width: 440, only: false });
  });

  it("asked from a screen the panel cannot sit beside opens the panel on the roadmap", () => {
    const [s] = reduce(typed(at({ name: "inbox" } as Route), "hi"), { type: "askPlanner", project: "DEMO" });
    expect(s.route).toEqual({ name: "project", project: "DEMO" });
    expect(s.assistant.open).toBe(true);
  });

  it("names the epic and the card in the chip", () => {
    expect(contextLabel({ kind: "roadmap" })).toBe("Roadmap");
    expect(contextLabel({ kind: "chain", name: "phase-7" })).toBe("the epic phase-7");
    expect(contextLabel({ kind: "card", key: "DEMO-1" })).toBe("the story DEMO-1");
  });

  it("openAssistant opens the panel once and leaves an open one alone", () => {
    const [s, fx] = reduce(at({ name: "project", project: "DEMO" }), { type: "openAssistant" });
    expect(s.assistant.open).toBe(true);
    expect(fx).toHaveLength(1);
    expect(reduce(s, { type: "openAssistant" })[1]).toEqual([]);
  });
});

describe("only about this", () => {
  const all = [msg("a"), msg("b", { kind: "card", key: "DEMO-1" }), msg("c", { kind: "chain", name: "plan" }), msg("d", { kind: "card", key: "DEMO-2" }), msg("e", { kind: "roadmap" })];
  it("keeps the messages of the open card", () => expect(aboutThis(all, { kind: "card", key: "DEMO-1" }).map((m) => m.id)).toEqual(["b"]));
  it("keeps the messages of the open chain", () => expect(aboutThis(all, { kind: "chain", name: "plan" }).map((m) => m.id)).toEqual(["c"]));
  it("keeps the messages about the roadmap on the roadmap", () => expect(aboutThis(all, { kind: "roadmap" }).map((m) => m.id)).toEqual(["e"]));
  it("is off by default and the choice is remembered", () => {
    const s = at({ name: "project", project: "DEMO" });
    expect(s.assistant.only).toBe(false);
    const [on, fx] = reduce(s, { type: "setOnlyAboutThis", only: true });
    expect(on.assistant.only).toBe(true);
    expect(fx).toEqual([{ type: "saveAssistant", open: false, width: 440, only: true }]);
  });
});

describe("the panel's state survives a reload", () => {
  it("opens, resizes within its limits and is stored and read back", () => {
    const s = apply(at({ name: "project", project: "DEMO" }), { type: "toggleAssistant" }, { type: "setAssistantWidth", width: 9000 });
    expect(s.assistant).toMatchObject({ open: true, width: 720 });
    saveAssistant(s.assistant);
    expect(readAssistant()).toMatchObject({ open: true, width: 720, only: false });
    expect(reduce(s, { type: "closeAssistant" })[0].assistant.open).toBe(false);
  });

  it("falls back to the default for a value that is not readable", () => {
    kept.set(ASSISTANT_KEY, "{nope");
    expect(readAssistant()).toEqual({ open: false, width: 440, only: false, chipOff: false });
    expect(parseAssistant(JSON.stringify({ open: true, width: 10 })).width).toBe(320);
  });
});
