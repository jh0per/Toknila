import { describe, expect, it } from "vitest";
import { chooseRenderer } from "./renderer";

describe("chooseRenderer", () => {
  it("uses the canvas for people and the DOM for automated browsers", () => {
    expect(chooseRenderer(false, null)).toBe("webgl");
    expect(chooseRenderer(true, null)).toBe("dom");
  });
  it("lets the stored override win, and ignores junk", () => {
    expect(chooseRenderer(true, "webgl")).toBe("webgl");
    expect(chooseRenderer(false, "dom")).toBe("dom");
    expect(chooseRenderer(false, "gpu")).toBe("webgl");
  });
});
