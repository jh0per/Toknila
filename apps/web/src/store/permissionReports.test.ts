import { describe, expect, it } from "vitest";
import type { Comment } from "../api/types";
import { finishReport, isPermissionReport } from "./permissionReports";

const comment = (kind: Comment["kind"], author: Comment["author"], gist: string): Comment =>
  ({ id: "c", ticket_id: "t", kind, author, body: { gist }, created_at: 0, delivered_at: null }) as unknown as Comment;

describe("isPermissionReport", () => {
  it("recognises the old report of a permission request", () => {
    expect(isPermissionReport(comment("report", "agent", "Claude Code asks for permission: Bash: ls"))).toBe(true);
    expect(isPermissionReport(comment("report", "agent", "Claude needs your permission to use Bash: ls"))).toBe(true);
  });

  it("keeps every other comment, including a question and a person's text", () => {
    expect(isPermissionReport(comment("report", "agent", "Which database should I use?"))).toBe(false);
    expect(isPermissionReport(comment("instruction", "human", "Claude Code asks for permission"))).toBe(false);
    expect(isPermissionReport(comment("report", "human", "Claude Code asks for permission"))).toBe(false);
  });
});

describe("finishReport", () => {
  it("is the agent's last written report, skipping permission requests and other people's comments", () => {
    const list = [
      comment("report", "agent", "First hand-in"),
      comment("report", "agent", "Second hand-in"),
      comment("report", "agent", "Claude Code asks for permission: Bash: ls"),
      comment("instruction", "human", "Please add a test"),
    ];
    expect(finishReport(list)?.body.gist).toBe("Second hand-in");
  });

  it("is nothing when the agent wrote no report", () => {
    expect(finishReport([comment("instruction", "human", "Go")])).toBeUndefined();
    expect(finishReport([])).toBeUndefined();
  });
});
