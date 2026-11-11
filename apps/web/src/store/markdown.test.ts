import { describe, expect, it } from "vitest";
import { blocks, inlines } from "./markdown";

describe("the goal's markdown", () => {
  it("reads bold and code inside a line", () => {
    expect(inlines("a **b** and `c`")).toEqual([{ text: "a " }, { text: "b", bold: true }, { text: " and " }, { text: "c", code: true }]);
  });
  it("splits paragraphs at blank lines and gathers list items", () => {
    expect(blocks("one\ntwo\n\n- a\n- b\n\n# Head")).toEqual([
      { kind: "p", inline: [{ text: "one two" }] },
      { kind: "ul", items: [[{ text: "a" }], [{ text: "b" }]] },
      { kind: "h", inline: [{ text: "Head" }] },
    ]);
  });
  it("gives nothing for an empty text", () => {
    expect(blocks("  \n ")).toEqual([]);
  });
});
