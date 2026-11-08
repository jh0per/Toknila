import type { Project } from "../api/types";
import type { Intent } from "../store/msg";
import { type State, currentChain } from "../store/state";
import { Canvas } from "./Canvas";
import { PanelDivider } from "./PanelDivider";
import { PlannerPanel } from "./PlannerPanel";
import { ThemeButton } from "./ThemeButton";

/// The assistant's screen: the project's most recently active chain above ("Plan so far"), which
/// follows what the assistant edits as the journal arrives, and the conversation below.
export function PlannerScreen({ project, state, dispatch }: { project: Project; state: State; dispatch: (i: Intent) => void }) {
  const chain = currentChain(state);
  return (
    <>
      <header className="top">
        <div className="crumb">
          <b>{project.key}</b><i>/</i><b data-testid="planner-title">Plan with the assistant</b>
        </div>
        {chain && <span className="muted" data-testid="planner-chain">Plan so far: chain {chain.chain.name}</span>}
        {state.error && <span className="error">{state.error}</span>}
        <span className="grow" />
        <ThemeButton theme={state.theme} dispatch={dispatch} />
      </header>
      {chain ? (
        <Canvas chain={chain} state={state} dispatch={dispatch} />
      ) : (
        <div className="canvas planner-empty" data-testid="planner-empty">
          <h3>Plan so far</h3>
          <p className="muted empty">Nothing yet. Tell the assistant below what you want to build; the chains and cards it makes show here as it plans.</p>
        </div>
      )}
      <PanelDivider />
      <PlannerPanel project={project} state={state} dispatch={dispatch} />
    </>
  );
}
