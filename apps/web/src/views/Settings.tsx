import type { Intent } from "../store/msg";
import { isProjectSection, settingsSection } from "../store/route";
import { type State, currentProject } from "../store/state";
import { About } from "./About";
import { Checks } from "./Checks";
import { ProfilesSettings } from "./ProfilesSettings";
import { ProjectSettings } from "./ProjectSettings";
import { TrustNote } from "./TrustNote";
import { UpdatesSettings } from "./UpdatesSettings";

const TITLE = { projects: "Projects", profiles: "Agents", updates: "Updates", about: "About", checks: "System checks" } as const;

/// Settings: one page of the flat list (Projects, Agents, Updates, About, System checks), shown
/// beside the settings sidebar. Each page and each project section has an address of its own
/// (`/settings`, `/settings/updates`, `/projects/KEY/settings/review`, ...).
export function Settings({ state, dispatch }: { state: State; dispatch: (i: Intent) => void }) {
  const project = currentProject(state);
  const section = state.route.name === "settings" ? settingsSection(state.route) : "general";
  const page = isProjectSection(section) ? "projects" : section;
  return (
    <div className="settings-screen" data-testid="settings">
      <div className="settings-body" data-section={section}>
        <h1>{TITLE[page]}</h1>
        {isProjectSection(section) && <ProjectSettings state={state} dispatch={dispatch} project={project} section={section} />}
        {section === "profiles" && <ProfilesSettings state={state} dispatch={dispatch} />}
        {section === "updates" && <UpdatesSettings state={state} dispatch={dispatch} />}
        {section === "about" && <About state={state} dispatch={dispatch} />}
        {section === "checks" && (
          <>
            <Checks state={state} dispatch={dispatch} title={false} />
            <TrustNote />
          </>
        )}
      </div>
    </div>
  );
}
