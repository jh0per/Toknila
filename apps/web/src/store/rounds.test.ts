import { describe, expect, it } from "vitest";
import type { EventRecord } from "../api/types";
import { reviewRounds } from "./rounds";

let seq = 0;
const ev = (entity: string, kind: string, payload: unknown): EventRecord =>
  ({ seq: ++seq, at: 1000 + seq, actor: "human", entity, kind, entity_id: "", payload }) as EventRecord;
const status = (from: string, to: string, id = "T1") => ev("ticket", "status_changed", { from, to, ticket: { id } });
const comment = (text: string, ticket = "T1") =>
  ev("comment", "added", { comment: { ticket_id: ticket, kind: "instruction", body: { gist: text } } });

describe("reviewRounds", () => {
  it("every return from review is a round, with the comment that came with it", () => {
    const events = [
      status("running", "review"), status("review", "running"), comment("handle the empty case"),
      status("running", "review"), status("review", "running"), comment("and the error case"),
    ];
    expect(reviewRounds(events, "T1")).toEqual([
      { round: 2, seq: events[1].seq, at: events[1].at, comment: "handle the empty case" },
      { round: 3, seq: events[4].seq, at: events[4].at, comment: "and the error case" },
    ]);
  });

  it("finds the comment when it is journaled before the status change", () => {
    const c = comment("fix it");
    const r = status("review", "running");
    expect(reviewRounds([c, r], "T1")[0].comment).toBe("fix it");
  });

  it("takes the comment that was added with the return, not an earlier one that was delivered just before it", () => {
    const earlier = { comment: { ticket_id: "T1", kind: "instruction", body: { gist: "earlier" } } };
    const events = [
      ev("comment", "delivered", earlier),
      status("review", "running"),
      comment("this return"),
      ev("comment", "delivered", { comment: { ticket_id: "T1", kind: "instruction", body: { gist: "this return" } } }),
    ];
    expect(reviewRounds(events, "T1")[0].comment).toBe("this return");
  });

  it("ignores other tickets and other moves", () => {
    const events = [status("waiting", "running"), comment("answer"), status("review", "running", "T2")];
    expect(reviewRounds(events, "T1")).toEqual([]);
  });
});
