// What the right-click menu of the sidebar's projects offers. On a project row: its settings,
// adding a project, archiving (or bringing it back) and deleting; on the section: adding a project.

import type { Project } from "../api/types";
import type { Intent } from "../store/msg";

export interface ProjectMenuEntry {
  label: string;
  testId: string;
  intent: Intent;
  danger?: boolean;
  /// Entries of one group sit together; a separator is drawn between groups.
  group?: number;
}

const addProject: ProjectMenuEntry = {
  label: "Add project…",
  testId: "menu-add-project",
  intent: { type: "openDialog", dialog: { kind: "addProject" } },
};

/// The entries for a right click on `project`'s row, or on the section when `project` is absent.
/// Grouped: the project's page | adding | archiving | deleting, the one red entry, last.
export function projectMenu(project: Project | undefined): ProjectMenuEntry[] {
  if (!project) return [addProject];
  const archived = Boolean(project.archived);
  return [
    {
      label: "Project settings",
      testId: "menu-project-settings",
      intent: { type: "navigate", route: { name: "settings", project: project.key } },
    },
    { ...addProject, group: 1 },
    archived
      ? {
        label: "Unarchive",
        testId: "menu-unarchive",
        intent: { type: "archiveProject", project: project.key, archived: false },
        group: 2,
      }
      : {
        label: "Archive",
        testId: "menu-archive",
        intent: { type: "archiveProject", project: project.key, archived: true },
        group: 2,
      },
    {
      label: "Delete project…",
      testId: "menu-delete-project",
      intent: { type: "openDialog", dialog: { kind: "deleteProject", project: project.key } },
      danger: true,
      group: 3,
    },
  ];
}

/// The entries split by group, in order, for the menu to draw separators between them.
export function byGroup(entries: ProjectMenuEntry[]): ProjectMenuEntry[][] {
  const groups: ProjectMenuEntry[][] = [];
  for (const e of entries) {
    const last = groups[groups.length - 1];
    if (last && (last[0].group ?? 0) === (e.group ?? 0)) last.push(e);
    else groups.push([e]);
  }
  return groups;
}
