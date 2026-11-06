import { describe, expect, it } from "vitest";
import { outOfOrderText } from "./format";

describe("outOfOrderText", () => {
  it("names the earlier cards that were still open", () => {
    expect(outOfOrderText({ behind: ["DEMO-1", "DEMO-2"] })).toBe("done out of order: DEMO-1, DEMO-2 still open");
  });
  it("is empty without the mark", () => {
    expect(outOfOrderText(null)).toBe("");
    expect(outOfOrderText(undefined)).toBe("");
  });
});
