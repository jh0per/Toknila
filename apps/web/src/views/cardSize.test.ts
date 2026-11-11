import { describe, expect, it } from "vitest";
import {
  AUTO_MAX_W, type CardContent, type Measure, MIN_W, RESIZE_MAX_H, RESIZE_MAX_W, autoCard, autoWidth, clampSize, contentHeight,
  descriptionLine, minHeight, sizedCard, wrapTitle,
} from "./cardSize";

/// Every character is 7 px in the title face, 6 px in the pill and 5 px in the key.
const PX = { title: 7, pill: 6, key: 5 };
const m: Measure = (text, face) => text.length * PX[face];
const card = (title: string, over: Partial<CardContent> = {}): CardContent => ({ title, pill: "review", key: "TOK-1", foot: "none", ...over });

describe("wrapTitle", () => {
  it("keeps a title that fits on one line", () => {
    expect(wrapTitle("Short title", 200, 2, m)).toEqual(["Short title"]);
  });

  it("wraps at spaces into lines that fit, keeping the space it broke at", () => {
    const lines = wrapTitle("Shell edits are refused, not asked about", 170, 2, m);
    expect(lines).toEqual(["Shell edits are refused, ", "not asked about"]);
    expect(lines.join("")).toBe("Shell edits are refused, not asked about");
    for (const l of lines) expect(m(l.trimEnd(), "title")).toBeLessThanOrEqual(170);
  });

  it("cuts with an ellipsis only after the last line", () => {
    const lines = wrapTitle("one two three four five six seven eight nine ten", 100, 2, m);
    expect(lines).toHaveLength(2);
    expect(lines[0].includes("…")).toBe(false);
    expect(lines[1].endsWith("…")).toBe(true);
    expect(m(lines[1], "title")).toBeLessThanOrEqual(100);
    expect(wrapTitle("one two three four five six seven eight nine ten", 100, 4, m)).toHaveLength(4);
  });

  it("breaks a word wider than a line", () => {
    const lines = wrapTitle("Supercalifragilisticexpialidocious", 100, 3, m);
    expect(lines.length).toBeGreaterThan(1);
    expect(lines.join("")).toBe("Supercalifragilisticexpialidocious");
    for (const l of lines) expect(m(l, "title")).toBeLessThanOrEqual(100);
  });

  it("gives an empty title one empty line", () => {
    expect(wrapTitle("  ", 100, 2, m)).toEqual([""]);
  });
});

describe("card size from content", () => {
  it("is never narrower than 196 px and never wider than 320 px", () => {
    expect(autoWidth(card("Hi"), m)).toBe(MIN_W);
    expect(autoWidth(card("x".repeat(200)), m)).toBe(AUTO_MAX_W);
  });

  it("is as wide as the title needs between the two", () => {
    // 30 characters: 210 px of text and 22 of padding, on the 8 px grid.
    expect(autoWidth(card("y".repeat(30)), m)).toBe(232);
  });

  it("is wide enough for the pill and the key side by side", () => {
    const wide = card("Hi", { pill: "awaiting approval", key: "TOKNILA-123" });
    // 17 * 6 + 28 + 6 + 11 * 5 = 191, plus 22 of padding.
    expect(autoWidth(wide, m)).toBeGreaterThan(MIN_W);
    expect(autoWidth(wide, m)).toBeGreaterThanOrEqual(191 + 22);
  });

  it("shows two lines and no ellipsis for the long title of a real card", () => {
    const c = card("Shell edits are refused, not asked about");
    const l = autoCard(c, m);
    expect(l.lines.length).toBeLessThanOrEqual(2);
    expect(l.lines.join("")).toBe(c.title);
    expect(l.lines.join("").includes("…")).toBe(false);
    expect(l.w).toBeLessThanOrEqual(AUTO_MAX_W);
  });

  it("grows taller with a second title line and with what sits under the pill", () => {
    const one = contentHeight(1, "none");
    expect(contentHeight(2, "none")).toBeGreaterThan(one);
    expect(contentHeight(1, "extras")).toBeGreaterThan(one);
    expect(contentHeight(1, "actions")).toBeGreaterThan(contentHeight(1, "extras"));
    for (const h of [one, contentHeight(2, "actions")]) expect(h % 8).toBe(0);
  });
});

describe("description line", () => {
  const d = card("Hi", { description: "## Build the thing\n\nsecond paragraph" });
  it("takes the first non-empty line, without Markdown marks", () => {
    expect(descriptionLine("\n\n# Heading\nmore")).toBe("Heading");
    expect(descriptionLine("- first item\n- second")).toBe("first item");
    expect(descriptionLine("```\ncode")).toBe("code");
    expect(descriptionLine("  \n")).toBeUndefined();
    expect(descriptionLine(undefined)).toBeUndefined();
  });

  it("shows only on a card taller than its title and rows need", () => {
    expect(autoCard(d, m).desc).toBeUndefined();
    const need = contentHeight(1, "none");
    expect(sizedCard(d, { w: 240, h: need }, m).desc).toBeUndefined();
    expect(sizedCard(d, { w: 240, h: contentHeight(1, "none", true) }, m).desc).toBe("Build the thing");
    expect(sizedCard(card("Hi"), { w: 240, h: 320 }, m).desc).toBeUndefined();
  });
});

describe("manual size", () => {
  it("clamps width to 196..480 and height to the content..320, on the grid", () => {
    expect(clampSize({ w: 10, h: 10 }, 72)).toEqual({ w: MIN_W, h: 72 });
    expect(clampSize({ w: 9000, h: 9000 }, 72)).toEqual({ w: RESIZE_MAX_W, h: RESIZE_MAX_H });
    expect(clampSize({ w: 301, h: 133 }, 72)).toEqual({ w: 304, h: 136 });
    // A content taller than the limit cannot push the limit.
    expect(clampSize({ w: 300, h: 100 }, 900).h).toBe(RESIZE_MAX_H);
  });

  it("lets a wider card show more title lines, and a narrower one cut sooner", () => {
    const c = card("one two three four five six seven eight nine ten eleven twelve thirteen fourteen fifteen sixteen seventeen eighteen nineteen twenty");
    expect(sizedCard(c, { w: 320, h: 100 }, m).lines).toHaveLength(2);
    expect(sizedCard(c, { w: 480, h: 200 }, m).lines.length).toBeGreaterThan(2);
  });

  it("never draws a card shorter than its content", () => {
    const c = card("Hi", { foot: "extras" });
    expect(sizedCard(c, { w: 240, h: 20 }, m).h).toBe(contentHeight(1, "extras"));
    expect(minHeight(c, 240, m)).toBe(contentHeight(1, "extras"));
  });

  it("needs more height at a narrow width where the title wraps", () => {
    const c = card("one two three four five six seven eight nine ten");
    expect(minHeight(c, MIN_W, m)).toBeGreaterThan(minHeight(c, RESIZE_MAX_W, m));
  });
});
