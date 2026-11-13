import { describe, expect, it } from "vitest";
import type { Project } from "../api/types";
import { byGroup, projectMenu } from "./projectMenu";

const project = { id: "p1", key: "DEMO", name: "Demo" } as Project;

describe("the projects menu", () => {
  it("offers only adding a project on the section", () => {
    expect(projectMenu(undefined).map((e) => e.testId)).toEqual(["menu-add-project"]);
    expect(projectMenu(undefined)[0].intent).toEqual({ type: "openDialog", dialog: { kind: "addProject" } });
  });

  it("offers a row's settings, adding, hiding, archiving and deleting, with only Delete red and last", () => {
    const entries = projectMenu(project);
    expect(entries.map((e) => e.testId)).toEqual([
      "menu-project-settings", "menu-add-project", "menu-hide-project", "menu-archive", "menu-delete-project",
    ]);
    expect(entries[0].intent).toEqual({ type: "navigate", route: { name: "settings", project: "DEMO" } });
    expect(entries[2].label).toBe("Hide from list");
    expect(entries[2].intent).toEqual({ type: "hideProject", project: "DEMO", hidden: true });
    expect(entries[3].intent).toEqual({ type: "archiveProject", project: "DEMO", archived: true });
    expect(entries[4].intent).toEqual({ type: "openDialog", dialog: { kind: "deleteProject", project: "DEMO" } });
    expect(entries.map((e) => Boolean(e.danger))).toEqual([false, false, false, false, true]);
    // Hide sits with Archive, above it.
    expect(byGroup(entries).map((g) => g.map((e) => e.testId))).toEqual([
      ["menu-project-settings"], ["menu-add-project"], ["menu-hide-project", "menu-archive"], ["menu-delete-project"],
    ]);
  });

  it("offers bringing a hidden project back to the list in Hide's place, not red", () => {
    const entries = projectMenu({ ...project, hidden: true });
    expect(entries.map((e) => e.testId)).toEqual([
      "menu-project-settings", "menu-add-project", "menu-show-project", "menu-archive", "menu-delete-project",
    ]);
    expect(entries[2].label).toBe("Show in list");
    expect(entries[2].intent).toEqual({ type: "hideProject", project: "DEMO", hidden: false });
    expect(entries.map((e) => Boolean(e.danger))).toEqual([false, false, false, false, true]);
    expect(byGroup(entries).map((g) => g.length)).toEqual([1, 1, 2, 1]);
  });

  it("offers bringing an archived project back instead, hidden or not", () => {
    for (const p of [{ ...project, archived: true }, { ...project, archived: true, hidden: true }]) {
      const entries = projectMenu(p);
      expect(entries.map((e) => e.testId)).toEqual([
        "menu-project-settings", "menu-add-project", "menu-unarchive", "menu-delete-project",
      ]);
      expect(entries[2].intent).toEqual({ type: "archiveProject", project: "DEMO", archived: false });
    }
  });
});
