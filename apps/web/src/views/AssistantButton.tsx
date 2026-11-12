import type { Intent } from "../store/msg";
import type { State } from "../store/state";

/// The top bar's Assistant button: opens and closes the panel beside the work (Ctrl or Cmd and the
/// full stop does the same).
export function AssistantButton({ state, dispatch }: { state: State; dispatch: (i: Intent) => void }) {
  const open = state.assistant.open;
  return (
    <button
      type="button" className={`btn${open ? " on" : ""}`} data-testid="assistant-toggle" aria-pressed={open}
      title="Show or hide the assistant (Ctrl or Cmd and .)" onClick={() => dispatch({ type: "toggleAssistant" })}
    >
      Assistant
    </button>
  );
}
