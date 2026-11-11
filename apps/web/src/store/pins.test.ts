import { describe, expect, it } from "vitest";
import type { EventRecord } from "../api/types";
import { MAX_ANSWER, type Pin, answerProblem, isOpen, pinPlace, pinVersion, pinsFor } from "./pins";

const pin = (over: Partial<Pin>): Pin => ({
  id: "p1", project: "x", design: "d1", design_name: "a.png", number: 1,
  region: { x: 1, y: 2, w: 3, h: 4 }, text: "Q?", card: "DEMO-1", created_at: 0, ...over,
});
const ev = (seq: number, entity: string): EventRecord => ({ seq, at: 0, actor: "agent", entity, entity_id: "e", kind: "k", payload: {} });

describe("pins", () => {
  it("is open until it has an answer", () => {
    expect(isOpen(pin({}))).toBe(true);
    expect(isOpen(pin({ answer: "Yes" }))).toBe(false);
  });

  it("selects the pins of a design, and of a card on it", () => {
    const all = [pin({ id: "a" }), pin({ id: "b", card: "DEMO-2" }), pin({ id: "c", design: "d2" }), pin({ id: "d", card: undefined })];
    expect(pinsFor(all, "d1").map((p) => p.id)).toEqual(["a", "b", "d"]);
    expect(pinsFor(all, "d1", "DEMO-1").map((p) => p.id)).toEqual(["a"]);
    expect(pinsFor(all, "nothing")).toEqual([]);
  });

  it("takes its version from the newest pin event", () => {
    expect(pinVersion([ev(1, "ticket"), ev(2, "design_pin"), ev(3, "ticket")])).toBe(2);
    expect(pinVersion([ev(1, "ticket")])).toBe(0);
  });

  it("refuses an empty or too long answer", () => {
    expect(answerProblem("  ")).toBe("Write an answer first.");
    expect(answerProblem("x".repeat(MAX_ANSWER + 1))).toContain("at most 500");
    expect(answerProblem("x".repeat(MAX_ANSWER))).toBeUndefined();
  });

  it("names its place in words", () => {
    expect(pinPlace(pin({}))).toBe("x 1%, y 2%, 3% wide, 4% tall");
  });
});
