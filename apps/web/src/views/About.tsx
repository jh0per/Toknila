import type { Intent } from "../store/msg";
import type { State } from "../store/state";
import { lastCheckText } from "../store/updates";

/// About: the version, and the action that links the bundled `tok` command into `~/.local/bin`.
export function About({ state, dispatch }: { state: State; dispatch: (i: Intent) => void }) {
  const { cli, cliError } = state.settings;
  return (
    <section className="card-block" data-testid="about">
      <h2>About</h2>
      <p data-testid="version">Toknila {cli?.version ?? "…"}</p>
      <UpdateLine state={state} dispatch={dispatch} />
      {cli && !cli.bundled && (
        <p className="muted">
          The <code>tok</code> command ships inside the Toknila app; this daemon has none next to it.
        </p>
      )}
      {cli?.bundled && cli.installed && (
        <p data-testid="cli-installed">
          <code>tok</code> is installed: <code>{cli.link}</code>
        </p>
      )}
      {cli?.bundled && cli.installed && !cli.on_path && (
        <p className="hint" data-testid="cli-path-hint">
          The folder of that link is not on your PATH yet. Add this line to your shell profile (for
          example <code>~/.zshrc</code>) and open a new terminal: <code>export PATH="$HOME/.local/bin:$PATH"</code>
        </p>
      )}
      {cli?.bundled && !cli.installed && (
        <>
          <p className="muted">
            Links <code>{cli.link}</code> to the <code>tok</code> in this app. Nothing outside your home
            folder is written.
          </p>
          <button className="btn" data-testid="cli-install" onClick={() => dispatch({ type: "installCli" })}>
            Install the <code>tok</code> command
          </button>
        </>
      )}
      {cliError && <p className="field-error" data-testid="cli-error">{cliError}</p>}
    </section>
  );
}

/// The update line of About, only inside the desktop app (a browser has no shell to ask).
function UpdateLine({ state, dispatch }: { state: State; dispatch: (i: Intent) => void }) {
  const { view, error } = state.updates;
  if (!view) return null;
  const failure = error ?? view.error;
  return (
    <>
      <p className="muted" data-testid="last-check">{lastCheckText(view.last_check)}</p>
      <button className="btn" data-testid="check-updates" disabled={view.busy} onClick={() => dispatch({ type: "checkForUpdates" })}>
        {view.busy ? "Checking…" : "Check for updates"}
      </button>
      {failure && <p className="field-error" data-testid="update-error">{failure}</p>}
    </>
  );
}
