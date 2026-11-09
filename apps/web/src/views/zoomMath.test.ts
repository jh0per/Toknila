import { describe, expect, it } from "vitest";
import { MAX_ZOOM, MIN_ZOOM, clampZoom, pinchZoom, stepZoom, zoomAbout } from "./zoomMath";

describe("clampZoom", () => {
  it("keeps the zoom between the bounds", () => {
    expect(clampZoom(0.1)).toBe(MIN_ZOOM);
    expect(clampZoom(5)).toBe(MAX_ZOOM);
    expect(clampZoom(1.3)).toBe(1.3);
  });
  it("turns a non-number into 100%", () => {
    expect(clampZoom(Number.NaN)).toBe(1);
  });
});

describe("stepZoom", () => {
  it("goes to the next size", () => {
    expect(stepZoom(1, 1)).toBe(1.1);
    expect(stepZoom(1, -1)).toBe(0.9);
    expect(stepZoom(0.95, 1)).toBe(1);
  });
  it("stops at the bounds", () => {
    expect(stepZoom(MAX_ZOOM, 1)).toBe(MAX_ZOOM);
    expect(stepZoom(MIN_ZOOM, -1)).toBe(MIN_ZOOM);
  });
});

describe("pinchZoom", () => {
  it("grows on a negative delta, shrinks on a positive one, and stays in bounds", () => {
    expect(pinchZoom(1, -10)).toBeGreaterThan(1);
    expect(pinchZoom(1, 10)).toBeLessThan(1);
    expect(pinchZoom(1.9, -500)).toBe(MAX_ZOOM);
    expect(pinchZoom(0.5, 500)).toBe(MIN_ZOOM);
  });
});

describe("zoomAbout", () => {
  it("keeps the point under the pointer", () => {
    const s = zoomAbout({ left: 100, top: 50 }, { x: 200, y: 100 }, 1, 2);
    // The diagram point (300, 150) is at (600, 300) now; the pointer is at (200, 100) in the view.
    expect(s).toEqual({ left: 400, top: 200 });
    expect(s.left + 200).toBe(300 * 2);
    expect(s.top + 100).toBe(150 * 2);
  });
  it("does not move when the zoom does not change", () => {
    expect(zoomAbout({ left: 7, top: 9 }, { x: 40, y: 40 }, 1.5, 1.5)).toEqual({ left: 7, top: 9 });
  });
  it("zooms out around the pointer", () => {
    expect(zoomAbout({ left: 400, top: 200 }, { x: 200, y: 100 }, 2, 1)).toEqual({ left: 100, top: 50 });
  });
});
