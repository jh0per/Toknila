import { describe, expect, it } from "vitest";
import { reduce } from "./reduce";
import { SHORTCUTS, asksForShortcuts, tabAfterKey } from "./shortcuts";
import { initialState } from "./state";

const press = (key: string, over: Partial<{ ctrlKey: boolean; metaKey: boolean; altKey: boolean }> = {}) => ({
  key, ctrlKey: false, metaKey: false, altKey: false, ...over,
});

describe("the shortcuts list", () => {
  it("names every key the window answers to", () => {
    const keys = SHORTCUTS.map((s) => s.keys.join(" "));
    expect(keys).toEqual(["N", "S", "Enter", "Delete", "Esc", "Cmd/Ctrl ,", "Cmd/Ctrl .", "Cmd/Ctrl K", "?"]);
    expect(SHORTCUTS.every((s) => s.what.length > 0)).toBe(true);
  });

  it("opens, closes and toggles", () => {
    const s0 = initialState({ name: "chains" });
    expect(s0.shortcutsOpen).toBe(false);
    const [open] = reduce(s0, { type: "toggleShortcuts" });
    expect(open.shortcutsOpen).toBe(true);
    const [shut] = reduce(open, { type: "toggleShortcuts" });
    expect(shut.shortcutsOpen).toBe(false);
    const [closed] = reduce(open, { type: "closeShortcuts" });
    expect(closed.shortcutsOpen).toBe(false);
    expect(reduce(s0, { type: "closeShortcuts" })[0].shortcutsOpen).toBe(false);
  });
});

describe("asksForShortcuts", () => {
  it("is the ? key outside text fields", () => {
    expect(asksForShortcuts(press("?"), false)).toBe(true);
    expect(asksForShortcuts(press("?"), true)).toBe(false);
    expect(asksForShortcuts(press("/"), false)).toBe(false);
    expect(asksForShortcuts(press("?", { metaKey: true }), false)).toBe(false);
    expect(asksForShortcuts(press("?", { ctrlKey: true }), false)).toBe(false);
    expect(asksForShortcuts(press("?", { altKey: true }), false)).toBe(false);
  });
});

describe("tabAfterKey", () => {
  const tabs = ["details", "events", "chat"] as const;
  it("steps with the arrows and wraps round", () => {
    expect(tabAfterKey(tabs, "details", "ArrowRight")).toBe("events");
    expect(tabAfterKey(tabs, "chat", "ArrowRight")).toBe("details");
    expect(tabAfterKey(tabs, "details", "ArrowLeft")).toBe("chat");
    expect(tabAfterKey(tabs, "events", "ArrowLeft")).toBe("details");
  });
  it("Home and End go to the ends, other keys do nothing", () => {
    expect(tabAfterKey(tabs, "events", "Home")).toBe("details");
    expect(tabAfterKey(tabs, "events", "End")).toBe("chat");
    expect(tabAfterKey(tabs, "events", "ArrowDown")).toBeUndefined();
    expect(tabAfterKey(tabs, "events", "a")).toBeUndefined();
    expect(tabAfterKey(tabs, "missing" as never, "ArrowRight")).toBeUndefined();
    expect(tabAfterKey([], "x", "Home")).toBeUndefined();
  });
});
