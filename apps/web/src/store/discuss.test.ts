import { describe, expect, it } from "vitest";
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
