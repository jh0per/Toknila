import { describe, expect, it } from "vitest";
import type { Comment, ReviewRound, ReviewView } from "../api/types";
import { hintDismissed, needsYou, storeHintDismissed } from "./needsYou";

const report = (extra: object): Comment =>
  ({ id: "c", ticket_id: "t", kind: "report", author: "agent", body: { gist: "Built it", ...extra }, created_at: 1, delivered_at: null }) as unknown as Comment;
const round = (number: number, reviewer: ReviewRound["reviewer"], comment: string): ReviewRound =>
  ({ number, reviewer, verdict: "request_changes", comment, decided_at: 0 }) as ReviewRound;
const view = (rounds: ReviewRound[]) => ({ rounds }) as unknown as ReviewView;

describe("needsYou", () => {
  it("lists the PERSON items of the hand-in and of the reviewer's verdict", () => {
    const handIn = report({
      how_to_verify: "- PASS cargo xtask verify: ok\n- PERSON look at the login on a phone",
      consequences: "PERSON: say if the blue is the right blue",
    });
    const verdict = round(
      1,
      "agent",
      "PASS 1. Errors are shown - ran [ran]\nPERSON 2. Looks right on a phone - open it on a phone [read]",
    );
    expect(needsYou(handIn, view([verdict]))).toEqual([
      "look at the login on a phone",
      "say if the blue is the right blue",
      "Looks right on a phone - open it on a phone",
    ]);
  });

  it("is empty when nothing is for a person, and ignores a PASS or FAIL line", () => {
    const handIn = report({ how_to_verify: "- PASS a: ok\n- FAIL b: no" });
    expect(needsYou(handIn, view([round(1, "agent", "FAIL 1. x - y [read]")]))).toEqual([]);
    expect(needsYou(undefined, undefined)).toEqual([]);
  });

  it("reads only the newest round that has items, so an old PERSON item does not come back", () => {
    const old = round(1, "agent", "PERSON 1. old thing [read]");
    const now = round(3, "agent", "PASS 1. fine [ran]");
    expect(needsYou(undefined, view([old, now]))).toEqual([]);
  });

  it("does not list the same item twice", () => {
    const handIn = report({ how_to_verify: "- PERSON open it on a phone" });
    const verdict = round(1, "agent", "PERSON 2. open it on a phone [read]");
    expect(needsYou(handIn, view([verdict]))).toEqual(["open it on a phone"]);
  });
});

describe("the first-time hint", () => {
  const storage = () => {
    const m = new Map<string, string>();
    return { getItem: (k: string) => m.get(k) ?? null, setItem: (k: string, v: string) => void m.set(k, v) } as unknown as Storage;
  };
  it("shows until it is dismissed, per project", () => {
    const s = storage();
    expect(hintDismissed("DEMO", s)).toBe(false);
    storeHintDismissed("DEMO", s);
    expect(hintDismissed("DEMO", s)).toBe(true);
    expect(hintDismissed("OTHER", s)).toBe(false);
  });
  it("shows when storage cannot be read", () => {
    const broken = { getItem: () => { throw new Error("blocked"); }, setItem: () => { throw new Error("blocked"); } } as unknown as Storage;
    expect(hintDismissed("DEMO", broken)).toBe(false);
    expect(() => storeHintDismissed("DEMO", broken)).not.toThrow();
  });
});
