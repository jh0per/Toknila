import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { PullRequest } from "../api/types";
import { PrChip, checksSummary } from "./PrChip";

const pr = (...states: Array<"pending" | "passed" | "failed">) =>
  ({
    number: 7,
    url: "https://github.com/o/r/pull/7",
    checks: states.map((state, i) => ({ name: `c${i}`, state })),
  }) as PullRequest;

describe("checksSummary", () => {
  it("is failed when any failed, else pending while any runs or none is reported, else passed", () => {
    expect(checksSummary(pr("passed", "failed", "pending"))).toBe("failed");
    expect(checksSummary(pr("passed", "pending"))).toBe("pending");
    expect(checksSummary(pr())).toBe("pending");
    expect(checksSummary(pr("passed", "passed"))).toBe("passed");
  });
});

describe("PrChip", () => {
  it("shows the number and the link, and takes the state of the checks as its class", () => {
    const html = renderToStaticMarkup(<PrChip pr={pr("failed")} />);
    expect(html).toContain("PR #7");
    expect(html).toContain('data-href="https://github.com/o/r/pull/7"');
    expect(html).toContain("mark pr failed");
  });
});
