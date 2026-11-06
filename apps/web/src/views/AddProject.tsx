import { type ReactNode, useState } from "react";
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
/// is wrong with it and, when the base branch is missing, which branches the repository has.
export function AddProject({ state, dispatch }: { state: State; dispatch: (i: Intent) => void }) {
  const [key, setKey] = useState("");
  const [name, setName] = useState("");
  const [path, setPath] = useState("");
  const [branch, setBranch] = useState("develop");
  const s = state.settings;
  return (
    <form
      className="card-block form"
      data-testid="add-project"
      onSubmit={(e) => {
        e.preventDefault();
        dispatch({ type: "addProject", draft: { key: key.trim(), name, repo_path: path.trim(), work_branch: branch.trim() } });
      }}
    >
      <h2>Add a project</h2>
      <Field label="Folder of the repository" name="repo_path" error={fieldError(s, "repo_path")}>
        <input
          data-testid="repo-path" value={path} placeholder="/Users/you/Work/my-repo" spellCheck={false}
          onChange={(e) => setPath(e.target.value)}
        />
      </Field>
      <Field label="Base branch" name="work_branch" error={fieldError(s, "work_branch")}>
        <input data-testid="work-branch" value={branch} spellCheck={false} onChange={(e) => setBranch(e.target.value)} />
      </Field>
      {s.branches.length > 0 && fieldError(s, "work_branch") && (
        <p className="branches" data-testid="branches">
          Branches in this repository:{" "}
          {s.branches.map((b) => (
            <button type="button" className="btn" key={b} data-branch={b} onClick={() => setBranch(b)}>{b}</button>
          ))}
        </p>
      )}
      <Field label="Key (prefix of ticket keys)" name="key" error={fieldError(s, "key")}>
        <input data-testid="project-key" value={key} placeholder="TOKNILA" spellCheck={false} onChange={(e) => setKey(e.target.value)} />
      </Field>
      <Field label="Name (optional)" name="name" error={undefined}>
        <input value={name} onChange={(e) => setName(e.target.value)} />
      </Field>
      {formError(s) && <p className="error" role="alert">{formError(s)}</p>}
      <button className="btn primary" type="submit" disabled={s.busy} data-testid="add-project-submit">Add project</button>
    </form>
  );
}
