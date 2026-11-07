import type { Project } from "../api/types";
import type { Intent } from "../store/msg";
import { type State, currentChain } from "../store/state";
import { Canvas } from "./Canvas";
import { PlannerPanel } from "./PlannerPanel";

/// The planner entry: the project's most recently active chain above, which follows what the
/// planner edits as the journal arrives, and the planner's terminal below.
export function PlannerScreen({ project, state, dispatch }: { project: Project; state: State; dispatch: (i: Intent) => void }) {
  const chain = currentChain(state);
  return (
    <>
      <header className="top">
        <div className="crumb">
          <b>{project.key}</b><i>/</i><b data-testid="planner-title">Planner</b>
        </div>
        {chain && <span className="muted" data-testid="planner-chain">showing chain {chain.chain.name}</span>}
        {state.error && <span className="error">{state.error}</span>}
      </header>
      {chain ? (
        <Canvas chain={chain} state={state} dispatch={dispatch} />
      ) : (
        <div className="canvas planner-empty" data-testid="planner-empty">
          <p className="muted empty">No chains yet. Start the planner and ask it to plan this project; its chains show here as it makes them.</p>
        </div>
      )}
      <PlannerPanel project={project} state={state} dispatch={dispatch} />
    </>
  );
}
