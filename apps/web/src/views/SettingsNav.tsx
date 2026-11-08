import type { Project } from "../api/types";
import type { Intent } from "../store/msg";
import { type SettingsSection, settingsSection } from "../store/route";
import type { State } from "../store/state";
import { NavLink } from "./Link";

const PROJECT_ITEMS: { section: SettingsSection; label: string }[] = [
  { section: "general", label: "General" },
  { section: "agents", label: "Agents" },
  { section: "review", label: "Review and merge" },
  { section: "plans", label: "Plans" },
  { section: "danger", label: "Danger zone" },
];

/// The section list of the settings screen: App and Profiles, then the focused project's sections.
/// Every entry is a route, so a section has its own address.
export function SettingsNav({ state, dispatch, project }: {
  state: State;
  dispatch: (i: Intent) => void;
  project: Project | undefined;
}) {
  const r = state.route;
  const at = r.name === "settings" ? settingsSection(r) : "app";
  const item = (section: SettingsSection, label: string) => (
    <li key={section}>
      <NavLink
        route={{ name: "settings", project: project?.key, section }} dispatch={dispatch}
        className={`item${at === section ? " sel" : ""}`} testId={`settings-nav-${section}`}
      >
        <span className="t">{label}</span>
      </NavLink>
    </li>
  );
  return (
    <nav className="settings-nav" aria-label="Settings sections" data-testid="settings-nav">
      <ul>
        {item("app", "App")}
        {item("profiles", "Profiles")}
      </ul>
      {project && (
        <>
          <h3 data-testid="settings-nav-project">Project {project.key}</h3>
          <ul>{PROJECT_ITEMS.map((i) => item(i.section, i.label))}</ul>
        </>
      )}
    </nav>
  );
}
