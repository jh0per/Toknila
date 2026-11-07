import type { FoundPlan } from "../api/types";
import { type PlanOutcome, importing } from "../store/imports";
import type { Intent } from "../store/msg";
import type { State } from "../store/state";

function Outcome({ o }: { o: PlanOutcome | undefined }) {
  if (!o) return null;
  if (o.state === "running") return <span className="muted" data-testid="plan-outcome">importing…</span>;
  if (o.state === "done") return <span className="saved" data-testid="plan-outcome">imported as {o.chains.join(", ")}</span>;
  return <span className="error" role="alert" data-testid="plan-outcome">{o.message}</span>;
}

const what = (p: FoundPlan): string =>
  [
    p.kind === "spec_folder" ? "spec folder" : "chain file",
    p.cards == null ? "does not parse" : `${p.cards} card${p.cards === 1 ? "" : "s"}`,
    p.imported ? "already imported" : "",
  ].filter(Boolean).join(" · ");

/// The plans in a project's repository (`specs/*/tasks.md`, `plans/*.toml`): scanned on request,
/// ticked for import (all but those already imported), imported one by one with each outcome.
export function FoundPlans({ project, state, dispatch }: { project: string; state: State; dispatch: (i: Intent) => void }) {
  const i = state.imports;
  const f = i.found?.project === project ? i.found : undefined;
  const scanning = i.scanning === project;
  const scan = (label: string) => (
    <button type="button" className="btn" data-testid="scan-plans" disabled={scanning} onClick={() => dispatch({ type: "scanPlans", project })}>
      {scanning ? "Scanning…" : label}
    </button>
  );
  if (!f) {
    return (
      <div className="found" data-testid="found-plans">
        {scan("Scan the repository")}
        {i.scanError && <p className="error" role="alert">{i.scanError}</p>}
      </div>
    );
  }
  const busy = importing(f);
  const finished = Object.keys(f.outcomes).length > 0 && !busy;
  const n = f.plans.length;
  return (
    <div className="found" data-testid="found-plans">
      <p data-testid="found-count">
        {n === 0 ? "No plans in this repository (specs/*/tasks.md, plans/*.toml)." : `Found ${n} plan${n === 1 ? "" : "s"} in this repository`}
      </p>
      {f.behind && (
        <p className="muted" data-testid="behind-note">
          {f.behind.branch} is {f.behind.behind} commit{f.behind.behind === 1 ? "" : "s"} behind {f.behind.upstream}: these plans are read from your working copy, which may be older than the upstream's.
        </p>
      )}
      {f.specify_dir && (
        <p className="muted" data-testid="specify-note">The repository has a .specify/ folder: its spec folders under specs/ are listed here.</p>
      )}
      {n > 0 && (
        <ul className="plans">
          {f.plans.map((p) => (
            <li key={p.path} data-plan={p.relative}>
              <label className="check">
                <input
                  type="checkbox" checked={f.selected.includes(p.path)} disabled={busy}
                  onChange={(e) => dispatch({ type: "togglePlan", path: p.path, on: e.target.checked })}
                />
                <span className="key">{p.relative}</span>
                <span className="muted">{what(p)}</span>
              </label>
              <Outcome o={f.outcomes[p.path]} />
            </li>
          ))}
        </ul>
      )}
      <div className="row">
        {n > 0 && (
          <button
            type="button" className="btn primary" data-testid="import-selected" disabled={busy || f.selected.length === 0}
            onClick={() => dispatch({ type: "importSelected" })}
          >
            Import selected
          </button>
        )}
        <button type="button" className="btn" data-testid="skip-plans" disabled={busy} onClick={() => dispatch({ type: "skipPlans" })}>
          {finished || n === 0 ? "Close" : "Skip"}
        </button>
        {scan("Scan again")}
      </div>
    </div>
  );
}
