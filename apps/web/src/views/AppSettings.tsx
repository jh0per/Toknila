import type { Intent } from "../store/msg";
import type { State } from "../store/state";
import { About } from "./About";
import { Checks } from "./Checks";
import { TrustNote } from "./TrustNote";

/// App: the system checks, the trust note, the version with updates and the `tok` command, where
/// Toknila keeps its files, and a way to add another project.
export function AppSettings({ state, dispatch }: { state: State; dispatch: (i: Intent) => void }) {
  const places = state.settings.places;
  return (
    <>
      <Checks state={state} dispatch={dispatch} />
      <TrustNote />
      <About state={state} dispatch={dispatch} />
      <section className="card-block" data-testid="places">
        <h2>Where things live</h2>
        <dl className="facts">
          <dt>Data</dt>
          <dd data-testid="place-home"><code>{places?.home ?? "…"}</code></dd>
          <dt>Ships</dt>
          <dd data-testid="place-ships"><code>{places?.ships ?? "…"}</code></dd>
          <dt>Profiles</dt>
          <dd data-testid="place-profiles"><code>{places?.profiles ?? "…"}</code></dd>
        </dl>
        <p className="muted">The data folder holds the database and run logs; a ship is a card's git worktree.</p>
      </section>
      <section className="card-block">
        <h2>Another project</h2>
        <p className="muted">Point Toknila at another repository; its key and name come from the folder.</p>
        <button
          type="button" className="btn" data-testid="open-add-project"
          onClick={() => dispatch({ type: "openDialog", dialog: { kind: "addProject" } })}
        >
          Add a project…
        </button>
      </section>
    </>
  );
}
