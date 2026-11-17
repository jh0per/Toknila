import { useEffect, useState } from "react";
import { get, put } from "../api/http";
import type { MachineSettings as Settings } from "../api/types";

/// What the machine page shows: how many checks run at once, with a stepper that asks the daemon.
export function MachineView({ max, busy, error, save }: { max: number | undefined; busy: boolean; error?: string; save: (n: number) => void }) {
  return (
    <section className="card-block" data-testid="machine">
      <div className="slots" data-testid="checks-stepper">
        <span id="checks-label" className="muted">Checks at once</span>
        <div className="stepper" role="group" aria-labelledby="checks-label">
          <button type="button" data-testid="checks-fewer" aria-label="Fewer checks at once" disabled={busy || max === undefined || max <= 1} onClick={() => max !== undefined && save(max - 1)}>−</button>
          <output data-testid="checks-limit" aria-live="polite">{max ?? "…"}</output>
          <button type="button" data-testid="checks-more" aria-label="More checks at once" disabled={busy || max === undefined} onClick={() => max !== undefined && save(max + 1)}>+</button>
        </div>
      </div>
      <p className="muted">
        A second check starts only while the machine's one-minute load is below its number of cores; a check held back says so on its card. 1 keeps them one at a time.
      </p>
      {error && <p className="field-error" role="alert" data-testid="checks-error">{error}</p>}
    </section>
  );
}

/// Settings → Machine: what the daemon does on this computer, whichever the project. The number is
/// the daemon's; a refused or failed save leaves the old one and says why.
export function MachineSettings() {
  const [max, setMax] = useState<number | undefined>();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | undefined>();
  const say = (e: unknown) => setError(e instanceof Error ? e.message : String(e));
  useEffect(() => {
    get<Settings>("/machine/settings").then((s) => setMax(s.max_checks)).catch(say);
  }, []);
  const save = (n: number) => {
    setBusy(true);
    setError(undefined);
    put<Settings>("/machine/settings", { max_checks: n })
      .then((s) => setMax(s.max_checks))
      .catch(say)
      .finally(() => setBusy(false));
  };
  return <MachineView max={max} busy={busy} error={error} save={save} />;
}
