import { describe, expect, it } from "vitest";
import type { EventRecord } from "../api/types";
import { lastScreenAsked, shipsFolder } from "./trustAsked";

const raised = (seq: number, key: string, kind: string, text?: string) =>
  ({ seq, at: seq * 1000, entity: "ticket", entity_id: "x", kind: "question_raised", payload: { key, kind, text } }) as unknown as EventRecord;

describe("lastScreenAsked", () => {
  it("finds the last screen question of the project's cards and says what it asked", () => {
    const events = [
      raised(1, "DEMO-1", "screen", "the agent's screen asks: Is this a project you trust?"),
      raised(2, "OTHER-1", "screen", "other"),
      raised(3, "DEMO-2", "permission"),
    ];
    expect(lastScreenAsked(events, "DEMO")).toEqual({ at: 1000, text: "the agent's screen asks: Is this a project you trust?" });
  });

  it("is undefined while Claude Code has not asked (control: another project did)", () => {
    expect(lastScreenAsked([raised(1, "OTHER-1", "screen", "x")], "DEMO")).toBeUndefined();
    expect(lastScreenAsked([], "DEMO")).toBeUndefined();
  });
});

describe("shipsFolder", () => {
  it("is the project's folder under the ships root", () => {
    expect(shipsFolder("/home/me/.toknila-ships/", "DEMO")).toBe("/home/me/.toknila-ships/demo");
  });
});
