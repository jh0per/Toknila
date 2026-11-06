import type { Intent } from "../store/msg";
import { type State, currentProject } from "../store/state";
import { AddProject } from "./AddProject";
import { Checks } from "./Checks";
import { ImportChain } from "./ImportChain";
import { ProjectSettings } from "./ProjectSettings";

/// Settings: the system checks, the settings of the project in focus, a chain file to import, and
/// a form to add another project.
export function Settings({ state, dispatch }: { state: State; dispatch: (i: Intent) => void }) {
  const project = currentProject(state);
  // The form restarts from the daemon's values whenever they change (after a save, after a restart).
  const stamp = project && [
    project.id, project.default_profile, project.max_parallel, project.default_review_mode,
    project.default_reviewer, project.work_branch, state.settings.profiles.length,
  ].join("|");
  return (
    <div className="settings" data-testid="settings">
      <h1>Settings</h1>
      <Checks state={state} dispatch={dispatch} />
      {project && <ProjectSettings key={stamp} project={project} state={state} dispatch={dispatch} />}
      {project && <ImportChain project={project} state={state} dispatch={dispatch} />}
      <AddProject state={state} dispatch={dispatch} />
    </div>
  );
}
