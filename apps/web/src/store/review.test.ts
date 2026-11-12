import { describe, expect, it } from "vitest";
import type { ReviewRound, ReviewView } from "../api/types";
import { awaitingText, basisText, itemRows, loopText, roundText } from "./review";

const round = (extra: Partial<ReviewRound>) =>
  ({ number: 1, reviewer: "human", verdict: "request_changes", comment: "", decided_at: 0, ...extra }) as ReviewRound;
const view = (awaiting: ReviewView["awaiting"]) =>
  ({ mode: "both", reviewer: "x", awaiting, rounds: [], max_rounds: 3, used_rounds: 1 }) as ReviewView;

describe("roundText", () => {
  it("says who decided and what, with the comment", () => {
    expect(roundText(round({ number: 2, comment: "handle the error case" })))
      .toBe("Round 2: you asked for changes — handle the error case");
    expect(roundText(round({ reviewer: "agent", verdict: "approve" }))).toBe("Round 1: the reviewer agent approved it");
  });

  it("words the daemon's check as passed or failed, and keeps only the first line of its output", () => {
    expect(roundText(round({ reviewer: "system", verdict: "approve", comment: "The check passed." })))
      .toBe("Round 1: the check passed — The check passed.");
    const none = round({ reviewer: "system", verdict: "approve", comment: "Brought up to main; the project has no check command." });
    expect(roundText(none)).toBe("Round 1: brought up to main; the project has no check command");
    const failed = round({ reviewer: "system", comment: "`cargo xtask verify` exited with code 101.\n\n```\nlong output\n```" });
    expect(roundText(failed)).toBe("Round 1: the check failed — `cargo xtask verify` exited with code 101.");
  });
});

describe("loopText", () => {
  it("counts the rounds used against the project's limit", () => {
    expect(loopText(view(null))).toBe("1 of 3 rounds of the loop used");
  });
});

describe("awaitingText", () => {
  it("names whose verdict the card waits for", () => {
    expect(awaitingText(view({ by: "person" }))).toBe("waiting for you");
    expect(awaitingText(view({ by: "agent", decides: true }))).toBe("waiting for the reviewer agent");
    expect(awaitingText(view({ by: "agent", decides: false }))).toBe("waiting for the reviewer agent, then you");
    expect(awaitingText(view({ by: "merging" }))).toBe("merging into the main branch");
    expect(awaitingText(view({ by: "check" }))).toBe("being checked: starting");
    const running = { ...view({ by: "check" }), check: { phase: "running", command: "cargo xtask verify", since: 0 } } as ReviewView;
    expect(awaitingText(running, 3 * 60_000)).toBe("being checked: cargo xtask verify, 3 min");
    expect(awaitingText(view(null))).toBe("");
  });
});

describe("itemRows", () => {
  it("reads each item line with whether the reviewer ran it, read it or could not run it", () => {
    const rows = itemRows(
      "PASS 1. It logs in - test result: ok. 3 passed [ran]\nFAIL 2. It is tidy - messy [read]\nPASS 3. Docs [NOT_RUN]\nplease fix 2",
    );
    expect(rows).toEqual([
      { outcome: "PASS", item: 1, text: "It logs in - test result: ok. 3 passed", basis: "ran" },
      { outcome: "FAIL", item: 2, text: "It is tidy - messy", basis: "read" },
      { outcome: "PASS", item: 3, text: "Docs", basis: "NOT_RUN" },
    ]);
    expect(basisText("ran")).toBe("ran");
    expect(basisText("read")).toBe("read only");
    expect(basisText("NOT_RUN")).toBe("NOT_RUN");
  });

  it("shows an item only a person can judge as PERSON", () => {
    const rows = itemRows("PASS 1. It logs in [ran]\nPERSON 2. It looks right - the picture is for a person [read]");
    expect(rows).toEqual([
      { outcome: "PASS", item: 1, text: "It logs in", basis: "ran" },
      { outcome: "PERSON", item: 2, text: "It looks right - the picture is for a person", basis: "read" },
    ]);
  });

  it("leaves out a round without item lines", () => {
    expect(itemRows("The check passed.")).toEqual([]);
    expect(itemRows("PASS 1. old verdict without a basis")).toEqual([]);
  });
});
