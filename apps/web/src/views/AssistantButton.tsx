import type { Intent } from "../store/msg";
import { plannerOf } from "../store/planner";
import { workingTitle } from "../store/plannerTurn";
import { type State, currentProject } from "../store/state";

/// The top bar's Assistant button: opens and closes the panel beside the work (Ctrl or Cmd and the
/// full stop does the same). While the project's assistant has a turn on it pulses, and its title
/// says the step.
export function AssistantButton({ state, dispatch }: { state: State; dispatch: (i: Intent) => void }) {
  const open = state.assistant.open;
  const project = currentProject(state);
  const working = workingTitle(project && plannerOf(state, project.key));
  return (
    <button
      type="button" className={`btn${open ? " on" : ""}`} data-testid="assistant-toggle" aria-pressed={open}
      title={working ? `${working}. Show or hide it (Ctrl or Cmd and .)` : "Show or hide the assistant (Ctrl or Cmd and .)"}
      onClick={() => dispatch({ type: "toggleAssistant" })}
    >
      {working && <span className="dot pulse" aria-hidden="true" data-testid="assistant-working" />}
      Assistant
    </button>
  );
}
