import { describe, expect, it } from "vitest";
import { type Route, parseRoute, routePath, settingsSection } from "./route";

describe("the settings routes", () => {
  const cases: [string, Route][] = [
    ["/settings", { name: "settings" }],
    ["/settings/profiles", { name: "settings", section: "profiles" }],
    ["/projects/DEMO/settings", { name: "settings", project: "DEMO" }],
    ["/projects/DEMO/settings/agents", { name: "settings", project: "DEMO", section: "agents" }],
    ["/projects/DEMO/settings/review", { name: "settings", project: "DEMO", section: "review" }],
    ["/projects/DEMO/settings/plans", { name: "settings", project: "DEMO", section: "plans" }],
    ["/projects/DEMO/settings/danger", { name: "settings", project: "DEMO", section: "danger" }],
    ["/projects/DEMO/settings/app", { name: "settings", project: "DEMO", section: "app" }],
    ["/projects/DEMO/settings/profiles", { name: "settings", project: "DEMO", section: "profiles" }],
  ];

  it.each(cases)("%s is a section of its own and round-trips", (path, route) => {
    expect(parseRoute(path)).toEqual(route);
    expect(routePath(route)).toBe(path);
  });

  it("General is the bare project address, and an unknown section falls back to it", () => {
    expect(routePath({ name: "settings", project: "DEMO", section: "general" })).toBe("/projects/DEMO/settings");
    expect(parseRoute("/projects/DEMO/settings/nonsense")).toEqual({ name: "settings", project: "DEMO" });
    expect(parseRoute("/settings/nonsense")).toEqual({ name: "chains" });
  });

  it("the section shown follows the route", () => {
    expect(settingsSection({})).toBe("app");
    expect(settingsSection({ project: "DEMO" })).toBe("general");
    expect(settingsSection({ project: "DEMO", section: "agents" })).toBe("agents");
    expect(settingsSection({ section: "profiles" })).toBe("profiles");
    // A project's section without a project is the app's.
    expect(settingsSection({ section: "agents" })).toBe("app");
  });
});
