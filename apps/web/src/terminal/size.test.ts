import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { type Size, sizeReporter, usable } from "./size";

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
