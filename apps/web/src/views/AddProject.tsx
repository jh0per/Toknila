import { type ChangeEvent, useState } from "react";
import type { MergeMode } from "../api/types";
import type { DraftField } from "../store/addForm";
import type { Intent } from "../store/msg";
import { fieldError, formError } from "../store/settings";
import type { State } from "../store/state";
import { canBrowse } from "../store/shell";
import { Field } from "./Field";

/// The id of the add-project form, for a button that sits outside it.
export const ADD_PROJECT_FORM = "add-project-form";

const DETAIL_FIELDS =["work_branch", "key", "name", "check_command", "merge_mode"];

/// Add a project from a folder. In the desktop app Browse opens the system's folder picker; in a
/// browser (and beside it) the path is typed or pasted. Toknila fills in what it finds in the
/// folder, as real values, under Details; the daemon says what is wrong with the folder and, when
/// the main branch is missing, which branches the repository has.
export function AddProject({ state, dispatch, focus = false, submit = true }: {
  state: State;
  dispatch: (i: Intent) => void;
  /// Put the cursor in the folder field (in a dialog, not on the onboarding page).
  focus?: boolean;
  /// Draw the form's own "Add project" button. The onboarding page puts it at the end of its last
  /// step instead, tied to this form by `ADD_PROJECT_FORM`.
  submit?: boolean;
}) {
  const s = state.settings;
  const d = s.add.draft;
  const [opened, setOpened] = useState(false);
  // A refusal about a field inside Details opens it, so the person sees it.
  const refused = DETAIL_FIELDS.some((f) => fieldError(s, f));
  const edit = (field: DraftField) => (e: ChangeEvent<HTMLInputElement>) =>
    dispatch({ type: "editDraft", field, value: e.target.value });
  const found = s.add.suggested;
  return (
    <form
      className="card-block form add-project"
      id={ADD_PROJECT_FORM}
      data-testid="add-project"
      onSubmit={(e) => {
        e.preventDefault();
        dispatch({ type: "submitProject" });
      }}
    >
      <h2>Add a project</h2>
      <Field
        label="Folder of your project" name="repo_path" error={fieldError(s, "repo_path")}
        hint={canBrowse() ? "Pick it with Browse, or paste its path." : "Paste the path of the folder that holds your project (a git repository)."}
      >
        <span className="path-row">
          <input data-testid="repo-path" value={d.repo_path} spellCheck={false} autoFocus={focus} onChange={edit("repo_path")} />
          {canBrowse() && (
            <button type="button" className="btn" data-testid="browse-folder" onClick={() => dispatch({ type: "browseFolder" })}>
              Browse…
            </button>
          )}
        </span>
      </Field>
      <details
        className="fold" data-testid="add-details" open={opened || refused}
        onToggle={(e) => setOpened((e.currentTarget as HTMLDetailsElement).open)}
      >
        <summary>
          Details
          {found && (
            <span className="muted found" data-testid="add-found">
              {found.key} · {found.work_branch} · {found.merge_mode === "github" ? "pull requests" : "merges locally"}
            </span>
          )}
        </summary>
        <Field label="Main branch" name="work_branch" error={fieldError(s, "work_branch")} hint="Agents start from it, and approved work lands on it.">
          <input data-testid="work-branch" value={d.work_branch} spellCheck={false} onChange={edit("work_branch")} />
        </Field>
        {s.branches.length > 0 && fieldError(s, "work_branch") && (
          <p className="branches" data-testid="branches">
            Branches in this folder:{" "}
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
        <Field label="Card prefix" name="key" error={fieldError(s, "key")} hint="Cards are named with it, like DEMO-1.">
          <input data-testid="project-key" value={d.key} spellCheck={false} onChange={edit("key")} />
        </Field>
        <Field label="Name" name="name" error={fieldError(s, "name")}>
          <input data-testid="project-name" value={d.name} onChange={edit("name")} />
        </Field>
        <Field label="Test command (optional)" name="check_command" error={fieldError(s, "check_command")} hint="Agents run it before they hand in.">
          <input data-testid="new-check-command" value={d.check_command} spellCheck={false} onChange={edit("check_command")} />
        </Field>
        <Field label="When a card is approved" name="merge_mode" error={fieldError(s, "merge_mode")}>
          <select
            data-testid="new-merge-mode" value={d.merge_mode}
            onChange={(e) => dispatch({ type: "editDraft", field: "merge_mode", value: e.target.value as MergeMode })}
          >
            <option value="local">Merge it into the main branch</option>
            <option value="github">Open a pull request on GitHub</option>
          </select>
        </Field>
        <p className="muted" data-testid="add-defaults-note">
          Filled in from the folder: its test command, and pull requests when it has a GitHub remote and <code>gh</code> is
          logged in. Change them here or later in the project's settings.
        </p>
      </details>
      {formError(s) && <p className="error" role="alert">{formError(s)}</p>}
      {submit && <button className="btn primary" type="submit" disabled={s.busy} data-testid="add-project-submit">Add project</button>}
    </form>
  );
}
