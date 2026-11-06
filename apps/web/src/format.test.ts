import { describe, expect, it } from "vitest";
import type { StatusReason } from "./api/types";
import { outOfOrderText, reasonText } from "./format";

describe("outOfOrderText", () => {
  it("names the earlier cards that were still open", () => {
    expect(outOfOrderText({ behind: ["DEMO-1", "DEMO-2"] })).toBe("done out of order: DEMO-1, DEMO-2 still open");
  });
  it("is empty without the mark", () => {
    expect(outOfOrderText(null)).toBe("");
    expect(outOfOrderText(undefined)).toBe("");
  });
});

describe("reasonText", () => {
  const blocked = (reason: object) => ({ status: "blocked", resume_to: "review", reason }) as StatusReason;
  it("names the failing checks and says a closed pull request was not merged", () => {
    expect(reasonText(blocked({ kind: "ci_failed", url: "https://x/pull/1", checks: ["verify (ubuntu)", "desktop"] }))).toBe(
      "checks failed: verify (ubuntu), desktop",
    );
    expect(reasonText(blocked({ kind: "pr_closed", url: "https://x/pull/1" }))).toBe(
      "the pull request was closed without merging",
    );
  });
});
