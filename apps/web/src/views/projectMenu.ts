// What the right-click menu of the sidebar's projects offers. On a project row: its settings and
// archiving (or bringing it back); everywhere: adding a project. Phase 2 adds "Delete project..."
// to a row's entries here, next to Archive.

import type { Project } from "../api/types";
import type { Intent } from "../store/msg";

export interface ProjectMenuEntry {
  label: string;
  testId: string;
  intent: Intent;
  danger?: boolean;
}

const addProject: ProjectMenuEntry = {
  label: "Add project…",
  testId: "menu-add-project",
  intent: { type: "openDialog", dialog: { kind: "addProject" } },
};

/// The entries for a right click on `project`'s row, or on the section when `project` is absent.
export function projectMenu(project: Project | undefined): ProjectMenuEntry[] {
  if (!project) return [addProject];
  const archived = Boolean(project.archived);
  return [
    {
      label: "Project settings",
      testId: "menu-project-settings",
      intent: { type: "navigate", route: { name: "settings", project: project.key } },
    },
    archived
      ? {
        label: "Unarchive",
        testId: "menu-unarchive",
        intent: { type: "archiveProject", project: project.key, archived: false },
      }
      : {
        label: "Archive",
        testId: "menu-archive",
        intent: { type: "archiveProject", project: project.key, archived: true },
        danger: true,
      },
    addProject,
  ];
}
