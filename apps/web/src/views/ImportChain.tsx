import type { Project } from "../api/types";
import type { Intent } from "../store/msg";
import { formError } from "../store/settings";
import type { State } from "../store/state";

/// Import a chain file (TOML) into the project: the browser reads the chosen file, the daemon
/// validates and imports it. Spec folders stay with `tok chain import <folder>`.
export function ImportChain({ project, state, dispatch }: { project: Project; state: State; dispatch: (i: Intent) => void }) {
  const s = state.settings;
  return (
    <section className="card-block" data-testid="import-chain">
      <h2>Import a chain file</h2>
      <p className="muted">Optional. A chain file plans the project's cards; it is read once and then lives in the database.</p>
      <input
        type="file" accept=".toml,text/plain" data-testid="chain-file" disabled={s.busy}
        onChange={(e) => {
          const file = e.target.files?.[0];
          if (file) void file.text().then((toml) => dispatch({ type: "importChain", project: project.key, toml }));
          e.target.value = "";
        }}
      />
      {s.imported && <p className="saved" data-testid="imported">Imported chain {s.imported}.</p>}
      {formError(s) && <p className="error" role="alert">{formError(s)}</p>}
    </section>
  );
}
