import type { Project } from "../api/types";
import type { Intent } from "../store/msg";
import type { State } from "../store/state";
import { FoundPlans } from "./FoundPlans";
import { ImportChain } from "./ImportChain";

type Props = { project: Project; state: State; dispatch: (i: Intent) => void };

/// Plans: the plans found in the project's repository, and a chain file, path or pasted text to
/// import.
export function PlansSection({ project, state, dispatch }: Props) {
  return (
    <div className="page-section">
      <section className="card-block" data-testid="repo-plans">
        <h2>Plans found in the project</h2>
        <p className="muted">
          Spec folders (<code>specs/*/tasks.md</code>) and epic files (<code>plans/*.toml</code>) in {project.key}'s folder.
        </p>
        <FoundPlans project={project.key} state={state} dispatch={dispatch} />
      </section>
      <ImportChain project={project} state={state} dispatch={dispatch} />
      <section className="card-block">
        <h2>Import from a path or pasted text</h2>
        <p className="muted">An epic file or a spec folder on this computer, or an epic file pasted as text.</p>
        <button
          type="button" className="btn" data-testid="open-import-dialog"
          onClick={() => dispatch({ type: "openDialog", dialog: { kind: "import", project: project.key } })}
        >
          Import…
        </button>
      </section>
    </div>
  );
}
