import type { ReactNode } from "react";
import type { PatchProject, Project } from "../api/types";
import type { Intent } from "../store/msg";
import { formError } from "../store/settings";
import type { State } from "../store/state";

/// The role names of the profile catalog for a select, keeping a saved name the catalog no longer
/// lists selectable, so saving never changes it silently.
export function profileOptions(state: State, role: string, current: string): string[] {
  const all = state.settings.profiles.filter((l) => l.profile.role === role).map((l) => l.profile.name);
  return current && !all.includes(current) ? [current, ...all] : all;
}

/// One section of a project's settings: a form that saves the fields of that section only.
/// `patch` reads them when Save is pressed; the daemon judges them and the refusal shows next to
/// the field it names (or under the form).
export function SectionForm({ project, title, testId, state, dispatch, patch, children }: {
  project: Project;
  title: string;
  testId: string;
  state: State;
  dispatch: (i: Intent) => void;
  patch: () => PatchProject;
  children: ReactNode;
}) {
  const s = state.settings;
  return (
    <form
      className="card-block form"
      noValidate
      data-testid={testId}
      onSubmit={(e) => {
        e.preventDefault();
        dispatch({ type: "saveProject", project: project.key, patch: patch() });
      }}
    >
      <h2>{title}</h2>
      {children}
      {formError(s) && <p className="error" role="alert">{formError(s)}</p>}
      <div className="row">
        <button className="btn primary" type="submit" disabled={s.busy} data-testid="save-settings">Save</button>
        {s.saved && <span className="saved" data-testid="saved">Saved</span>}
      </div>
    </form>
  );
}
