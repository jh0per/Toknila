import type { Intent } from "../store/msg";
import type { State } from "../store/state";
import { lastCheckText } from "../store/updates";

/// Updates: inside the desktop app, when Toknila last looked for a newer version and a button to
/// look now; in a browser there is no shell to ask, and it says so.
export function UpdatesSettings({ state, dispatch }: { state: State; dispatch: (i: Intent) => void }) {
  const { view, error } = state.updates;
  const failure = view && (error ?? view.error);
  return (
    <section className="card-block" data-testid="updates">
      {!view ? (
        <p className="muted" data-testid="updates-browser">
          Updates come with the Toknila desktop app. In a browser, install the newer version yourself.
        </p>
      ) : (
        <>
          <p>Toknila {view.current_version}</p>
          <p className="muted" data-testid="last-check">{lastCheckText(view.last_check)}</p>
          <p className="muted">A newer version downloads in the background; you choose when to restart.</p>
          <button className="btn" data-testid="check-updates" disabled={view.busy} onClick={() => dispatch({ type: "checkForUpdates" })}>
            {view.busy ? "Checking…" : "Check for updates"}
          </button>
          {failure && <p className="field-error" data-testid="update-error">{failure}</p>}
        </>
      )}
    </section>
  );
}
