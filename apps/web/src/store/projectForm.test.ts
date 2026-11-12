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

  it("saves the setup command: dirty after a change, an emptied field clears it, and the Agents tab shows its refusals", () => {
    const f = formOf(project);
    expect(f.setup_command).toBe("");
    expect(isDirty({ ...f, setup_command: "npm ci" }, project)).toBe(true);
    const set = { ...project, setup_command: "npm ci" } as Project;
    expect(formOf(set).setup_command).toBe("npm ci");
    expect(patchOf({ ...formOf(set), setup_command: "  " }).setup_command).toBe("");
    expect(sectionOfField("setup_command")).toBe("agents");
  });

  it("sends every field, trimmed, with the limit as a number", () => {
    const patch = patchOf({
      ...formOf(project), max_parallel: "4", check_command: " npm test ", setup_command: " npm ci --prefix apps/web ",
      bump_command: " npm version {version} ", worker_model: "sonnet",
    });
    expect(patch).toEqual({
      work_branch: "main", default_profile: "claude-code", max_parallel: 4, check_command: "npm test",
      setup_command: "npm ci --prefix apps/web", bump_command: "npm version {version}",
      worker_model: "sonnet", reviewer_model: "", planner_model: "opus", review_mode: "human", max_review_rounds: 3, reviewer: "",
      merge_mode: "local", pr_title: "{key}: {title}", merge_method: "merge", no_checks_timeout_secs: 600,
      command_policy: "ask", allowed_commands: [], reviewer_web_domains: [],
    });
  });

  it("shows the no-checks timeout in minutes (default 10) and sends seconds", () => {
    const f = formOf(project);
    expect(f.no_checks_timeout_min).toBe("10");
    expect(isDirty({ ...f, no_checks_timeout_min: "25" }, project)).toBe(true);
    expect(patchOf({ ...f, no_checks_timeout_min: "25" }).no_checks_timeout_secs).toBe(1500);
    const set = { ...project, merge: { ...project.merge, no_checks_timeout_secs: 90 } } as Project;
    expect(formOf(set).no_checks_timeout_min).toBe("1.5");
  });

  it("says which section a refused field is in", () => {
    expect(sectionOfField("work_branch")).toBe("general");
    expect(sectionOfField("max_parallel")).toBe("agents");
    expect(sectionOfField("pr_title")).toBe("review");
    expect(sectionOfField("planner_model")).toBe("advanced");
  });
});
