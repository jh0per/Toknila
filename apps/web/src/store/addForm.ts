// The add-a-project form: what the person typed, which fields they typed into, and what the
// daemon suggests for the folder. Key and name follow the folder's suggestion until the person
// types into them; the suggestion itself comes from core (`POST /v1/repos/check`).

import type { RepoCheck } from "../api/types";

/// What a person types to add a project.
export interface ProjectDraft {
  key: string;
  name: string;
  repo_path: string;
  work_branch: string;
}

export type DraftField = keyof ProjectDraft;

export interface AddForm {
  draft: ProjectDraft;
  /// Key, name and base branch the person typed into: the folder's suggestion no longer replaces them.
  edited: { key: boolean; name: boolean; work_branch: boolean };
  /// What the daemon suggested for a folder, and which folder (trimmed) it was for.
  suggested: { repo_path: string; key: string; name: string; work_branch: string } | undefined;
  /// Sent before the folder's suggestion came: the form goes out when the check answers.
  waiting: boolean;
}

export const initialAddForm: AddForm = {
  // The base branch is the repository's own (the branch it is on): the folder's check fills it in.
  draft: { key: "", name: "", repo_path: "", work_branch: "" },
  edited: { key: false, name: false, work_branch: false },
  suggested: undefined,
  waiting: false,
};

/// The form after the person typed into one field. Typing into the key or the name marks it as
/// theirs (even when they empty it); an emptied folder clears the fields that followed it.
export function editDraft(f: AddForm, field: DraftField, value: string): AddForm {
  const draft = { ...f.draft, [field]: value };
  if (field === "key" || field === "name" || field === "work_branch") return { ...f, draft, edited: { ...f.edited, [field]: true } };
  if (field === "repo_path" && !value.trim()) {
    return {
      ...f,
      suggested: undefined,
      draft: {
        ...draft,
        key: f.edited.key ? draft.key : "",
        name: f.edited.name ? draft.name : "",
        work_branch: f.edited.work_branch ? draft.work_branch : "",
      },
    };
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
  return {
    ...f,
    suggested: { repo_path: repoPath, key, name, work_branch },
    draft: {
      ...f.draft,
      key: f.edited.key ? f.draft.key : key,
      name: f.edited.name ? f.draft.name : name,
      work_branch: f.edited.work_branch ? f.draft.work_branch : work_branch,
    },
  };
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
    work_branch: f.draft.work_branch.trim(),
  };
}

/// Sent with an empty key before the folder's suggestion came: wait for the check first.
export function mustWait(f: AddForm): boolean {
  const d = resolvedDraft(f);
  return !d.key && d.repo_path !== "" && !currentSuggestion(f);
}
