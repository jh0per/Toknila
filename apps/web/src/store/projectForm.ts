// The project's settings as one form: every field the Projects page edits, kept as the person
// typed it, compared with the daemon's project to know whether anything changed (the Save bar
// shows only then) and turned into the one patch that Save sends. The daemon judges the values.

import type { MergeMethod, MergeMode, PatchProject, Project, ReviewMode } from "../api/types";
import type { ProjectSection } from "./route";

export interface ProjectForm {
  work_branch: string;
  default_profile: string;
  max_parallel: string;
  check_command: string;
  worker_model: string;
  reviewer_model: string;
  planner_model: string;
  review_mode: ReviewMode;
  reviewer: string;
  merge_mode: MergeMode;
  pr_title: string;
  merge_method: MergeMethod;
}

/// The form as the project has it now.
export function formOf(p: Project): ProjectForm {
  return {
    work_branch: p.work_branch,
    default_profile: p.default_profile,
    max_parallel: String(p.max_parallel),
    check_command: p.check_command ?? "",
    worker_model: p.worker_model ?? "",
    reviewer_model: p.reviewer_model ?? "",
    planner_model: p.planner_model ?? "",
    review_mode: p.default_review_mode,
    reviewer: p.default_reviewer ?? "",
    merge_mode: p.merge.mode,
    pr_title: p.merge.pr_title,
    merge_method: p.merge.method,
  };
}

const TEXT = ["work_branch", "check_command", "worker_model", "reviewer_model", "planner_model", "pr_title"] as const;

/// The form with its text fields trimmed, as Save sends them.
export function tidy(f: ProjectForm): ProjectForm {
  const out = { ...f };
  for (const k of TEXT) out[k] = f[k].trim();
  return out;
}

/// What Save sends: every field of the form (the daemon keeps the ones that did not change).
export function patchOf(f: ProjectForm): PatchProject {
  const t = tidy(f);
  return {
    work_branch: t.work_branch,
    default_profile: t.default_profile,
    max_parallel: Number(t.max_parallel),
    check_command: t.check_command,
    worker_model: t.worker_model,
    reviewer_model: t.reviewer_model,
    planner_model: t.planner_model,
    review_mode: t.review_mode,
    reviewer: t.reviewer,
    merge_mode: t.merge_mode,
    pr_title: t.pr_title,
    merge_method: t.merge_method,
  };
}

/// Whether the form differs from the project.
export function isDirty(f: ProjectForm, p: Project): boolean {
  const now = tidy(formOf(p));
  const then = tidy(f);
  return (Object.keys(now) as (keyof ProjectForm)[]).some((k) => now[k] !== then[k]);
}

/// The section a daemon refusal about `field` belongs to, so the Save bar can say where to look.
export function sectionOfField(field: string): ProjectSection {
  switch (field) {
    case "default_profile":
    case "max_parallel":
    case "check_command":
      return "agents";
    case "review_mode":
    case "reviewer":
    case "merge_mode":
    case "pr_title":
    case "merge_method":
      return "review";
    case "worker_model":
    case "reviewer_model":
    case "planner_model":
      return "advanced";
    default:
      return "general";
  }
}

/// Words for the sections, as the tabs and the Save bar show them.
export const SECTION_LABEL: Record<ProjectSection, string> = {
  general: "General",
  agents: "Agents",
  review: "Review and merge",
  plans: "Plans",
  advanced: "Advanced",
};
