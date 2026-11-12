import type { Project } from "../api/types";
import { ROADMAP_STARTER } from "../store/assistant";
import type { Intent } from "../store/msg";
import type { State } from "../store/state";
import { AskBox } from "./AskBox";
import { FoundPlans } from "./FoundPlans";
import { NavLink } from "./Link";

/// A project without chains leads with one question for the project's assistant; typing an answer
/// and pressing Enter starts the assistant with it. Starting a chain by hand and importing a plan
/// are the quieter ways in, below.
export function EmptyChains({ project, state, dispatch }: { project: Project; state: State; dispatch: (i: Intent) => void }) {
  return (
    <div className="empty-state" data-testid="no-chains">
      <h2>What do you want to build?</h2>
      <p data-testid="roadmap-starter"><b>{ROADMAP_STARTER}</b></p>
      <p className="muted">
        Tell the assistant in a sentence or two. It reads {project.name || project.key}, drafts the plan as an epic of stories, and
        you start the stories when you are happy with it.
      </p>
      <AskBox
        project={project.key} state={state} dispatch={dispatch} testId="empty-ask" label="What do you want to build?"
        placeholder="For example: a sign-in page with email and password" button="Plan it with the assistant" focus
      />
      <p className="secondary-links">
        Or{" "}
        <button
          type="button" className="inline-link" data-testid="empty-new-chain"
          onClick={() => dispatch({ type: "openDialog", dialog: { kind: "newChain", project: project.key } })}
        >
          New epic
        </button>{" "}
        by hand, or{" "}
        <button
          type="button" className="inline-link" data-testid="empty-import"
          onClick={() => dispatch({ type: "openDialog", dialog: { kind: "import", project: project.key } })}
        >
          Import
        </button>{" "}
        a plan you already have. New to Toknila?{" "}
        <NavLink route={{ name: "guide" }} dispatch={dispatch} className="inline-link" testId="empty-guide">How it works</NavLink>
      </p>
      {/* A dialog over the page shows the list itself; the page's copy waits behind it. */}
      {project.repo_path && !state.plan.dialog && <FoundPlans project={project.key} state={state} dispatch={dispatch} />}
    </div>
  );
}
