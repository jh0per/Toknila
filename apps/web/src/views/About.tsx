import type { Intent } from "../store/msg";
import type { State } from "../store/state";

/// About: the version, and the action that links the bundled `tok` command into `~/.local/bin`.
export function About({ state, dispatch }: { state: State; dispatch: (i: Intent) => void }) {
  const { cli, cliError } = state.settings;
  return (
    <section className="card-block" data-testid="about">
      <h2>About</h2>
      <p data-testid="version">Toknila {cli?.version ?? "…"}</p>
      <h3 className="sub">The <code>tok</code> command</h3>
      <p className="muted">A command for the terminal that does what this window does: list cards, start them, answer them.</p>
      {cli && !cli.bundled && (
        <p className="muted">The <code>tok</code> command comes with the Toknila app; this daemon has none next to it.</p>
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
