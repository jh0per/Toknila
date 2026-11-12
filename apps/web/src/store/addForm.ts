// The add-a-project form: what the person typed, which fields they typed into, and what the
// daemon suggests for the folder. Key and name follow the folder's suggestion until the person
// types into them; the suggestion itself comes from core (`POST /v1/repos/check`).

import type { MergeMode, RepoCheck, RepoFix } from "../api/types";
import { REAL_AGENT } from "./profiles";

/// What a person types to add a project.
export interface ProjectDraft {
  key: string;
  name: string;
  repo_path: string;
  work_branch: string;
  /// The command that checks the work; empty names none.
  check_command: string;
  /// The command run once in every new workspace; empty names none.
  setup_command: string;
  merge_mode: MergeMode;
}

export type DraftField = keyof ProjectDraft;

export interface AddForm {
  draft: ProjectDraft;
  /// Key, name and base branch the person typed into: the folder's suggestion no longer replaces them.
  edited: { key: boolean; name: boolean; work_branch: boolean; check_command: boolean; setup_command: boolean; merge_mode: boolean };
  /// What the daemon suggested for a folder, and which folder (trimmed) it was for.
  suggested: Suggestion | undefined;
  /// Sent before the folder's suggestion came: the form goes out when the check answers.
  waiting: boolean;
  /// A fix the daemon is making for a folder (`POST /v1/repos/prepare`); the form waits for its answer.
  fixing: { repo_path: string; fix: RepoFix } | undefined;
  /// The daemon's sentence about the last fix it refused, for the folder the form shows.
  fixError: { repo_path: string; message: string } | undefined;
}

/// What the folder suggests; the check command and the merge mode come from the repository's files
/// and remote (`POST /v1/repos/check`).
export interface Suggestion {
  repo_path: string;
  key: string;
  name: string;
  work_branch: string;
  check_command: string;
  setup_command: string;
  merge_mode: MergeMode;
}

const FOLLOWING = ["key", "name", "work_branch", "check_command", "setup_command", "merge_mode"] as const;

export const initialAddForm: AddForm = {
  // The base branch is the repository's own (the branch it is on): the folder's check fills it in.
  draft: { key: "", name: "", repo_path: "", work_branch: "", check_command: "", setup_command: "", merge_mode: "local" },
  edited: { key: false, name: false, work_branch: false, check_command: false, setup_command: false, merge_mode: false },
  suggested: undefined,
  waiting: false,
  fixing: undefined,
  fixError: undefined,
};

/// The form after the person typed into one field. Typing into the key or the name marks it as
/// theirs (even when they empty it); an emptied folder clears the fields that followed it.
export function editDraft(f: AddForm, field: DraftField, value: string): AddForm {
  const draft = { ...f.draft, [field]: value };
  if ((FOLLOWING as readonly string[]).includes(field)) return { ...f, draft, edited: { ...f.edited, [field]: true } };
  if (field === "repo_path" && !value.trim()) {
    const blank = initialAddForm.draft;
    const kept = Object.fromEntries(FOLLOWING.map((k) => [k, f.edited[k] ? draft[k] : blank[k]]));
    return { ...f, suggested: undefined, draft: { ...draft, ...kept } };
  }
  return { ...f, draft };
}

/// Whether a change of the folder field asks the daemon for a new suggestion.
export function needsCheck(f: AddForm, field: DraftField, value: string): boolean {
  return field === "repo_path" && value.trim() !== "" && value.trim() !== f.draft.repo_path.trim();
}

/// The form after the daemon checked a folder: the suggestion fills the key and the name the
/// person has not typed into. An answer about a folder the form no longer shows is dropped.
export function applyCheck(f: AddForm, repoPath: string, check: RepoCheck): AddForm {
  if (repoPath !== f.draft.repo_path.trim()) return f;
  const key = check.suggested_key ?? "";
  const name = check.suggested_name ?? "";
  const work_branch = check.suggested_branch ?? "";
  const check_command = check.suggested_check_command ?? "";
  const setup_command = check.suggested_setup_command ?? "";
  const merge_mode = check.suggested_merge_mode ?? "local";
  const suggested: Suggestion = { repo_path: repoPath, key, name, work_branch, check_command, setup_command, merge_mode };
  const follow = Object.fromEntries(FOLLOWING.map((k) => [k, f.edited[k] ? f.draft[k] : suggested[k]]));
  return { ...f, suggested, draft: { ...f.draft, ...follow } };
}

/// The form after a fix was answered or refused: the fix is over, and a refusal is kept for the
/// folder it was about. An answer about a folder the form no longer shows leaves no trace but that.
export function endFix(f: AddForm, repoPath: string, refusal?: string): AddForm {
  const fixing = f.fixing?.repo_path === repoPath ? undefined : f.fixing;
  const fixError = refusal !== undefined && repoPath === f.draft.repo_path.trim() ? { repo_path: repoPath, message: refusal } : undefined;
  return { ...f, fixing, fixError };
}

/// The daemon's sentence about a refused fix, while the form still shows that folder.
export function fixRefusal(f: AddForm): string | undefined {
  return f.fixError && f.fixError.repo_path === f.draft.repo_path.trim() ? f.fixError.message : undefined;
}

/// The suggestion for the folder the form shows now, if the daemon has answered for it.
export function currentSuggestion(f: AddForm): AddForm["suggested"] {
  return f.suggested && f.suggested.repo_path === f.draft.repo_path.trim() ? f.suggested : undefined;
}

/// The draft to send: trimmed, with an empty key or name taken from the folder's suggestion.
export function resolvedDraft(f: AddForm): ProjectDraft {
  const s = currentSuggestion(f);
  return {
    key: f.draft.key.trim() || s?.key || "",
    name: f.draft.name.trim() || s?.name || "",
    repo_path: f.draft.repo_path.trim(),
    // Never empty when the folder suggested a branch: an empty one is not sent at all.
    work_branch: f.draft.work_branch.trim() || s?.work_branch || "",
    check_command: f.draft.check_command.trim(),
    setup_command: f.draft.setup_command.trim(),
    merge_mode: f.draft.merge_mode,
  };
}

/// The body of `POST /v1/projects` for a draft. The agent is named: a project added from the window
/// runs the real one, never the simulated agent the daemon would not default to either.
export function newProjectBody(d: ProjectDraft) {
  return {
    key: d.key, name: d.name || undefined, repo_path: d.repo_path, work_branch: d.work_branch || undefined,
    check_command: d.check_command || undefined, setup_command: d.setup_command || undefined, merge_mode: d.merge_mode,
    default_profile: REAL_AGENT,
  };
}

/// Sent with an empty key before the folder's suggestion came: wait for the check first.
export function mustWait(f: AddForm): boolean {
  const d = resolvedDraft(f);
  return !d.key && d.repo_path !== "" && !currentSuggestion(f);
}
