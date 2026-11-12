import { useCallback, useEffect, useState } from "react";
import { get, post } from "../api/http";
import type { Stray, Strays as StraysList, StraysCleaned } from "../api/types";
import { cleanedText, strayText } from "../store/strays";

/// Leftover ships: the workspaces and `ship/*` branches of a project that no live card owns
/// (cancelled or deleted cards), listed with a Clean up button. The list is read when the
/// section opens and after every clean up; Clean up looks again before it removes anything.
export function Strays({ project }: { project: string }) {
  const [items, setItems] = useState<Stray[] | undefined>();
  const [note, setNote] = useState<string | undefined>();
  const [error, setError] = useState<string | undefined>();
  const [busy, setBusy] = useState(false);
  const load = useCallback(async () => {
    try {
      setItems((await get<StraysList>(`/projects/${project}/strays`)).items);
      setError(undefined);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, [project]);
  useEffect(() => {
    void load();
  }, [load]);
  const clean = async () => {
    setBusy(true);
    setNote(undefined);
    try {
      const done = await post<StraysCleaned>(`/projects/${project}/strays/clean`, {});
      setNote(cleanedText(done.removed.length, done.warnings.length));
      setError(done.warnings.length > 0 ? done.warnings.join("; ") : undefined);
      await load();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };
  return (
    <section data-testid="strays">
      <h3 className="sub">Leftover workspaces</h3>
      <p className="muted">
        Workspaces and branches that no story owns any more, left by merged, cancelled or deleted stories. Clean up deletes them,
        and the commits on a branch with it.
      </p>
      {items && items.length === 0 && <p className="muted" data-testid="strays-none">Nothing left over.</p>}
      {items && items.length > 0 && (
        <>
          <ul data-testid="strays-list">
            {items.map((s) => <li key={`${s.kind}:${s.name}`}><code>{strayText(s)}</code></li>)}
          </ul>
          <button type="button" className="btn danger" disabled={busy} data-testid="strays-clean" onClick={() => void clean()}>
            {busy ? "Cleaning…" : "Clean up"}
          </button>
        </>
      )}
      {note && <p className="muted" role="status" data-testid="strays-note">{note}</p>}
      {error && <p className="field-error" role="alert" data-testid="strays-error">{error}</p>}
    </section>
  );
}
