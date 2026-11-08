import { describe, expect, it } from "vitest";
import type { EventRecord } from "../api/types";
import journal from "../../../../crates/core/src/events.rs?raw";
import { WORDED, eventWords } from "./eventWords";

const e = (entity: string, kind: string, payload: object = {}) =>
  ({ entity, kind, payload }) as Pick<EventRecord, "entity" | "kind" | "payload">;

/// The kinds the daemon journals, read from the source of the journal's `kind()`.
function journalKinds(): string[] {
  const body = journal.slice(journal.indexOf("pub fn kind(&self)"));
  const arms = body.slice(0, body.indexOf("\n    }\n"));
  return [...new Set([...arms.matchAll(/=> "([a-z_]+)"/g)].map((m) => m[1]))];
}

describe("eventWords", () => {
  it("words a status change with its reason", () => {
    expect(eventWords(e("ticket", "status_changed", { from: "ready", to: "running", reason: null }))).toBe("Moved from ready to running");
    expect(
      eventWords(e("ticket", "status_changed", {
        from: "running", to: "waiting", reason: { status: "waiting", kind: "question", text: "Which one?" },
      })),
    ).toBe("Moved from running to waiting (question: Which one?)");
    expect(
      eventWords(e("ticket", "status_changed", {
        from: "review", to: "blocked", reason: { status: "blocked", reason: { kind: "merge_conflict", detail: "a.rs" }, resume_to: "review" },
      })),
    ).toBe("Moved from review to blocked (the merge conflicts: a.rs)");
  });

  it("words comments, subtasks and answers", () => {
    expect(eventWords(e("comment", "added", { comment: { kind: "report", body: { gist: "All done" } } }))).toBe("Wrote a report: All done");
    expect(eventWords(e("comment", "added", { comment: { kind: "instruction", body: { gist: "Use blue" } } }))).toBe("Wrote an instruction: Use blue");
    expect(eventWords(e("comment", "delivered", { comment: { kind: "decision", body: { gist: "Ship" } } }))).toBe("The agent received a decision: Ship");
    expect(eventWords(e("subtask", "updated", { subtask: { text: "Write tests", done: true } }))).toBe("Subtask done: Write tests");
    expect(eventWords(e("ticket", "question_answered", { answer: "deny", role: "worker" }))).toBe("Answered: Turned down");
    expect(eventWords(e("ticket", "runner_started", { run: { role: "reviewer" } }))).toBe("The reviewer started");
  });

  it("still prints something for a kind it does not know", () => {
    expect(eventWords(e("widget", "was_polished"))).toBe("widget was polished");
  });

  it("has words for every kind the daemon journals", () => {
    const kinds = journalKinds();
    // The probe finds the kinds it should: a control, so an empty list cannot pass.
    expect(kinds.length).toBeGreaterThan(20);
    expect(kinds).toContain("question_answered");
    const worded = new Set(WORDED.map((k) => k.split(".")[1]));
    expect(kinds.filter((k) => !worded.has(k))).toEqual([]);
  });

  it("every worded pair reads as a sentence on an empty payload, not as the fallback", () => {
    for (const pair of WORDED) {
      const [entity, kind] = pair.split(".");
      const words = eventWords(e(entity, kind));
      expect(words, pair).not.toBe("");
      expect(words, pair).not.toBe(`${entity} ${kind.replace(/_/g, " ")}`);
      expect(words, pair).not.toMatch(/undefined|\[object/);
    }
  });
});
