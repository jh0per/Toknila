import type { Project } from "../api/types";
import type { Intent } from "../store/msg";
import type { State } from "../store/state";
import { AskBox } from "./AskBox";
import { FoundPlans } from "./FoundPlans";

/// A project without chains leads with one question for the project's assistant; typing an answer
/// and pressing Enter starts the assistant with it. Starting a chain by hand and importing a plan
/// are the quieter ways in, below.
export function EmptyChains({ project, state, dispatch }: { project: Project; state: State; dispatch: (i: Intent) => void }) {
  return (
    <div className="empty-state" data-testid="no-chains">
      <h2>What do you want to build?</h2>
      <p className="muted">
        Tell the assistant in a sentence or two. It reads {project.name || project.key}, drafts the plan as a chain of cards, and
        you start the cards when you are happy with it.
      </p>
      <AskBox
        project={project.key} state={state} dispatch={dispatch} testId="empty-ask" label="What do you want to build?"
        placeholder="For example: a sign-in page with email and password" button="Plan it with the assistant"
      />
      <p className="secondary-links">
        Or{" "}
        <button
          type="button" className="inline-link" data-testid="empty-new-chain"
          onClick={() => dispatch({ type: "openDialog", dialog: { kind: "newChain", project: project.key } })}
        >
          New chain
        </button>{" "}
        by hand, or{" "}
        <button
          type="button" className="inline-link" data-testid="empty-import"
          onClick={() => dispatch({ type: "openDialog", dialog: { kind: "import", project: project.key } })}
        >
          Import
        </button>{" "}
        a plan you already have.
      </p>
      {/* A dialog over the page shows the list itself; the page's copy waits behind it. */}
      {project.repo_path && !state.plan.dialog && <FoundPlans project={project.key} state={state} dispatch={dispatch} />}
    </div>
  );
}
