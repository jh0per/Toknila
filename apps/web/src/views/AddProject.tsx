import type { ChangeEvent, ReactNode } from "react";
import type { MergeMode } from "../api/types";
import type { DraftField } from "../store/addForm";
import type { Intent } from "../store/msg";
import { fieldError, formError } from "../store/settings";
import type { State } from "../store/state";

export function Field({ label, name, error, children }: { label: string; name: string; error: string | undefined; children: ReactNode }) {
  return (
    <label className="field">
      <span>{label}</span>
      {children}
      {error && <span className="field-error" role="alert" data-testid={`error-${name}`}>{error}</span>}
    </label>
  );
}

/// Add a project from a folder. A text field takes the path (paste-friendly); the daemon says what
/// is wrong with it and, when the base branch is missing, which branches the repository has. Key
/// and name follow what the folder suggests until the person types into them (the store decides).
export function AddProject({ state, dispatch, focus = false }: {
  state: State;
  dispatch: (i: Intent) => void;
  /// Put the cursor in the folder field (in a dialog, not on the onboarding page).
  focus?: boolean;
}) {
  const s = state.settings;
  const d = s.add.draft;
  const edit = (field: DraftField) => (e: ChangeEvent<HTMLInputElement>) =>
    dispatch({ type: "editDraft", field, value: e.target.value });
  return (
    <form
      className="card-block form"
      data-testid="add-project"
      onSubmit={(e) => {
        e.preventDefault();
        dispatch({ type: "submitProject" });
      }}
    >
      <h2>Add a project</h2>
      <Field label="Folder of the repository" name="repo_path" error={fieldError(s, "repo_path")}>
        <input
          data-testid="repo-path" value={d.repo_path} placeholder="/Users/you/Work/my-repo" spellCheck={false}
          autoFocus={focus} onChange={edit("repo_path")}
        />
      </Field>
      <Field label="Base branch" name="work_branch" error={fieldError(s, "work_branch")}>
        <input
          data-testid="work-branch" value={d.work_branch} placeholder="the branch the repository is on" spellCheck={false}
          onChange={edit("work_branch")}
        />
      </Field>
      {s.branches.length > 0 && fieldError(s, "work_branch") && (
        <p className="branches" data-testid="branches">
          Branches in this repository:{" "}
          {s.branches.map((b) => (
            <button
              type="button" className="btn" key={b} data-branch={b}
              onClick={() => dispatch({ type: "editDraft", field: "work_branch", value: b })}
            >
              {b}
            </button>
          ))}
        </p>
      )}
      <Field label="Key (prefix of ticket keys)" name="key" error={fieldError(s, "key")}>
        <input data-testid="project-key" value={d.key} placeholder="from the folder's name" spellCheck={false} onChange={edit("key")} />
      </Field>
      <Field label="Name" name="name" error={fieldError(s, "name")}>
        <input data-testid="project-name" value={d.name} placeholder="the folder's name" onChange={edit("name")} />
      </Field>
      <Field label="Check command (agents run it before they hand in)" name="check_command" error={fieldError(s, "check_command")}>
        <input
          data-testid="new-check-command" value={d.check_command} spellCheck={false}
          placeholder="none found in the repository" onChange={edit("check_command")}
        />
      </Field>
      <Field label="Accepted work lands" name="merge_mode" error={fieldError(s, "merge_mode")}>
        <select
          data-testid="new-merge-mode" value={d.merge_mode}
          onChange={(e) => dispatch({ type: "editDraft", field: "merge_mode", value: e.target.value as MergeMode })}
        >
          <option value="local">local (merge into the base branch)</option>
          <option value="github">github (open a pull request)</option>
        </select>
      </Field>
      <p className="muted" data-testid="add-defaults-note">
        Prefilled from the repository: its test command, and a pull request when it has a GitHub remote and <code>gh</code> is
        logged in. Change them here or later in the project's settings.
      </p>
      {formError(s) && <p className="error" role="alert">{formError(s)}</p>}
      <button className="btn primary" type="submit" disabled={s.busy} data-testid="add-project-submit">Add project</button>
    </form>
  );
}
