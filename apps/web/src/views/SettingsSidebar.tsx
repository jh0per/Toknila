import type { Intent } from "../store/msg";
import { type SettingsSection, isProjectSection, settingsSection } from "../store/route";
import { backRoute } from "../store/settingsNav";
import { type State, currentProject } from "../store/state";
import { NavLink } from "./Link";

const PAGES: { id: string; label: string; section: SettingsSection | undefined }[] = [
  { id: "projects", label: "Projects", section: undefined },
  { id: "profiles", label: "Agents", section: "profiles" },
  { id: "updates", label: "Updates", section: "updates" },
  { id: "about", label: "About", section: "about" },
  { id: "checks", label: "System checks", section: "checks" },
];

/// While in settings the sidebar is a back arrow and one flat list of pages. Every entry is a
/// route, so a page has its own address.
export function SettingsSidebar({ state, dispatch }: { state: State; dispatch: (i: Intent) => void }) {
  const project = currentProject(state);
  const at = state.route.name === "settings" ? settingsSection(state.route) : "general";
  const here = isProjectSection(at) ? "projects" : at;
  return (
    <nav className="sidebar settings-side" aria-label="Settings" data-testid="settings-sidebar">
      <NavLink route={backRoute(state.settingsFrom)} dispatch={dispatch} className="back-link" testId="settings-back">
        <span aria-hidden>←</span> Back to work
      </NavLink>
      <h2 className="side-title">Settings</h2>
      <ul>
        {PAGES.map((p) => (
          <li key={p.id}>
            <NavLink
              route={{ name: "settings", project: project?.key, section: p.section }} dispatch={dispatch}
              className={`item${here === (p.section ?? "projects") ? " sel" : ""}`} testId={`settings-page-${p.id}`}
            >
              <span className="t">{p.label}</span>
            </NavLink>
          </li>
        ))}
      </ul>
      <p className="muted side-note shortcut-hint">
        <kbd>{navigator.platform.toLowerCase().includes("mac") ? "⌘" : "Ctrl"}</kbd> <kbd>,</kbd> opens settings from anywhere
      </p>
    </nav>
  );
}
