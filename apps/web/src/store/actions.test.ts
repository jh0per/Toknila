import { describe, expect, it } from "vitest";
import type { Ticket } from "../api/types";
import { actionsFor } from "./actions";

const card = (status: Ticket["status"], paused = false) => ({ status, paused }) as Ticket;
const names = (t: Ticket) => actionsFor(t).map((a) => a.action);

describe("actionsFor", () => {
  it("offers what the machine accepts from each status", () => {
    expect(names(card("ready"))).toEqual(["start", "pause", "cancel"]);
    expect(names(card("running"))).toEqual(["pause", "cancel"]);
    expect(names(card("blocked"))).toEqual(["unblock", "pause", "cancel"]);
    expect(names(card("review"))).toEqual(["accept", "return", "cancel"]);
  });

  it("a paused card resumes instead of pausing", () => {
    expect(names(card("backlog", true))).toEqual(["resume", "cancel"]);
    expect(names(card("backlog"))).toEqual(["pause", "cancel"]);
  });

  it("a finished card has nothing to do", () => {
    expect(names(card("merged"))).toEqual([]);
    expect(names(card("cancelled"))).toEqual([]);
  });

  it("only a return asks for a comment", () => {
    expect(actionsFor(card("review")).filter((a) => a.needsText).map((a) => a.action)).toEqual(["return"]);
  });
});
