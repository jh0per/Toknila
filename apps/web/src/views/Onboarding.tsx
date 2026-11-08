import type { Intent } from "../store/msg";
import { ONBOARDING_DRAFT, draftOf } from "../store/planner";
import { fieldError } from "../store/settings";
import type { State } from "../store/state";
import { ADD_PROJECT_FORM, AddProject } from "./AddProject";
import { PromptField } from "./AskBox";
import { Checks } from "./Checks";
import { TrustNote } from "./TrustNote";

/// What the window shows when there is no project yet. It asks for the project's folder first (the
/// rest of what the folder tells is under Details), then checks this machine, then the one thing
/// to do in Claude Code, and the last step is describing what to build: adding the project leads
/// straight into the assistant, which gets that description.
export function Onboarding({ state, dispatch }: { state: State; dispatch: (i: Intent) => void }) {
  const s = state.settings;
  const text = draftOf(state, ONBOARDING_DRAFT);
  const submit = () => dispatch({ type: "submitProject" });
  const refused = fieldError(s, "repo_path");
  return (
    <div className="settings" data-testid="onboarding">
      <h1>Welcome to Toknila</h1>
      <p className="muted">
        You tell an assistant what you want to build. It plans the work as cards, and agents build them while you watch and
        accept the result. Four steps to start.
      </p>
      <h3 className="step">1. Choose the folder of your project</h3>
      <AddProject state={state} dispatch={dispatch} submit={false} />
      <h3 className="step">2. Check this machine</h3>
      <Checks state={state} dispatch={dispatch} />
      <h3 className="step">3. Trust the folder once</h3>
      <TrustNote />
      <h3 className="step">4. Describe your project</h3>
      <div className="card-block describe" data-testid="describe">
        <PromptField
          value={text} onChange={(t) => dispatch({ type: "editPlannerDraft", where: ONBOARDING_DRAFT, text: t })} onSubmit={submit}
          rows={3} label="Describe your project" testId="describe-input"
          placeholder="What is it, and what do you want to build or change first? You can also leave this empty."
        />
        {refused && <p className="error" role="alert">{refused}</p>}
        <button className="btn primary" type="submit" form={ADD_PROJECT_FORM} disabled={s.busy} data-testid="add-project-submit">
          {text.trim() ? "Add project and start planning" : "Add project"}
        </button>
      </div>
    </div>
  );
}
