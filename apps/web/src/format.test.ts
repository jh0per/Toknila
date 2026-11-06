import { describe, expect, it } from "vitest";
import type { StatusReason } from "./api/types";
import { ESTIMATE_NOTE, chipText, costText, outOfOrderText, reasonText, shareText, tokensText } from "./format";

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
  it("says a card without a working directory has no repository to run in", () => {
    expect(reasonText(blocked({ kind: "no_working_dir" }))).toBe(
      "no working directory: the project has no repository; give it one or use the sim profile",
    );
  });
});

describe("usage text", () => {
  it("shortens token counts", () => {
    expect(tokensText(0)).toBe("0");
    expect(tokensText(842)).toBe("842");
    expect(tokensText(12_340)).toBe("12.3k");
    expect(tokensText(1_234_567)).toBe("1.2M");
    expect(tokensText(3_400_000_000)).toBe("3.4B");
  });
  it("writes dollars", () => {
    expect(costText(0)).toBe("$0.00");
    expect(costText(0.004)).toBe("<$0.01");
    expect(costText(3.4)).toBe("$3.40");
    expect(costText(1234.5)).toBe("$1,234.50");
  });
  it("shows a share as a percentage", () => {
    expect(shareText(0.931)).toBe("93%");
    expect(shareText(0)).toBe("0%");
  });
  it("makes the chip of a card, and none for a card that used nothing", () => {
    expect(chipText({ processed: 1_200_000, cost_usd: 3.4, messages: 9 } as never)).toBe("1.2M tok · ~$3.40");
    expect(chipText({ processed: 0, cost_usd: 0, messages: 0 } as never)).toBe("");
    expect(chipText(undefined)).toBe("");
  });
  it("always says it is an estimate", () => {
    expect(ESTIMATE_NOTE).toMatch(/estimate/i);
    expect(ESTIMATE_NOTE).toMatch(/subscription/i);
  });
});
