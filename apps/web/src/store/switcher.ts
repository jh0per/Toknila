// The project switcher of the top bar: which route a chosen project opens, which projects the
// typed filter keeps, and the key that opens it. Pure; the menu is `views/ProjectSwitcher.tsx`.

import type { Project } from "../api/types";
import { type Route, isProjectSection } from "./route";
import type { State } from "./state";

/// What the person did.
export type SwitcherIntent = { type: "toggleSwitcher" } | { type: "closeSwitcher" };

/// How many other projects the sidebar lists as rows before it folds the rest into "+ N more".
export const SIDEBAR_OTHERS = 3;

/// The reducer of the switcher menu.
export function reduceSwitcher(s: State, msg: SwitcherIntent): State {
  return { ...s, switcherOpen: msg.type === "toggleSwitcher" ? !s.switcherOpen : false };
}

/// The route that choosing `project` opens: the screen the person is on, in the other project,
/// when that screen exists there (the roadmap, the assistant, the inbox, the settings and their
/// section); an epic or a story belongs to one project, so any other screen opens the roadmap.
export function switchRoute(from: Route, project: string): Route {
  switch (from.name) {
    case "planner": return { name: "planner", project };
    case "inbox": return { name: "inbox", project };
    case "settings":
      return from.section && isProjectSection(from.section) && from.section !== "general"
        ? { name: "settings", project, section: from.section }
        : { name: "settings", project };
    default: return { name: "project", project };
  }
}

/// The projects whose name or key contains the typed text, in their order; blank keeps all.
export function filterProjects(projects: Project[], text: string): Project[] {
  const q = text.trim().toLowerCase();
  if (!q) return projects;
  return projects.filter((p) => p.name.toLowerCase().includes(q) || p.key.toLowerCase().includes(q));
}

/// Whether the press opens the switcher: Cmd/Ctrl+K. (Cmd/Ctrl+P is the browser's print.)
export function asksForSwitcher(e: { key: string; ctrlKey: boolean; metaKey: boolean; altKey: boolean; shiftKey: boolean }): boolean {
  return e.key.toLowerCase() === "k" && (e.ctrlKey || e.metaKey) && !e.altKey && !e.shiftKey;
}

/// The other projects the sidebar lists as collapsed rows, and how many it folds into "+ N more".
export function sidebarOthers(others: Project[]): { shown: Project[]; more: number } {
  if (others.length <= SIDEBAR_OTHERS) return { shown: others, more: 0 };
  return { shown: others.slice(0, SIDEBAR_OTHERS), more: others.length - SIDEBAR_OTHERS };
}
