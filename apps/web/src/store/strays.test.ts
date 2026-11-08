import { describe, expect, it } from "vitest";
import type { Stray } from "../api/types";
import { cleanedText, strayText } from "./strays";

const tree = (name: string, branch?: string): Stray => ({ kind: "worktree", name, branch });
const branch = (name: string, ahead?: number): Stray => ({ kind: "branch", name, ahead });

describe("strayText", () => {
  it("names a workspace by its path and the branch in it", () => {
    expect(strayText(tree("/s/a", "ship/c/a"))).toBe("/s/a (branch ship/c/a)");
    expect(strayText(tree("/s/a"))).toBe("/s/a");
  });
  it("says what deleting a branch would lose", () => {
    expect(strayText(branch("ship/c/a", 0))).toBe("ship/c/a (no commits of its own)");
    expect(strayText(branch("ship/c/a", 1))).toBe("ship/c/a (1 commit not in the base)");
    expect(strayText(branch("ship/c/a", 3))).toBe("ship/c/a (3 commits not in the base)");
    expect(strayText(branch("ship/c/a"))).toBe("ship/c/a");
  });
});

describe("cleanedText", () => {
  it("counts what went and what stayed", () => {
    expect(cleanedText(1, 0)).toBe("1 leftover removed");
    expect(cleanedText(3, 2)).toBe("3 leftovers removed; 2 could not be removed");
  });
});
