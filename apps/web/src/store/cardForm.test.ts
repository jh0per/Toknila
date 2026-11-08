import { describe, expect, it } from "vitest";
import { acceptanceFrom, cardTitle, titleFromTask } from "./cardForm";

describe("titleFromTask", () => {
  it("takes the first non-empty line", () => {
    expect(titleFromTask("\n\n  Add a login page\nwith a form")).toBe("Add a login page");
  });

  it("drops Markdown marks and extra spaces", () => {
    expect(titleFromTask("## Fix   the  header")).toBe("Fix the header");
    expect(titleFromTask("- Write the tests")).toBe("Write the tests");
    expect(titleFromTask("2) Ship it")).toBe("Ship it");
  });

  it("cuts a long line at a word with an ellipsis", () => {
    const title = titleFromTask("Make the settings page look good on a phone and on a very large monitor as well, please");
    expect(title.endsWith("…")).toBe(true);
    expect(title.length).toBeLessThanOrEqual(71);
    expect(title).toBe("Make the settings page look good on a phone and on a very large…");
  });

  it("is empty for an empty task", () => {
    expect(titleFromTask("  \n ")).toBe("");
  });
});

describe("acceptanceFrom", () => {
  it("is one item per non-empty line, bullets dropped", () => {
    expect(acceptanceFrom("Login works\n\n- Logout works\n  * Errors are shown  \n")).toEqual([
      "Login works", "Logout works", "Errors are shown",
    ]);
  });
});

describe("cardTitle", () => {
  it("prefers what the person typed over the derived one, and falls back when it is cleared", () => {
    expect(cardTitle("My title", "Add a login page")).toBe("My title");
    expect(cardTitle(undefined, "Add a login page\nmore")).toBe("Add a login page");
    expect(cardTitle("  ", "Add a login page")).toBe("Add a login page");
  });
});
