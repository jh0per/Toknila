// Navigation is state that lives in the URL, so a deep link opens the right screen.
// `chains` (`/`) and `chain` (`/chains/:chain`) are the phase 0 paths; they still render, inside
// the same layout, with the project taken from the chain.

export type Route =
  | { name: "chains" }
  | { name: "chain"; chain: string }
  | { name: "inbox" }
  | { name: "usage" }
  | { name: "settings"; project?: string }
  | { name: "project"; project: string }
  | { name: "planner"; project: string }
  | { name: "projectChain"; project: string; chain: string }
  | { name: "card"; project: string; chain: string; key: string };

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
  if (p[0] === "projects" && p.length === 3 && p[2] === "settings") return { name: "settings", project: p[1] };
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
    case "settings": return route.project ? `/projects/${e(route.project)}/settings` : "/settings";
    case "project": return `/projects/${e(route.project)}`;
    case "planner": return `/projects/${e(route.project)}/planner`;
    case "projectChain": return `/projects/${e(route.project)}/chains/${e(route.chain)}`;
    case "card": return `/projects/${e(route.project)}/chains/${e(route.chain)}/cards/${e(route.key)}`;
  }
}
