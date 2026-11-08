import type { Project } from "../api/types";
import type { Intent } from "../store/msg";
import { fieldError } from "../store/settings";
import type { State } from "../store/state";

/// "Agents at once − 2 +": the project's agent slot limit, changed where the agents are listed.
/// It writes the project's `max_parallel` through the same save as Settings; the daemon judges the
/// number and its refusal shows under the stepper. A lowered limit lets running agents finish.
export function SlotStepper({ project, state, dispatch }: { project: Project; state: State; dispatch: (i: Intent) => void }) {
  const limit = project.max_parallel;
  const busy = state.settings.busy;
  const save = (n: number) => dispatch({ type: "saveProject", project: project.key, patch: { max_parallel: n } });
  // On the settings page the same refusal shows next to its field instead.
  const refused = state.route.name === "settings" ? undefined : fieldError(state.settings, "max_parallel");
  return (
    <>
      <div className="slots" data-testid="slot-stepper">
        <span id="slot-label" className="muted">Agents at once</span>
        <div className="stepper" role="group" aria-labelledby="slot-label">
          <button type="button" data-testid="slots-fewer" aria-label="Fewer agents at once" disabled={busy || limit <= 1} onClick={() => save(limit - 1)}>−</button>
          <output data-testid="slot-limit" aria-live="polite">{limit}</output>
          <button type="button" data-testid="slots-more" aria-label="More agents at once" disabled={busy} onClick={() => save(limit + 1)}>+</button>
        </div>
      </div>
      {refused && <p className="error side-note" role="alert" data-testid="slots-error">{refused}</p>}
    </>
  );
}
