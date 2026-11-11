import type { Suggestion } from "../api/types";
import type { Intent } from "../store/msg";
import type { State } from "../store/state";
import { suggestionIntents, suggestionLabel, suggestionRoute } from "../store/suggestion";
import { NavLink } from "./Link";

/// The planner's next step: its text and the one button of its action (the person clicks it; the
/// planner never does). Shown at the top of the assistant's screen and as the first row of the
/// inbox.
export function SuggestionRow({ state, suggestion, dispatch }: {
  state: State;
  suggestion: Suggestion;
  dispatch: (i: Intent) => void;
}) {
  const label = suggestionLabel(suggestion);
  const route = suggestionRoute(state, suggestion);
  return (
    <div className="suggestion" data-testid="suggestion" data-project={suggestion.project} data-action={suggestion.action ?? "none"}>
      <span className="key">Next step</span>
      <span className="t" data-testid="suggestion-text">{suggestion.text}</span>
      <span className="grow" />
      {route && suggestion.action !== "answer" && (
        <NavLink route={route} dispatch={dispatch} className="key">{suggestion.card}</NavLink>
      )}
      {label && (
        <button
          type="button" className="btn primary" data-testid="suggestion-button"
          onClick={() => suggestionIntents(state, suggestion).forEach(dispatch)}
        >
          {label}
        </button>
      )}
    </div>
  );
}
