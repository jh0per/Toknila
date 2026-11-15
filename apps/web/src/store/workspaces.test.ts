import { describe, expect, it } from "vitest";
import type { Workspace, Workspaces } from "../api/types";
import { retiredText, workspaceText, workspacesLine } from "./workspaces";

const list = (count: number, finished: number, items: Workspace[] = []): Workspaces => ({ items, count, finished });

describe("workspacesLine", () => {
  it("counts the workspaces and those of finished stories", () => {
    expect(workspacesLine(list(15, 13))).toBe("Workspaces: 15, of which 13 belong to finished stories");
    expect(workspacesLine(list(0, 0))).toBe("Workspaces: 0, of which 0 belong to finished stories");
  });
  it("says a single finished story in the singular", () => {
    expect(workspacesLine(list(3, 1))).toBe("Workspaces: 3, of which 1 belongs to a finished story");
  });
});

describe("workspaceText", () => {
  it("names a story's workspace by story, status and path, marking a finished one", () => {
    const w: Workspace = {
      kind: "story",
      story: "DEMO-1",
      status: "merged",
      branch: "ship/auth/demo-1-login",
      path: "/s/demo/auth/demo-1-login",
      finished: true,
    };
    expect(workspaceText(w)).toBe("DEMO-1 (merged, finished): /s/demo/auth/demo-1-login");
    expect(workspaceText({ ...w, status: "running", finished: false })).toBe(
      "DEMO-1 (running): /s/demo/auth/demo-1-login",
    );
  });
  it("names the assistant's reading copy as such", () => {
    const w: Workspace = { kind: "planner", path: "/s/demo/_planner", finished: false };
    expect(workspaceText(w)).toBe("The assistant's reading copy (not counted): /s/demo/_planner");
  });
});

describe("retiredText", () => {
  it("says how many workspaces are being cleaned up", () => {
    expect(retiredText(1)).toBe("1 workspace is being cleaned up");
    expect(retiredText(13)).toBe("13 workspaces are being cleaned up");
    expect(retiredText(0)).toBe("Nothing to clean up");
  });
});
