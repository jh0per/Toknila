import { describe, expect, it } from "vitest";
import { type KeyLike, isMac, keyAction } from "./keys";
import { decodeOsc52 } from "./osc52";
import { type BufferLine, type TextBuffer, allText, visibleText } from "./screen";

const key = (k: string, mods: Partial<KeyLike> = {}): KeyLike => ({
  key: k, ctrlKey: false, metaKey: false, shiftKey: false, altKey: false, ...mods,
});

describe("keyAction", () => {
  it("Cmd+C copies on a Mac, with or without a selection; Ctrl+C stays the agent's", () => {
    expect(keyAction(key("c", { metaKey: true }), true, true)).toBe("copy");
    expect(keyAction(key("c", { metaKey: true }), true, false)).toBe("copy");
    expect(keyAction(key("c", { ctrlKey: true }), true, true)).toBe("pass");
  });
  it("Ctrl+C copies with a selection and interrupts without one on Windows and Linux", () => {
    expect(keyAction(key("c", { ctrlKey: true }), false, true)).toBe("copy");
    expect(keyAction(key("c", { ctrlKey: true }), false, false)).toBe("pass");
  });
  it("Ctrl+Shift+C always copies off a Mac", () => {
    expect(keyAction(key("C", { ctrlKey: true, shiftKey: true }), false, false)).toBe("copy");
    expect(keyAction(key("C", { ctrlKey: true, shiftKey: true }), false, true)).toBe("copy");
  });
  it("leaves every other key alone", () => {
    expect(keyAction(key("c"), false, true)).toBe("pass");
    expect(keyAction(key("v", { ctrlKey: true }), false, true)).toBe("pass");
    expect(keyAction(key("c", { ctrlKey: true, altKey: true }), false, true)).toBe("pass");
    expect(keyAction(key("c", { metaKey: true }), false, true)).toBe("pass");
  });
  it("knows a Mac from its platform string", () => {
    expect(isMac("MacIntel")).toBe(true);
    expect(isMac("Win32")).toBe(false);
    expect(isMac("Linux x86_64")).toBe(false);
  });
});

describe("decodeOsc52", () => {
  it("decodes the base64 of a write, ASCII and UTF-8", () => {
    expect(decodeOsc52(`c;${btoa("hello")}`)).toBe("hello");
    expect(decodeOsc52("c;0J/RgNC40LLQtdGC")).toBe("Привет");
    expect(decodeOsc52(`;${btoa("x")}`)).toBe("x");
  });
  it("ignores a query, a clear, bad base64 and a missing payload", () => {
    expect(decodeOsc52("c;?")).toBeNull();
    expect(decodeOsc52("c;")).toBeNull();
    expect(decodeOsc52("c;***")).toBeNull();
    expect(decodeOsc52("c;/w==")).toBeNull();
    expect(decodeOsc52("c")).toBeNull();
  });
});

const buffer = (rows: [string, boolean?][], viewportY: number): TextBuffer => ({
  length: rows.length,
  viewportY,
  getLine: (y): BufferLine | undefined => {
    const r = rows[y];
    return r && { translateToString: () => r[0].trimEnd(), isWrapped: Boolean(r[1]) };
  },
});

describe("screen text", () => {
  const b = buffer([["old 1"], ["old 2"], ["one  "], ["two"], ["wrap"], ["ped", true], [""], [""]], 2);
  it("the visible screen is the viewport, wrapped rows joined, empty tail dropped", () => {
    expect(visibleText(b, 6)).toBe("one\ntwo\nwrapped");
  });
  it("a short window cuts the screen to its rows", () => {
    expect(visibleText(b, 2)).toBe("one\ntwo");
  });
  it("all text includes the scrollback", () => {
    expect(allText(b)).toBe("old 1\nold 2\none\ntwo\nwrapped");
  });
  it("an empty buffer is empty text", () => {
    expect(allText(buffer([], 0))).toBe("");
  });
});
