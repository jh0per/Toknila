import { describe, expect, it } from "vitest";
import { MIN_ZOOM, fitZoom } from "./useFit";

describe("fitZoom", () => {
  it("is 100% when the diagram already fits", () => {
    expect(fitZoom({ w: 1200, h: 500 }, 1000, 400)).toBe(1);
  });

  it("shrinks to the tighter side", () => {
    expect(fitZoom({ w: 1000, h: 500 }, 2000, 400)).toBe(0.5);
    expect(fitZoom({ w: 2000, h: 300 }, 1000, 600)).toBe(0.5);
  });

  it("never goes below the readable minimum", () => {
    expect(fitZoom({ w: 100, h: 100 }, 5000, 5000)).toBe(MIN_ZOOM);
  });

  it("stays at 100% before the view is measured", () => {
    expect(fitZoom({ w: 0, h: 0 }, 2000, 800)).toBe(1);
  });
});
