import type { Project } from "../api/types";
import type { Intent } from "../store/msg";

/// A project without chains: start one in the window, or import a plan.
export function EmptyChains({ project, dispatch }: { project: Project; dispatch: (i: Intent) => void }) {
  return (
    <div className="empty-state" data-testid="no-chains">
      <h2>No chains yet</h2>
      <p className="muted">
        A chain is the plan of {project.key}: cards that agents work through in order. Start one here, or import a plan.
      </p>
      <div className="row">
        <button
          type="button" className="btn primary" data-testid="empty-new-chain"
          onClick={() => dispatch({ type: "openDialog", dialog: { kind: "newChain", project: project.key } })}
        >
          New chain
        </button>
        <button
          type="button" className="btn" data-testid="empty-import"
          onClick={() => dispatch({ type: "openDialog", dialog: { kind: "import", project: project.key } })}
        >
          Import
        </button>
      </div>
    </div>
  );
}
