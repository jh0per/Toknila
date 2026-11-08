import { describe, expect, it } from "vitest";
import { type MenuEntry, menuGroups, moveFocus, shortcutEntry, typeAhead } from "./menuGroups";

const e = (label: string, keys?: string): MenuEntry => ({ label, testId: label, run: () => undefined, keys });

describe("menuGroups", () => {
  it("puts a separator at the start of every group but the first", () => {
    const m = menuGroups([e("Open"), e("Edit")], [e("Start")], [e("Cancel")]);
    expect(m.map((x) => `${x.sep ? "|" : ""}${x.label}`)).toEqual(["Open", "Edit", "|Start", "|Cancel"]);
  });

  it("leaves no separator for an empty group, at the start or in the middle", () => {
    const m = menuGroups([], [e("A")], [], [e("B")]);
    expect(m.map((x) => `${x.sep ? "|" : ""}${x.label}`)).toEqual(["A", "|B"]);
  });
});

describe("shortcutEntry", () => {
  const m = [e("Open", "Enter"), e("Start", "S"), e("Remove dependency", "Delete")];
  it("runs the entry whose shortcut was pressed, in any case", () => {
    expect(shortcutEntry(m, "s")?.label).toBe("Start");
    expect(shortcutEntry(m, "S")?.label).toBe("Start");
    expect(shortcutEntry(m, "Delete")?.label).toBe("Remove dependency");
  });
  it("leaves Enter to the entry in focus and ignores keys nobody listens to", () => {
    expect(shortcutEntry(m, "Enter")).toBeUndefined();
    expect(shortcutEntry(m, "x")).toBeUndefined();
  });
});

describe("moveFocus", () => {
  it("wraps around with the arrows and Tab", () => {
    expect(moveFocus("ArrowDown", 2, 3)).toBe(0);
    expect(moveFocus("ArrowUp", 0, 3)).toBe(2);
    expect(moveFocus("Tab", 0, 3)).toBe(1);
    expect(moveFocus("Tab", 0, 3, true)).toBe(2);
  });
  it("starts at the first entry on Down and the last on Up when none has focus", () => {
    expect(moveFocus("ArrowDown", -1, 3)).toBe(0);
    expect(moveFocus("ArrowUp", -1, 3)).toBe(2);
  });
  it("jumps with Home and End and ignores other keys", () => {
    expect(moveFocus("Home", 2, 5)).toBe(0);
    expect(moveFocus("End", 0, 5)).toBe(4);
    expect(moveFocus("a", 0, 5)).toBeUndefined();
    expect(moveFocus("Home", 0, 0)).toBeUndefined();
  });
});

describe("typeAhead", () => {
  const labels = ["Open", "Edit", "Open terminal", "Start", "Add card after"];
  it("finds the next entry that starts with the letter, wrapping", () => {
    expect(typeAhead(labels, "o", 0)).toBe(2);
    expect(typeAhead(labels, "o", 2)).toBe(0);
    expect(typeAhead(labels, "s", -1)).toBe(3);
  });
  it("keeps the entry in focus while more letters still match it", () => {
    expect(typeAhead(labels, "op", 2)).toBe(2);
    expect(typeAhead(labels, "open t", 2)).toBe(2);
    expect(typeAhead(labels, "ed", 0)).toBe(1);
  });
  it("finds nothing for a letter no entry starts with", () => {
    expect(typeAhead(labels, "z", 0)).toBeUndefined();
  });
});
