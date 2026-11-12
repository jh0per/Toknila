import type { ReactNode } from "react";
import type { Intent } from "../store/msg";
import { ONBOARDING_DRAFT, draftOf } from "../store/planner";
import { addBusy, fieldError } from "../store/settings";
import type { State } from "../store/state";
import { ADD_PROJECT_FORM, AddProject } from "./AddProject";
import { PromptField } from "./AskBox";
import { Checks } from "./Checks";
import { NavLink } from "./Link";
import { TrustNote } from "./TrustNote";

/// One numbered step of the page: its title, then what it asks for.
function Step({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="step-section">
      <h3 className="step">{title}</h3>
      {children}
    </section>
  );
}

/// What the window shows when there is no project yet. It asks for the project's folder first (the
/// rest of what the folder tells is under Details), then checks this machine, then the one thing
/// to do in Claude Code, and the last step is describing what to build: adding the project leads
/// straight to the roadmap with the assistant beside it, which gets that description.
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
        accept the result. Four steps to start.{" "}
        <NavLink route={{ name: "guide" }} dispatch={dispatch} className="inline-link" testId="onboarding-guide">How Toknila works</NavLink>
      </p>
      <Step title="1. Choose the folder of your project">
        <AddProject state={state} dispatch={dispatch} submit={false} />
      </Step>
      <Step title="2. Check this machine">
        <Checks state={state} dispatch={dispatch} />
      </Step>
      <Step title="3. Trust the folder once">
        <TrustNote />
      </Step>
      <Step title="4. Describe your project">
        <div className="card-block describe" data-testid="describe">
          <PromptField
            value={text} onChange={(t) => dispatch({ type: "editPlannerDraft", where: ONBOARDING_DRAFT, text: t })} onSubmit={submit}
            rows={3} label="Describe your project" testId="describe-input"
            placeholder="What is it, and what do you want to build or change first? You can also leave this empty."
          />
          {refused && <p className="error" role="alert">{refused}</p>}
          <div className="row end">
            <button className="btn primary" type="submit" form={ADD_PROJECT_FORM} disabled={addBusy(s)} data-testid="add-project-submit">
              {text.trim() ? "Add project and start planning" : "Add project"}
            </button>
          </div>
        </div>
      </Step>
    </div>
  );
}
