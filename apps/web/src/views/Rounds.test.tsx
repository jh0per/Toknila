import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { ReviewRound, TicketDetail } from "../api/types";
import { Rounds } from "./Rounds";

const round = (extra: Partial<ReviewRound>) =>
  ({ number: 1, reviewer: "system", verdict: "approve", comment: "", decided_at: 0, ...extra }) as ReviewRound;

const render = (rounds: ReviewRound[]) =>
  renderToStaticMarkup(
    <Rounds detail={{ review: { mode: "agent", reviewer: "x", awaiting: null, rounds, max_rounds: 3, used_rounds: 1 } } as unknown as TicketDetail} />,
  );

describe("Rounds", () => {
  it("shows a check that passed on its second run with the flaky mark", () => {
    const html = render([
      round({ comment: "The check passed on the second run; the first failed:\ntest einval ... FAILED" }),
    ]);
    expect(html).toMatch(/data-testid="round"[^>]*>Round 1: the check passed on the second run/);
    expect(html).toMatch(/data-testid="flaky-mark"[^>]*>flaky</);
  });

  it("marks no other round flaky", () => {
    const html = render([round({ comment: "The check passed." })]);
    expect(html).toContain('data-testid="round"');
    expect(html).not.toContain("flaky-mark");
  });
});
