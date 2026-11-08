import type { Intent } from "../store/msg";
import { settingsSection } from "../store/route";
import { type State, currentProject } from "../store/state";
import { AppSettings } from "./AppSettings";
import { PlansSection, DangerSection } from "./PlansSection";
import { AgentsSection, GeneralSection, ReviewSection } from "./ProjectSections";
import { ProfilesSettings } from "./ProfilesSettings";
import { SettingsNav } from "./SettingsNav";

/// Settings: a section list on the left (App, Profiles, and the focused project's General, Agents,
/// Review and merge, Plans, Danger zone) and the section on the right. Each section has its own
/// address (`/settings`, `/projects/KEY/settings/agents`, ...).
export function Settings({ state, dispatch }: { state: State; dispatch: (i: Intent) => void }) {
  const project = currentProject(state);
  const section = state.route.name === "settings" ? settingsSection(state.route) : "app";
  // The form restarts from the daemon's values whenever they change (after a save, after a restart).
  const stamp = project && [
    project.id, project.default_profile, project.max_parallel, project.default_review_mode, project.default_reviewer,
    project.work_branch, project.check_command, project.agent_model, project.merge.mode, project.merge.pr_title,
    project.merge.method, state.settings.profiles.length,
  ].join("|");
  const form = { key: stamp, state, dispatch };
  return (
    <div className="settings-screen" data-testid="settings">
      <SettingsNav state={state} dispatch={dispatch} project={project} />
      <div className="settings-body" data-section={section}>
        <h1>Settings</h1>
        {section === "app" && <AppSettings state={state} dispatch={dispatch} />}
        {section === "profiles" && <ProfilesSettings state={state} />}
        {project && section === "general" && <GeneralSection project={project} {...form} />}
        {project && section === "agents" && <AgentsSection project={project} {...form} />}
        {project && section === "review" && <ReviewSection project={project} {...form} />}
        {project && section === "plans" && <PlansSection project={project} state={state} dispatch={dispatch} />}
        {project && section === "danger" && <DangerSection project={project} state={state} dispatch={dispatch} />}
      </div>
    </div>
  );
}
