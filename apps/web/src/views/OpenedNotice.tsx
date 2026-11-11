import type { Intent } from "../store/msg";
import type { State } from "../store/state";

/// A small notice in the corner: the assistant opened this card. Back returns to the page the
/// person was on; it goes by itself after a while.
export function OpenedNotice({ state, dispatch }: { state: State; dispatch: (i: Intent) => void }) {
  const o = state.opened;
  if (!o) return null;
  return (
    <aside className="opened-notice" role="status" data-testid="opened-notice">
      <span data-testid="opened-text">The assistant opened {o.key}</span>
      <button className="btn" data-testid="opened-back" onClick={() => dispatch({ type: "backFromOpened" })}>
        Back
      </button>
      <button className="btn" aria-label="Dismiss" data-testid="opened-dismiss" onClick={() => dispatch({ type: "dismissOpened" })}>
        ×
      </button>
    </aside>
  );
}
