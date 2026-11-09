import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { Project } from "../api/types";
import { CommandsFields } from "../views/ProjectCommands";
import { canAllowAlways, COMMAND_CHOICES, ruleWords } from "./commands";
import { formOf, isDirty, patchOf, sectionOfField } from "./projectForm";

const project = {
  id: "p1", key: "DEMO", name: "Demo", max_parallel: 2, work_branch: "main", release_branch: "main",
  default_profile: "claude-code", default_review_mode: "human", command_policy: "auto",
  allowed_commands: ["Bash(cargo test *)", "Bash(ls *)"],
  merge: { mode: "local", pr_title: "{key}: {title}", method: "merge" },
} as unknown as Project;

describe("the commands setting", () => {
  it("shows the three choices in plain words, auto first", () => {
    expect(COMMAND_CHOICES.map((c) => c.value)).toEqual(["auto", "ask", "trust"]);
    expect(COMMAND_CHOICES.every((c) => c.hint.endsWith(".") && !/permission|hook|bypass/i.test(c.hint))).toBe(true);
  });

  it("is in the form, becomes the patch, and a project without it asks", () => {
    const f = formOf(project);
    expect([f.command_policy, f.allowed_commands]).toEqual(["auto", ["Bash(cargo test *)", "Bash(ls *)"]]);
    expect(formOf({ ...project, command_policy: undefined, allowed_commands: undefined }).command_policy).toBe("ask");
    const patch = patchOf({ ...f, command_policy: "trust", allowed_commands: ["Bash(ls *)"] });
    expect([patch.command_policy, patch.allowed_commands]).toEqual(["trust", ["Bash(ls *)"]]);
    expect(sectionOfField("command_policy")).toBe("agents");
    expect(sectionOfField("allowed_commands")).toBe("agents");
  });

  it("is dirty after a choice or after a rule is removed, and clean again when put back", () => {
    const f = formOf(project);
    expect(isDirty(f, project)).toBe(false);
    expect(isDirty({ ...f, command_policy: "ask" }, project)).toBe(true);
    expect(isDirty({ ...f, allowed_commands: ["Bash(ls *)"] }, project)).toBe(true);
    expect(isDirty({ ...f, allowed_commands: [...f.allowed_commands] }, project)).toBe(false);
  });

  it("renders a radio per choice with the project's one checked, and each remembered rule with Remove", () => {
    const html = renderToStaticMarkup(<CommandsFields form={formOf(project)} set={() => {}} />);
    for (const v of ["auto", "ask", "trust"]) expect(html).toContain(`data-testid="command-policy-${v}"`);
    expect(html).toMatch(/data-testid="command-policy-auto"[^>]*checked/);
    expect(html).not.toMatch(/data-testid="command-policy-ask"[^>]*checked/);
    expect(html).toContain("<code>cargo test *</code>");
    expect(html).toContain('data-testid="forget-rule-1"');
    const none = renderToStaticMarkup(<CommandsFields form={{ ...formOf(project), allowed_commands: [] }} set={() => {}} />);
    expect(none).toContain("allowed-none");
    expect(none).not.toContain("forget-rule-0");
  });
});

describe("the sites the reviewer may read", () => {
  it("is empty by default, one per line in the form, a list in the patch, dirty when edited", () => {
    expect(formOf(project).reviewer_web_domains).toBe("");
    expect(patchOf(formOf(project)).reviewer_web_domains).toEqual([]);
    const withSites = { ...project, reviewer_web_domains: ["docs.rs", "example.com"] } as Project;
    const f = formOf(withSites);
    expect(f.reviewer_web_domains).toBe("docs.rs\nexample.com");
    expect(isDirty(f, withSites)).toBe(false);
    expect(isDirty({ ...f, reviewer_web_domains: " docs.rs,\n\nexample.com " }, withSites)).toBe(false);
    expect(isDirty({ ...f, reviewer_web_domains: "docs.rs" }, withSites)).toBe(true);
    expect(patchOf({ ...f, reviewer_web_domains: "a.com, b.org\nc.net" }).reviewer_web_domains).toEqual(["a.com", "b.org", "c.net"]);
    expect(sectionOfField("reviewer_web_domains")).toBe("review");
  });
});

describe("Allow always", () => {
  const ask: Parameters<typeof canAllowAlways>[0] = {
    role: "worker", answers: ["allow", "deny"], text: "Claude Code asks for permission: Bash: ls -la",
  };
  it("is offered for a worker's command prompt, also when only the full text names the command", () => {
    expect(canAllowAlways(ask)).toBe(true);
    expect(canAllowAlways({ ...ask, text: "Claude Code asks for permission", detail: "Bash: cargo test\n  -p core" })).toBe(true);
  });
  it("is not offered for another tool, a reviewer, or a question without Allow", () => {
    expect(canAllowAlways({ ...ask, text: "Claude Code asks for permission: Write: a.rs" })).toBe(false);
    expect(canAllowAlways({ ...ask, role: "reviewer" })).toBe(false);
    expect(canAllowAlways({ ...ask, answers: ["enter"] })).toBe(false);
  });
  it("shows a remembered rule as the command", () => {
    expect(ruleWords("Bash(cargo test *)")).toBe("cargo test *");
    expect(ruleWords("Read")).toBe("Read");
  });
});
