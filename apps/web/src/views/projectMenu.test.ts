import { describe, expect, it } from "vitest";
import type { Project } from "../api/types";
import { projectMenu } from "./projectMenu";

const project = { id: "p1", key: "DEMO", name: "Demo" } as Project;

describe("the projects menu", () => {
  it("offers only adding a project on the section", () => {
    expect(projectMenu(undefined).map((e) => e.testId)).toEqual(["menu-add-project"]);
    expect(projectMenu(undefined)[0].intent).toEqual({ type: "openDialog", dialog: { kind: "addProject" } });
  });

  it("offers a row's settings and archiving, then adding a project", () => {
    const entries = projectMenu(project);
    expect(entries.map((e) => e.testId)).toEqual([
      "menu-project-settings", "menu-archive", "menu-delete-project", "menu-add-project",
    ]);
    expect(entries[0].intent).toEqual({ type: "navigate", route: { name: "settings", project: "DEMO" } });
    expect(entries[1].intent).toEqual({ type: "archiveProject", project: "DEMO", archived: true });
    expect(entries[2].intent).toEqual({ type: "openDialog", dialog: { kind: "deleteProject", project: "DEMO" } });
  });

  it("offers bringing an archived project back instead", () => {
    const entries = projectMenu({ ...project, archived: true });
    expect(entries.map((e) => e.testId)).toEqual([
      "menu-project-settings", "menu-unarchive", "menu-delete-project", "menu-add-project",
    ]);
    expect(entries[1].intent).toEqual({ type: "archiveProject", project: "DEMO", archived: false });
  });
});
