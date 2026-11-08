import type { Project } from "../api/types";
import type { Intent } from "../store/msg";
import type { State } from "../store/state";
import { ArchiveProject } from "./ArchiveProject";
import { FoundPlans } from "./FoundPlans";
import { ImportChain } from "./ImportChain";

type Props = { project: Project; state: State; dispatch: (i: Intent) => void };

/// Plans: the plans found in the project's repository, and a chain file, path or pasted text to
/// import.
export function PlansSection({ project, state, dispatch }: Props) {
  return (
    <>
      <section className="card-block" data-testid="repo-plans">
        <h2>Plans in the repository</h2>
        <p className="muted">
          Spec folders (<code>specs/*/tasks.md</code>) and chain files (<code>plans/*.toml</code>) in {project.key}'s repository.
        </p>
        <FoundPlans project={project.key} state={state} dispatch={dispatch} />
      </section>
      <ImportChain project={project} state={state} dispatch={dispatch} />
      <section className="card-block">
        <h2>Import from a path or pasted text</h2>
        <p className="muted">A chain file or a spec folder the daemon reads from disk, or a chain file pasted as text.</p>
        <button
          type="button" className="btn" data-testid="open-import-dialog"
          onClick={() => dispatch({ type: "openDialog", dialog: { kind: "import", project: project.key } })}
        >
          Import…
        </button>
      </section>
    </>
  );
}

/// Danger zone: archiving the project, and deleting it.
export function DangerSection({ project, state, dispatch }: Props) {
  return (
    <>
      <h2 className="danger-title">Danger zone</h2>
      <ArchiveProject project={project} state={state} dispatch={dispatch} />
    </>
  );
}
