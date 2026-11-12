import type { Project } from "../api/types";
import type { Intent } from "../store/msg";
import { planError } from "../store/plan";
import type { State } from "../store/state";

/// Archiving a project: out of the sidebar, its data kept, and back on request. The daemon refuses
/// while the project's agents work; the refusal shows here. Deleting it opens its own dialog.
export function ArchiveProject({ project, state, dispatch }: { project: Project; state: State; dispatch: (i: Intent) => void }) {
  const archived = Boolean(project.archived);
  const refused = planError(state.plan, "archive");
  return (
    <section className="card-block" data-testid="archive-project">
      <h2>{archived ? `${project.key} is archived` : "Archive"}</h2>
      <p className="muted">
        {archived
          ? "It is hidden from the sidebar; its epics, stories and history are kept."
          : `Hides ${project.key} from the sidebar and keeps its epics, stories and history. Not while its agents work.`}
      </p>
      <div className="row">
        <button
          type="button" className={archived ? "btn" : "btn danger"} disabled={state.plan.busy}
          data-testid={archived ? "unarchive-project" : "archive-project-button"}
          onClick={() => dispatch({ type: "archiveProject", project: project.key, archived: !archived })}
        >
          {archived ? "Unarchive project" : "Archive project"}
        </button>
        <button
          type="button" className="btn danger" data-testid="delete-project-button"
          onClick={() => dispatch({ type: "openDialog", dialog: { kind: "deleteProject", project: project.key } })}
        >
          Delete project…
        </button>
      </div>
      {refused && <p className="error" role="alert" data-testid="archive-error">{refused}</p>}
    </section>
  );
}
