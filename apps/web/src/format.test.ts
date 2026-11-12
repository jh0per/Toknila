import { describe, expect, it } from "vitest";
import type { StatusReason } from "./api/types";
import { ESTIMATE_NOTE, agoText, chipText, costText, durationText, outOfOrderText, reasonText, shareText, tokensText } from "./format";

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

  it("says an agent ended from outside was terminated and that unblock starts it again", () => {
    expect(reasonText(blocked({ kind: "terminated", detail: "exit 143, SIGTERM" }))).toBe(
      "the agent was terminated from outside (exit 143, SIGTERM); unblock starts it again",
    );
  });
  it("names the failing checks and says a closed pull request was not merged", () => {
    expect(reasonText(blocked({ kind: "ci_failed", url: "https://x/pull/1", checks: ["verify (ubuntu)", "desktop"] }))).toBe(
      "checks failed: verify (ubuntu), desktop",
    );
    expect(reasonText(blocked({ kind: "pr_closed", url: "https://x/pull/1" }))).toBe(
      "the pull request was closed without merging",
    );
  });
  it("names a main branch that is behind its upstream and the decisions", () => {
    const behind = (can_fast_forward: boolean) =>
      reasonText(blocked({ kind: "base_behind", base: "main", upstream: "origin/main", behind: 133, can_fast_forward }));
    expect(behind(true)).toBe(
      "main branch main is 133 commits behind origin/main; fast-forward it and start, start anyway, or unblock to look again",
    );
    expect(behind(false)).toBe(
      "main branch main is 133 commits behind origin/main; start anyway, or unblock to look again",
    );
  });
  it("says a card without a working directory has no repository to run in", () => {
    expect(reasonText(blocked({ kind: "no_working_dir" }))).toBe(
      "no working directory: the project has no repository; give it a folder or use the simulated agent",
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
  it("says how old a figure is", () => {
    const now = 1_000_000_000_000;
    expect(agoText(now - 20_000, now)).toBe("just now");
    expect(agoText(now - 5 * 60_000, now)).toBe("5 min ago");
    expect(agoText(now - 2 * 3_600_000, now)).toBe("2 h ago");
    expect(agoText(now - 72 * 3_600_000, now)).toBe("3 d ago");
    expect(agoText(now + 60_000, now)).toBe("just now");
  });
  it("shows a share as a percentage", () => {
    expect(shareText(0.931)).toBe("93%");
    expect(shareText(0)).toBe("0%");
  });
  it("makes the chip of a card, and none for a card that used nothing", () => {
    expect(chipText({ cost_usd: 3.8, messages: 9 }, 137_000)).toBe("est. $3.80 · 2m 17s");
    expect(chipText({ cost_usd: 3.4, messages: 9 })).toBe("est. $3.40");
    expect(chipText({ cost_usd: 0, messages: 0 })).toBe("");
    expect(chipText(undefined)).toBe("");
  });
  it("says a run's time with its biggest units", () => {
    expect(chipText(undefined, 45_000)).toBe("45s");
    expect(chipText(undefined, 500)).toBe("");
    expect(durationText(125_000)).toBe("2m 5s");
    expect(durationText(3_900_000)).toBe("1h 05m");
  });
  it("always says it is an estimate", () => {
    expect(ESTIMATE_NOTE).toMatch(/estimate/i);
    expect(ESTIMATE_NOTE).toMatch(/subscription/i);
  });
});
