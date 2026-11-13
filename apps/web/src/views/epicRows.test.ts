import { describe, expect, it } from "vitest";
import { dropTarget, epicRowOf } from "./epicRows";

/// An element inside a sidebar row (or not): `closest` finds the row the way the DOM does.
const inRow = (chain: string, project: string) => ({
  closest: (sel: string) =>
    sel === "[data-chain-id]"
      ? { getAttribute: (n: string) => (n === "data-chain-id" ? chain : n === "data-chain-project" ? project : null) }
      : null,
});
const outside = { closest: () => null };

describe("epicRowOf", () => {
  it("reads the epic and the project of the row under the pointer", () => {
    expect(epicRowOf(inRow("c2", "DEMO"))).toEqual({ chain: "c2", project: "DEMO" });
  });
  it("finds nothing outside a row or without an element", () => {
    expect(epicRowOf(outside)).toBeUndefined();
    expect(epicRowOf(null)).toBeUndefined();
  });
});

describe("dropTarget", () => {
  it("is the row's epic when it is another epic of the card's own project", () => {
    expect(dropTarget(inRow("c2", "DEMO"), "DEMO", "c1")).toBe("c2");
  });
  it("is nothing on the card's own epic, on another project's row, or off the rows", () => {
    expect(dropTarget(inRow("c1", "DEMO"), "DEMO", "c1")).toBeUndefined();
    expect(dropTarget(inRow("c9", "OTHER"), "DEMO", "c1")).toBeUndefined();
    expect(dropTarget(outside, "DEMO", "c1")).toBeUndefined();
  });
});
