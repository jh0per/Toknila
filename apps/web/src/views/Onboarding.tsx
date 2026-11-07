import type { Intent } from "../store/msg";
import type { State } from "../store/state";
import { AddProject } from "./AddProject";
import { Checks } from "./Checks";
import { TrustNote } from "./TrustNote";

/// What the window shows when there is no project yet: the checks, then a form to add the first one.
export function Onboarding({ state, dispatch }: { state: State; dispatch: (i: Intent) => void }) {
  return (
    <div className="settings" data-testid="onboarding">
      <h1>Welcome to Toknila</h1>
      <p className="muted">Three steps: check this machine, point Toknila at the repository you work on, and trust that repository once in Claude Code.</p>
      <h3 className="step">1. Check this machine</h3>
      <Checks state={state} dispatch={dispatch} />
      <h3 className="step">2. Add your first project</h3>
      <AddProject state={state} dispatch={dispatch} />
      <h3 className="step">3. Trust the repository once</h3>
      <TrustNote />
      <p className="muted">Next you can import a chain file for it, then set its limits and review mode.</p>
    </div>
  );
}
