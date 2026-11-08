// Navigation is state that lives in the URL, so a deep link opens the right screen.
// `chains` (`/`) and `chain` (`/chains/:chain`) are the paths without a project; they render inside
// the same layout, with the project taken from the chain.

export type Route =
  | { name: "chains" }
  | { name: "chain"; chain: string }
  | { name: "inbox" }
  | { name: "usage" }
  | { name: "settings"; project?: string; section?: SettingsSection }
  | { name: "project"; project: string }
  | { name: "planner"; project: string }
  | { name: "projectChain"; project: string; chain: string }
  | { name: "card"; project: string; chain: string; key: string };

/// The sections of the settings screen: the app's (`app`, `profiles`) and the focused project's.
export type SettingsSection = "app" | "profiles" | "general" | "agents" | "review" | "plans" | "danger";

export const PROJECT_SECTIONS: SettingsSection[] = ["general", "agents", "review", "plans", "danger"];
const ALL_SECTIONS: SettingsSection[] = ["app", "profiles", ...PROJECT_SECTIONS];

/// The section a settings route shows. App and Profiles are the app's, whether or not a project is
/// in focus; the project's sections need one. Without a section: General with a project, else App.
export function settingsSection(route: { project?: string; section?: SettingsSection }): SettingsSection {
  const { project, section } = route;
  if (section === "app" || section === "profiles") return section;
  if (!project) return "app";
  return section && PROJECT_SECTIONS.includes(section) ? section : "general";
}

/// The bottom panel's tab; part of the page state, not of the URL.
export type Tab = "details" | "events" | "chat" | "terminal" | "threads" | "pr";

export const TABS: Tab[] = ["details", "events", "chat", "terminal", "threads", "pr"];

/// The route of a path; anything unknown is the home page.
export function parseRoute(pathname: string): Route {
  const p = pathname.split("/").filter(Boolean).map(decodeURIComponent);
  if (p[0] === "chains" && p.length === 2) return { name: "chain", chain: p[1] };
  if (p[0] === "inbox" && p.length === 1) return { name: "inbox" };
  if (p[0] === "usage" && p.length === 1) return { name: "usage" };
  if (p[0] === "settings" && p.length === 1) return { name: "settings" };
  if (p[0] === "settings" && p.length === 2 && p[1] === "profiles") return { name: "settings", section: "profiles" };
  if (p[0] === "projects" && p[2] === "settings" && (p.length === 3 || p.length === 4)) {
    const section = ALL_SECTIONS.find((s) => s === p[3] && s !== "general");
    return section ? { name: "settings", project: p[1], section } : { name: "settings", project: p[1] };
  }
  if (p[0] === "projects") {
    if (p.length === 2) return { name: "project", project: p[1] };
    if (p.length === 3 && p[2] === "planner") return { name: "planner", project: p[1] };
    if (p.length === 4 && p[2] === "chains") return { name: "projectChain", project: p[1], chain: p[3] };
    if (p.length === 6 && p[2] === "chains" && p[4] === "cards") {
      return { name: "card", project: p[1], chain: p[3], key: p[5] };
    }
  }
  return { name: "chains" };
}

export function routePath(route: Route): string {
  const e = encodeURIComponent;
  switch (route.name) {
    case "chains": return "/";
    case "chain": return `/chains/${e(route.chain)}`;
    case "inbox": return "/inbox";
    case "usage": return "/usage";
    case "settings": return settingsPath(route, e);
    case "project": return `/projects/${e(route.project)}`;
    case "planner": return `/projects/${e(route.project)}/planner`;
    case "projectChain": return `/projects/${e(route.project)}/chains/${e(route.chain)}`;
    case "card": return `/projects/${e(route.project)}/chains/${e(route.chain)}/cards/${e(route.key)}`;
  }
}

function settingsPath(route: Extract<Route, { name: "settings" }>, e: (s: string) => string): string {
  const section = settingsSection(route);
  if (route.project) {
    return section === "general" ? `/projects/${e(route.project)}/settings` : `/projects/${e(route.project)}/settings/${section}`;
  }
  return section === "profiles" ? "/settings/profiles" : "/settings";
}
