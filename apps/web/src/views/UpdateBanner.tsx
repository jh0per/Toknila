import type { Intent } from "../store/msg";
import type { State } from "../store/state";
import { banner } from "../store/updates";

/// A quiet note in the corner when a new version is downloaded and waits for a restart. It never
/// restarts by itself; with agents running, "Restart to update" first asks again.
export function UpdateBanner({ state, dispatch }: { state: State; dispatch: (i: Intent) => void }) {
  const b = banner(state.updates);
  if (!b) return null;
  const { warning } = state.updates;
  return (
    <aside className="update-banner" role="status" data-testid="update-banner">
      <b data-testid="update-title">{b.title}</b>
      {b.notes && <p className="muted notes" data-testid="update-notes">{b.notes}</p>}
      {warning && <p className="hint" data-testid="update-warning">{warning}</p>}
      <div className="btns">
        {warning ? (
          <button className="btn primary" data-testid="update-anyway" onClick={() => dispatch({ type: "restartAnyway" })}>
            Restart anyway
          </button>
        ) : (
          <button className="btn primary" data-testid="update-restart" onClick={() => dispatch({ type: "restartToUpdate" })}>
            Restart to update
          </button>
        )}
        <button className="btn" data-testid="update-later" onClick={() => dispatch({ type: "updateLater" })}>
          Later
        </button>
      </div>
      {state.updates.error && <p className="field-error">{state.updates.error}</p>}
    </aside>
  );
}
