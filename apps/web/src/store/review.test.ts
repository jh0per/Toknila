import { describe, expect, it } from "vitest";
import type { ReviewRound, ReviewView } from "../api/types";
import { awaitingText, roundText } from "./review";

const round = (extra: Partial<ReviewRound>) =>
  ({ number: 1, reviewer: "human", verdict: "request_changes", comment: "", decided_at: 0, ...extra }) as ReviewRound;
const view = (awaiting: ReviewView["awaiting"]) => ({ mode: "both", reviewer: "x", awaiting, rounds: [] }) as ReviewView;

describe("roundText", () => {
  it("says who decided and what, with the comment", () => {
    expect(roundText(round({ number: 2, comment: "handle the error case" })))
      .toBe("Round 2: you asked for changes — handle the error case");
    expect(roundText(round({ reviewer: "agent", verdict: "approve" }))).toBe("Round 1: the reviewer agent approved it");
  });
});

describe("awaitingText", () => {
  it("names whose verdict the card waits for", () => {
    expect(awaitingText(view({ by: "person" }))).toBe("waiting for you");
    expect(awaitingText(view({ by: "agent", decides: true }))).toBe("waiting for the reviewer agent");
    expect(awaitingText(view({ by: "agent", decides: false }))).toBe("waiting for the reviewer agent, then you");
    expect(awaitingText(view({ by: "merging" }))).toBe("merging into the main branch");
    expect(awaitingText(view(null))).toBe("");
  });
});
