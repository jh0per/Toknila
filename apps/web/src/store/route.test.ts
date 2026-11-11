import { describe, expect, it } from "vitest";
import { type Route, TABS, isProjectSection, parseRoute, routePath, settingsSection, visibleTabs } from "./route";

describe("the settings routes", () => {
  const cases: [string, Route][] = [
    ["/settings", { name: "settings" }],
    ["/settings/profiles", { name: "settings", section: "profiles" }],
    ["/projects/DEMO/settings", { name: "settings", project: "DEMO" }],
    ["/projects/DEMO/settings/agents", { name: "settings", project: "DEMO", section: "agents" }],
    ["/projects/DEMO/settings/review", { name: "settings", project: "DEMO", section: "review" }],
    ["/projects/DEMO/settings/plans", { name: "settings", project: "DEMO", section: "plans" }],
    ["/projects/DEMO/settings/designs", { name: "settings", project: "DEMO", section: "designs" }],
    ["/projects/DEMO/settings/advanced", { name: "settings", project: "DEMO", section: "advanced" }],
    ["/projects/DEMO/settings/profiles", { name: "settings", project: "DEMO", section: "profiles" }],
    ["/projects/DEMO/settings/updates", { name: "settings", project: "DEMO", section: "updates" }],
    ["/projects/DEMO/settings/about", { name: "settings", project: "DEMO", section: "about" }],
    ["/projects/DEMO/settings/checks", { name: "settings", project: "DEMO", section: "checks" }],
    ["/settings/updates", { name: "settings", section: "updates" }],
    ["/settings/about", { name: "settings", section: "about" }],
    ["/settings/checks", { name: "settings", section: "checks" }],
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

  it("the addresses of earlier versions still open the page they became", () => {
    expect(parseRoute("/projects/DEMO/settings/danger")).toEqual({ name: "settings", project: "DEMO", section: "advanced" });
    expect(parseRoute("/projects/DEMO/settings/app")).toEqual({ name: "settings", project: "DEMO", section: "about" });
    expect(parseRoute("/settings/app")).toEqual({ name: "settings", section: "about" });
  });

  it("the section shown follows the route, and the Projects page opens on General", () => {
    expect(settingsSection({})).toBe("general");
    expect(settingsSection({ project: "DEMO" })).toBe("general");
    expect(settingsSection({ project: "DEMO", section: "agents" })).toBe("agents");
    expect(settingsSection({ section: "profiles" })).toBe("profiles");
    expect(isProjectSection("advanced")).toBe(true);
    expect(isProjectSection("checks")).toBe(false);
  });
});

describe("the card's tabs", () => {
  it("hides Pull request for a project that merges locally, unless the card has one", () => {
    expect(visibleTabs(false, false)).not.toContain("pr");
    expect(visibleTabs(false, true)).toContain("pr");
    expect(visibleTabs(true, false)).toEqual(TABS);
  });
});
