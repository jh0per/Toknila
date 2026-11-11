import type { ReleaseStep } from "../api/types";
import type { Intent } from "../store/msg";
import type { State } from "../store/state";

const LABEL: Record<ReleaseStep, string> = {
  check: "Check",
  bump: "Bump the version",
  tag: "Tag",
  push: "Push",
};

const MARK = { pending: "○", running: "…", done: "✓", failed: "✗", interrupted: "⏸" } as const;

/// The Release dialog of a finished chain: the version (suggested from the last tag), the steps
/// the project has, and then each step as it runs with its output. Nothing is tagged or pushed
/// before the click on Release; a failure stops the run and names the step.
export function ReleaseDialog({ chain: id, state, dispatch }: { chain: string; state: State; dispatch: (i: Intent) => void }) {
  const chain = state.chains[id];
  const r = state.release;
  if (!chain) return null;
  const running = r.busy || r.run?.state === "running";
  const done = r.run?.state === "done";
  const interrupted = r.run?.state === "interrupted";
  const from = interrupted && r.run?.resume_from ? LABEL[r.run.resume_from] : undefined;
  const offer = r.offer;
  return (
    <div className="card-block form" data-testid="release-dialog">
      <h2>Release {chain.chain.name}</h2>
      {!offer && !r.error && <p className="muted">Looking at the repository…</p>}
      {offer && (
        <>
          <label className="field">
            <span>Version{offer.last_tag ? ` (last tag ${offer.last_tag})` : ""}</span>
            <input
              value={r.version}
              disabled={running || done || interrupted}
              data-testid="release-version"
              onChange={(e) => dispatch({ type: "setReleaseVersion", version: e.target.value })}
            />
          </label>
          <p className="muted">
            On the branch {offer.base}. Nothing is tagged or pushed until you click Release.
          </p>
          <ol className="release-steps" data-testid="release-steps">
            {(r.run?.steps ?? offer.steps.map((step) => ({ step, label: LABEL[step], state: "pending" as const, output: "" }))).map((s) => (
              <li key={s.step} data-step={s.step} data-state={s.state}>
                <b>{MARK[s.state]} {s.label}</b>
                {s.output && <pre className="output">{s.output}</pre>}
              </li>
            ))}
          </ol>
        </>
      )}
      {done && r.run && (
        <p data-testid="release-done">Released {r.run.version} as the tag {r.run.tag}.</p>
      )}
      {r.run?.state === "failed" && r.run.error && (
        <div className="error" role="alert" data-testid="release-failed">
          <p>{r.run.error}</p>
          <p>Nothing after that step ran. Fix it, then release again.</p>
        </div>
      )}
      {interrupted && (
        <div className="error" role="alert" data-testid="release-interrupted">
          <p>The daemon stopped while this release was going{from ? `, at the step ${from}` : ""}.</p>
          <p>Steps that passed are not done again.</p>
        </div>
      )}
      {r.error && (
        <div className="error" role="alert" data-testid="release-error"><p>{r.error}</p></div>
      )}
      <div className="row">
        {!done && (
          <button
            type="button" className="btn primary" data-testid="release-confirm"
            disabled={!offer || !offer.releasable || running || !r.version.trim()}
            onClick={() => dispatch({ type: "startRelease", chain: id, version: r.version, ...(interrupted ? { resume: true } : {}) })}
          >
            {interrupted ? `Run again from ${from ?? "the start"}` : r.run?.state === "failed" ? "Release again" : "Release"}
          </button>
        )}
        <button type="button" className="btn" data-testid="release-close" onClick={() => dispatch({ type: "closeDialog" })}>
          {done ? "Close" : "Cancel"}
        </button>
      </div>
    </div>
  );
}
