import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ChainDetail } from "../api/types";
import { dayText, finishText, readFinishedOpen, splitChains, storeFinishedOpen } from "./phase";

const chain = (name: string, phase: ChainDetail["summary"]["phase"]) =>
  ({ chain: { id: name, name }, summary: { phase, total: 4, done: 1, finished_at: null } }) as unknown as ChainDetail;

describe("phase", () => {
  it("puts active and waiting first and finished chains apart", () => {
    const { open, finished } = splitChains([
      chain("a", "finished"), chain("b", "idle"), chain("c", "waiting"), chain("d", "active"), chain("e", "finished"),
    ]);
    expect(open.map((c) => c.chain.name)).toEqual(["d", "c", "b"]);
    expect(finished.map((c) => c.chain.name)).toEqual(["a", "e"]);
  });
  it("words the Finish node: the date when finished, else what is left", () => {
    const ms = Date.UTC(2026, 10, 12, 12);
    expect(finishText({ phase: "finished", finished_at: ms, total: 4, done: 4 })).toBe(`Finished · ${dayText(ms)}`);
    expect(dayText(ms)).toBe("12 Nov");
    expect(finishText({ phase: "active", finished_at: null, total: 4, done: 1 })).toBe("3 left");
  });
});

describe("Finished group memory", () => {
  beforeEach(() => {
    const data = new Map<string, string>();
    vi.stubGlobal("window", {
      localStorage: { getItem: (k: string) => data.get(k) ?? null, setItem: (k: string, v: string) => data.set(k, v) },
    });
  });
  afterEach(() => vi.unstubAllGlobals());
  it("is closed by default and remembered per project", () => {
    expect(readFinishedOpen("DEMO")).toBe(false);
    storeFinishedOpen("DEMO", true);
    expect(readFinishedOpen("DEMO")).toBe(true);
    expect(readFinishedOpen("OTHER")).toBe(false);
  });
});
