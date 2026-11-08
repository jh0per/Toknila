import { describe, expect, it } from "vitest";
import type { Project } from "../api/types";
import { byGroup, projectMenu } from "./projectMenu";

const project = { id: "p1", key: "DEMO", name: "Demo" } as Project;

describe("the projects menu", () => {
  it("offers only adding a project on the section", () => {
    expect(projectMenu(undefined).map((e) => e.testId)).toEqual(["menu-add-project"]);
    expect(projectMenu(undefined)[0].intent).toEqual({ type: "openDialog", dialog: { kind: "addProject" } });
  });

  it("offers a row's settings, adding, archiving and deleting, with only Delete red and last", () => {
    const entries = projectMenu(project);
    expect(entries.map((e) => e.testId)).toEqual([
      "menu-project-settings", "menu-add-project", "menu-archive", "menu-delete-project",
    ]);
    expect(entries[0].intent).toEqual({ type: "navigate", route: { name: "settings", project: "DEMO" } });
    expect(entries[2].intent).toEqual({ type: "archiveProject", project: "DEMO", archived: true });
    expect(entries[3].intent).toEqual({ type: "openDialog", dialog: { kind: "deleteProject", project: "DEMO" } });
    expect(entries.map((e) => Boolean(e.danger))).toEqual([false, false, false, true]);
    expect(byGroup(entries).map((g) => g.length)).toEqual([1, 1, 1, 1]);
  });

  it("offers bringing an archived project back instead", () => {
    const entries = projectMenu({ ...project, archived: true });
    expect(entries.map((e) => e.testId)).toEqual([
      "menu-project-settings", "menu-add-project", "menu-unarchive", "menu-delete-project",
    ]);
    expect(entries[2].intent).toEqual({ type: "archiveProject", project: "DEMO", archived: false });
  });
});
