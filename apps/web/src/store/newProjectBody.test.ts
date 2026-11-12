import { describe, expect, it } from "vitest";
import { newProjectBody } from "./addForm";

describe("adding a project from the window", () => {
  it("names the real agent explicitly and never the simulated one", () => {
    const body = newProjectBody({
      key: "DEMO", name: "", repo_path: "/work/demo", work_branch: "", check_command: "", setup_command: "", merge_mode: "local",
    });
    expect(body.default_profile).toBe("claude-code");
    expect(body).toMatchObject({ key: "DEMO", repo_path: "/work/demo", merge_mode: "local" });
    expect(JSON.stringify(body)).not.toContain("sim");
  });
});
