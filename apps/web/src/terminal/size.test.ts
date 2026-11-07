import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { type Size, fitGrid, sizeReporter, usable } from "./size";

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

const setup = () => {
  const sent: Size[] = [];
  return { sent, r: sizeReporter((s) => sent.push(s), 100) };
};

describe("usable", () => {
  it("refuses a panel that is hidden or still laying out", () => {
    expect(usable({ cols: 0, rows: 0 })).toBe(false);
    expect(usable({ cols: 80, rows: 0 })).toBe(false);
    expect(usable({ cols: Number.NaN, rows: 24 })).toBe(false);
    expect(usable({ cols: 80.5, rows: 24 })).toBe(false);
    expect(usable({ cols: 80, rows: 24 })).toBe(true);
  });
});

describe("fitGrid", () => {
  const cell = { width: 7, height: 15 };

  it("counts only the rows that fit whole, so the last one is never cut", () => {
    expect(fitGrid({ width: 700, height: 300 }, cell)).toEqual({ cols: 100, rows: 20 });
    expect(fitGrid({ width: 700, height: 314 }, cell)?.rows).toBe(20);
    expect(fitGrid({ width: 700, height: 315 }, cell)?.rows).toBe(21);
    expect(fitGrid({ width: 700, height: 301 }, cell)?.rows).toBe(20);
  });

  it("takes the scrollbar off the width before counting columns", () => {
    expect(fitGrid({ width: 710, height: 300 }, cell, 10)?.cols).toBe(100);
    expect(fitGrid({ width: 709, height: 300 }, cell, 10)?.cols).toBe(99);
  });

  it("never gives a grid larger than the box at any height", () => {
    for (let h = 1; h < 900; h += 7) {
      const g = fitGrid({ width: 640, height: h }, cell);
      expect(g).not.toBeNull();
      if (h >= cell.height) expect((g?.rows ?? 0) * cell.height).toBeLessThanOrEqual(h);
    }
  });

  it("keeps the smallest grid for a box smaller than one cell", () => {
    expect(fitGrid({ width: 5, height: 5 }, cell)).toEqual({ cols: 2, rows: 1 });
  });

  it("waits while the box or the cell is not measured", () => {
    expect(fitGrid({ width: 0, height: 300 }, cell)).toBeNull();
    expect(fitGrid({ width: 700, height: 0 }, cell)).toBeNull();
    expect(fitGrid({ width: 700, height: 300 }, { width: 0, height: 15 })).toBeNull();
    expect(fitGrid({ width: Number.NaN, height: 300 }, cell)).toBeNull();
  });
});

describe("sizeReporter", () => {
  it("sends the last size once the panel has been still, not every step of a drag", () => {
    const { sent, r } = setup();
    r.report({ cols: 100, rows: 30 });
    vi.advanceTimersByTime(50);
    r.report({ cols: 110, rows: 30 });
    vi.advanceTimersByTime(50);
    r.report({ cols: 120, rows: 31 });
    expect(sent).toEqual([]);
    vi.advanceTimersByTime(100);
    expect(sent).toEqual([{ cols: 120, rows: 31 }]);
  });

  it("does not repeat a size the daemon already has", () => {
    const { sent, r } = setup();
    r.report({ cols: 90, rows: 30 });
    vi.advanceTimersByTime(100);
    r.report({ cols: 90, rows: 30 });
    vi.advanceTimersByTime(100);
    expect(sent).toHaveLength(1);
    r.report({ cols: 91, rows: 30 });
    vi.advanceTimersByTime(100);
    expect(sent).toHaveLength(2);
  });

  it("ignores a size no terminal can have", () => {
    const { sent, r } = setup();
    r.report({ cols: 0, rows: 0 });
    vi.advanceTimersByTime(500);
    expect(sent).toEqual([]);
  });

  it("sends the latest size again on request, even if it did not change", () => {
    const { sent, r } = setup();
    r.resend();
    expect(sent).toEqual([]);
    r.report({ cols: 90, rows: 30 });
    vi.advanceTimersByTime(100);
    r.resend();
    expect(sent).toEqual([
      { cols: 90, rows: 30 },
      { cols: 90, rows: 30 },
    ]);
  });

  it("sends nothing after it is disposed", () => {
    const { sent, r } = setup();
    r.report({ cols: 90, rows: 30 });
    r.dispose();
    vi.advanceTimersByTime(500);
    r.resend();
    expect(sent).toEqual([]);
  });
});
