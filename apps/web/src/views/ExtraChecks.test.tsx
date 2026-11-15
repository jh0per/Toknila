import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { Project } from "../api/types";
import { formOf } from "../store/projectForm";
import { ExtraChecksField } from "./ExtraChecks";

const project = {
  id: "p1", key: "DEMO", name: "Demo", max_parallel: 2, work_branch: "main", release_branch: "main",
  default_profile: "claude-code", default_review_mode: "human", default_reviewer: null, check_command: "cargo xtask verify",
  extra_checks: [
    { paths: ["apps/web/**"], command: "cargo xtask web && cargo xtask e2e-web" },
    { paths: ["docs/**", "*.md"], command: "make docs" },
  ],
  merge: { mode: "local", pr_title: "{key}: {title}", method: "merge" },
} as unknown as Project;

const render = (p: Project, error?: string) =>
  renderToStaticMarkup(
    <ExtraChecksField
      error={error}
      form={formOf(p)}
      set={() => {}}
    />,
  );

describe("the extra checks rows of the Settings page", () => {
  it("shows one row per check: its paths, its command and a button that removes it", () => {
    const html = render(project);
    expect(html).toContain("Extra checks");
    expect(html).toContain('data-testid="extra-check-paths-0"');
    expect(html).toContain('value="apps/web/**"');
    expect(html).toContain('value="cargo xtask web &amp;&amp; cargo xtask e2e-web"');
    expect(html).toContain('data-testid="extra-check-paths-1"');
    expect(html).toContain('value="docs/**, *.md"');
    expect(html).toContain('data-testid="remove-extra-check-1"');
    expect(html).not.toContain('data-testid="extra-check-paths-2"');
  });

  it("offers adding a check when there is none, and says what a check is for", () => {
    const html = render({ ...project, extra_checks: undefined } as unknown as Project);
    expect(html).toContain('data-testid="add-extra-check"');
    expect(html).not.toContain('data-testid="extra-check-paths-0"');
    expect(html).toContain("after the test command");
  });

  it("shows the daemon's refusal of the list", () => {
    const html = render(project, "an extra check names the paths it is for");
    expect(html).toContain('data-testid="error-extra_checks"');
    expect(html).toContain("an extra check names the paths it is for");
  });
});
