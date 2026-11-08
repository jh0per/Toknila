import type { Intent } from "../store/msg";
import type { State } from "../store/state";

const TOOL: Record<string, string> = { git: "Git", claude: "Claude Code", gh: "GitHub CLI" };

/// The system checks: git, claude and gh, each with what was found and, when it is not fine, how to fix it.
export function Checks({ state, dispatch, title = true }: { state: State; dispatch: (i: Intent) => void; title?: boolean }) {
  const checks = state.settings.checks;
  return (
    <section className="card-block" data-testid="checks">
      {title && <h2>System checks</h2>}
      {!checks && <p className="muted">Checking…</p>}
      <ul className="checks">
        {checks?.checks.map((c) => (
          <li key={c.tool} data-tool={c.tool} data-status={c.status}>
            <span className={`mark st-${c.status}`} aria-hidden>{c.status === "ok" ? "✓" : "!"}</span>
            <b>{TOOL[c.tool] ?? c.tool}</b>
            <span className="detail">{c.detail}</span>
            {c.status !== "ok" && <p className="hint" data-testid={`hint-${c.tool}`}>{c.hint}</p>}
          </li>
        ))}
      </ul>
      <button className="btn" onClick={() => dispatch({ type: "recheck" })}>Check again</button>
    </section>
  );
}
