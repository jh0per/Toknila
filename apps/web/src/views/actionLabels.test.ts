import { describe, expect, it } from "vitest";
import { specsFor } from "./actionLabels";

describe("specsFor", () => {
  it("makes a button of each action the daemon allows, in its order", () => {
    expect(specsFor(["accept", "return", "cancel"]).map((a) => a.label)).toEqual(["Accept", "Return", "Cancel"]);
    expect(specsFor([])).toEqual([]);
  });

  it("only a return asks for a comment, and cancel is the dangerous one", () => {
    const specs = specsFor(["accept", "return", "cancel"]);
    expect(specs.filter((a) => a.needsText).map((a) => a.action)).toEqual(["return"]);
    expect(specs.filter((a) => a.danger).map((a) => a.action)).toEqual(["cancel"]);
  });

  it("words the reviewer re-run", () => {
    expect(specsFor(["accept", "return", "run_reviewer"]).map((a) => a.label)).toEqual([
      "Accept", "Return", "Run the reviewer again",
    ]);
  });

  it("words the request for a design and keeps it a plain button", () => {
    const specs = specsFor(["pause", "ask_design", "cancel"]);
    expect(specs.map((a) => a.label)).toEqual(["Pause", "Ask for a design", "Cancel"]);
    expect(specs[1]).toEqual({ action: "ask_design", label: "Ask for a design" });
  });

  it("leaves out what the bar has no field for", () => {
    expect(specsFor(["pause", "block", "cancel"]).map((a) => a.action)).toEqual(["pause", "cancel"]);
  });
});
