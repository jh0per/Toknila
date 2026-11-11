// Navigation is state that lives in the URL, so a deep link opens the right screen.
// `chains` (`/`) and `chain` (`/chains/:chain`) are the paths without a project; they render inside
// the same layout, with the project taken from the chain.

export type Route =
  | { name: "chains" }
  | { name: "chain"; chain: string; view?: ChainView }
  | { name: "inbox" }
  | { name: "usage" }
  | { name: "settings"; project?: string; section?: SettingsSection }
  | { name: "project"; project: string }
  | { name: "planner"; project: string }
  | { name: "projectChain"; project: string; chain: string; view?: ChainView }
  | { name: "card"; project: string; chain: string; key: string; view?: ChainView };

/// What a chain shows next to its panel: the diagram (no `view`) or the board of its designs.
export type ChainView = "designs";

/// The view a chain route shows: `undefined` is the diagram.
export function viewOf(route: Route): ChainView | undefined {
  return route.name === "chain" || route.name === "projectChain" || route.name === "card" ? route.view : undefined;
}

/// The same chain (or card) route with another view; any other route is returned as it is.
export function withView(route: Route, view: ChainView | undefined): Route {
  if (route.name !== "chain" && route.name !== "projectChain" && route.name !== "card") return route;
  const { view: _old, ...rest } = route;
  return (view ? { ...rest, view } : rest) as Route;
}

/// The sections of a project's settings, tabs of the Projects page.
export type ProjectSection = "general" | "agents" | "review" | "plans" | "designs" | "advanced";

/// The pages of the settings list that are not about one project.
export type AppSection = "profiles" | "updates" | "about" | "checks";

/// The sections of the settings screen: the flat list (Projects, Agents, Updates, About, System
/// checks) and, under Projects, the focused project's sections.
export type SettingsSection = AppSection | ProjectSection;

export const PROJECT_SECTIONS: ProjectSection[] = ["general", "agents", "review", "plans", "designs", "advanced"];
export const APP_SECTIONS: AppSection[] = ["profiles", "updates", "about", "checks"];
const ALL_SECTIONS: SettingsSection[] = [...APP_SECTIONS, ...PROJECT_SECTIONS];

/// Addresses of earlier versions, kept working: the danger zone moved under Advanced, the App
/// page split into About and System checks.
const OLD_SECTIONS: Record<string, SettingsSection> = { danger: "advanced", app: "about" };

const sectionOf = (name: string | undefined): SettingsSection | undefined =>
  ALL_SECTIONS.find((s) => s === name) ?? (name === undefined ? undefined : OLD_SECTIONS[name]);

/// Whether a section belongs to the Projects page of the settings list.
export const isProjectSection = (s: SettingsSection): s is ProjectSection =>
  (PROJECT_SECTIONS as SettingsSection[]).includes(s);

/// The section a settings route shows; without one the Projects page opens on General.
export function settingsSection(route: { project?: string; section?: SettingsSection }): SettingsSection {
  return route.section ?? "general";
}

/// The bottom panel's tab; part of the page state, not of the URL.
export type Tab = "details" | "events" | "chat" | "terminal" | "threads" | "pr";

export const TABS: Tab[] = ["details", "events", "chat", "terminal", "threads", "pr"];

/// The tabs a card shows: the pull request only for a project that opens pull requests (or a card
/// that already has one), since with local merges there is none.
export function visibleTabs(opensPullRequests: boolean, hasPullRequest: boolean): Tab[] {
  return TABS.filter((t) => t !== "pr" || opensPullRequests || hasPullRequest);
}

/// The route of a path; anything unknown is the home page.
export function parseRoute(pathname: string): Route {
  const p = pathname.split("/").filter(Boolean).map(decodeURIComponent);
  if (p[0] === "chains" && p.length === 2) return { name: "chain", chain: p[1] };
  if (p[0] === "chains" && p.length === 3 && p[2] === "designs") return { name: "chain", chain: p[1], view: "designs" };
  if (p[0] === "inbox" && p.length === 1) return { name: "inbox" };
  if (p[0] === "usage" && p.length === 1) return { name: "usage" };
  if (p[0] === "settings" && p.length === 1) return { name: "settings" };
  if (p[0] === "settings" && p.length === 2) {
    const section = sectionOf(p[1]);
    if (section && section !== "general") return { name: "settings", section };
  }
  if (p[0] === "projects" && p[2] === "settings" && (p.length === 3 || p.length === 4)) {
    const section = sectionOf(p[3]);
    return section && section !== "general" ? { name: "settings", project: p[1], section } : { name: "settings", project: p[1] };
  }
  if (p[0] === "projects") {
    if (p.length === 2) return { name: "project", project: p[1] };
    if (p.length === 3 && p[2] === "planner") return { name: "planner", project: p[1] };
    if (p.length === 4 && p[2] === "chains") return { name: "projectChain", project: p[1], chain: p[3] };
    if (p.length === 5 && p[2] === "chains" && p[4] === "designs") {
      return { name: "projectChain", project: p[1], chain: p[3], view: "designs" };
    }
    if (p.length === 6 && p[2] === "chains" && p[4] === "cards") {
      return { name: "card", project: p[1], chain: p[3], key: p[5] };
    }
    if (p.length === 7 && p[2] === "chains" && p[4] === "cards" && p[6] === "designs") {
      return { name: "card", project: p[1], chain: p[3], key: p[5], view: "designs" };
    }
  }
  return { name: "chains" };
}

export function routePath(route: Route): string {
  const e = encodeURIComponent;
  switch (route.name) {
    case "chains": return "/";
    case "chain": return `/chains/${e(route.chain)}${tail(route.view)}`;
    case "inbox": return "/inbox";
    case "usage": return "/usage";
    case "settings": return settingsPath(route, e);
    case "project": return `/projects/${e(route.project)}`;
    case "planner": return `/projects/${e(route.project)}/planner`;
    case "projectChain": return `/projects/${e(route.project)}/chains/${e(route.chain)}${tail(route.view)}`;
    case "card": return `/projects/${e(route.project)}/chains/${e(route.chain)}/cards/${e(route.key)}${tail(route.view)}`;
  }
}

const tail = (view: ChainView | undefined): string => (view ? `/${view}` : "");

function settingsPath(route: Extract<Route, { name: "settings" }>, e: (s: string) => string): string {
  const section = settingsSection(route);
  if (route.project) {
    return section === "general" ? `/projects/${e(route.project)}/settings` : `/projects/${e(route.project)}/settings/${section}`;
  }
  return section === "general" ? "/settings" : `/settings/${section}`;
}
