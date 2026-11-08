import { describe, expect, it } from "vitest";
import type { Project } from "../api/types";
import { formOf, isDirty, patchOf, sectionOfField } from "./projectForm";

const project = {
  id: "p1", key: "DEMO", name: "Demo", max_parallel: 2, work_branch: "main", release_branch: "main",
  default_profile: "claude-code", default_review_mode: "human", default_reviewer: null, check_command: null,
  worker_model: null, reviewer_model: null, planner_model: "opus",
  merge: { mode: "local", pr_title: "{key}: {title}", method: "merge" },
} as unknown as Project;

describe("the project form", () => {
  it("is clean as the project has it, and dirty after a change", () => {
    const f = formOf(project);
    expect(isDirty(f, project)).toBe(false);
    expect(isDirty({ ...f, max_parallel: "3" }, project)).toBe(true);
    expect(isDirty({ ...f, merge_mode: "github" }, project)).toBe(true);
  });

  it("ignores spaces around text", () => {
    const f = formOf(project);
    expect(isDirty({ ...f, work_branch: "main " }, project)).toBe(false);
    expect(isDirty({ ...f, check_command: "  " }, project)).toBe(false);
    expect(isDirty({ ...f, check_command: "npm test" }, project)).toBe(true);
  });

  it("sends every field, trimmed, with the limit as a number", () => {
    const patch = patchOf({ ...formOf(project), max_parallel: "4", check_command: " npm test ", worker_model: "sonnet" });
    expect(patch).toEqual({
      work_branch: "main", default_profile: "claude-code", max_parallel: 4, check_command: "npm test",
      worker_model: "sonnet", reviewer_model: "", planner_model: "opus", review_mode: "human", reviewer: "",
      merge_mode: "local", pr_title: "{key}: {title}", merge_method: "merge",
    });
  });

  it("says which section a refused field is in", () => {
    expect(sectionOfField("work_branch")).toBe("general");
    expect(sectionOfField("max_parallel")).toBe("agents");
    expect(sectionOfField("pr_title")).toBe("review");
    expect(sectionOfField("planner_model")).toBe("advanced");
  });
});
