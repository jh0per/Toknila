import { useState } from "react";
import type { ProjectDeletion } from "../api/types";
import type { Intent } from "../store/msg";
import { folderText, keyConfirmed } from "../store/removal";
import type { State } from "../store/state";
import { Field } from "./Field";

/// Deleting a project: only its records, or its records and its repository folder (to the Trash,
/// once the person typed the key). Afterwards: what became of everything.
export function DeleteProject({ project: key, state, dispatch }: { project: string; state: State; dispatch: (i: Intent) => void }) {
  const [typed, setTyped] = useState("");
  const r = state.removal;
  if (r.result) return <Deleted d={r.result} trashAsked={r.trashAsked} dispatch={dispatch} />;
  const project = state.projects.find((p) => p.key === key);
  if (!project) return null;
  const remove = (trash_folder: boolean) => dispatch({ type: "deleteProject", project: project.key, trash_folder });
  return (
    <div className="card-block form" data-testid="delete-project">
      <h2>Delete {project.key}</h2>
      <p>
        Removes {project.key} from Toknila: its epics, stories, threads, journal and the links of its usage records, its
        run files and its workspaces. The project folder and the branches Toknila made for its stories stay untouched.
      </p>
      <button type="button" className="btn danger" disabled={r.busy} data-testid="delete-remove" onClick={() => remove(false)}>
        Remove from Toknila
      </button>
      {project.repo_path && (
        <div className="trash-zone">
          <p>
            Or remove it and move its folder to the Trash:{" "}
            <code data-testid="delete-folder">{project.repo_path}</code>
          </p>
          <Field label={`Type ${project.key} to confirm`} name="delete-confirm" error={undefined}>
            <input data-testid="delete-confirm" value={typed} spellCheck={false} onChange={(e) => setTyped(e.target.value)} />
          </Field>
          <button
            type="button" className="btn danger" data-testid="delete-trash"
            disabled={r.busy || !keyConfirmed(project, typed)} onClick={() => remove(true)}
          >
            Remove and move the folder to Trash
          </button>
        </div>
      )}
      {r.error && (
        <div className="error" role="alert" data-testid="delete-error">
          <p>{r.error.message}</p>
          {r.error.agentsWorking && (
            <p>
              Stop its agents first. The window has no button that stops an agent: let the running stories finish, or cancel
              them from their story, then delete again.
            </p>
          )}
        </div>
      )}
    </div>
  );
}

function Deleted({ d, trashAsked, dispatch }: { d: ProjectDeletion; trashAsked: boolean; dispatch: (i: Intent) => void }) {
  return (
    <div className="card-block form" data-testid="delete-result">
      <h2>{d.project.key} was removed</h2>
      <p>
        {d.chains.length} {d.chains.length === 1 ? "epic" : "epics"} and {d.tickets} {d.tickets === 1 ? "story" : "stories"} deleted
        {d.worktrees_removed.length > 0 ? `; workspaces removed: ${d.worktrees_removed.join(", ")}` : ""}.
      </p>
      <p className={d.folder === "kept" && trashAsked ? "error" : ""} data-testid="delete-folder-note">{folderText(d, trashAsked)}</p>
      {d.warnings.length > 0 && (
        <ul className="warnings" data-testid="delete-warnings">
          {d.warnings.map((w) => <li key={w}>{w}</li>)}
        </ul>
      )}
      <button type="button" className="btn" data-testid="delete-done" onClick={() => dispatch({ type: "closeDialog" })}>Close</button>
    </div>
  );
}
