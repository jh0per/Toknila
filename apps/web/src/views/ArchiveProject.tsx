import type { Project } from "../api/types";
import type { Intent } from "../store/msg";
import { planError } from "../store/plan";
import type { State } from "../store/state";

/// Archiving a project: put away (out of the sidebar and the switcher), its data kept, and back on
/// request. The daemon refuses while the project's agents work; the refusal shows here. The text
/// sends a person who only wants it off the list to Hide. Deleting it opens its own dialog.
export function ArchiveProject({ project, state, dispatch }: { project: Project; state: State; dispatch: (i: Intent) => void }) {
  const archived = Boolean(project.archived);
  const refused = planError(state.plan, "archive");
  return (
    <section className="card-block" data-testid="archive-project">
      <h2>{archived ? `${project.key} is archived` : "Archive"}</h2>
      <p className="muted">
        {archived
          ? "It is put away: out of the sidebar and the switcher; its epics, stories and history are kept."
          : `Puts ${project.key} away: out of the sidebar and the switcher, its epics, stories and history kept. Not while its agents work. ` +
            "To only take it off the sidebar's list while it keeps working, hide it (right click on its row)."}
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
