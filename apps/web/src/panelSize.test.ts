import { afterEach, describe, expect, it, vi } from "vitest";
import { DIVIDER_PX, MIN_DIAGRAM_PX, MIN_PANEL_PX, STORAGE_KEY, clampPanel, readStoredHeight, storeHeight } from "./panelSize";

describe("the panel height", () => {
  it("keeps a height that leaves room for both and clamps one that does not", () => {
    expect(clampPanel(300, 700)).toBe(300);
    expect(clampPanel(10, 700)).toBe(MIN_PANEL_PX);
    expect(clampPanel(900, 700)).toBe(700 - DIVIDER_PX - MIN_DIAGRAM_PX);
  });

  it("keeps the panel's minimum in a window too small for both", () => {
    expect(clampPanel(500, 200)).toBe(MIN_PANEL_PX);
  });
});

describe("the remembered height", () => {
  afterEach(() => vi.unstubAllGlobals());

  const withStorage = (store: Partial<Storage>) => vi.stubGlobal("window", { localStorage: store });

  it("is read back as it was stored", () => {
    const data = new Map<string, string>();
    withStorage({ getItem: (k) => data.get(k) ?? null, setItem: (k, v) => void data.set(k, v) });
    expect(readStoredHeight()).toBeNull();
    storeHeight(312.4);
    expect(data.get(STORAGE_KEY)).toBe("312");
    expect(readStoredHeight()).toBe(312);
  });

  it("is ignored when it is not a number", () => {
    withStorage({ getItem: () => "tall" });
    expect(readStoredHeight()).toBeNull();
  });

  it("does not break when the storage throws", () => {
    const boom = () => {
      throw new Error("blocked");
    };
    withStorage({ getItem: boom, setItem: boom });
    expect(readStoredHeight()).toBeNull();
    expect(() => storeHeight(300)).not.toThrow();
  });
});
