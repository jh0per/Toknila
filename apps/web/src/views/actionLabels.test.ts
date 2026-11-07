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

  it("leaves out what the bar has no field for", () => {
    expect(specsFor(["pause", "block", "cancel"]).map((a) => a.action)).toEqual(["pause", "cancel"]);
  });
});
