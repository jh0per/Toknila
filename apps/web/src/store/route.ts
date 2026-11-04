// Navigation is state that lives in the URL, so a deep link opens the right screen.

export type Route =
  | { name: "chains" }
  | { name: "chain"; chain: string }
  | { name: "inbox" };

/// The route of a path; anything unknown is the chain list.
export function parseRoute(pathname: string): Route {
  const parts = pathname.split("/").filter(Boolean).map(decodeURIComponent);
  if (parts[0] === "chains" && parts.length === 2) return { name: "chain", chain: parts[1] };
  if (parts[0] === "inbox" && parts.length === 1) return { name: "inbox" };
  return { name: "chains" };
}

export function routePath(route: Route): string {
  switch (route.name) {
    case "chains": return "/";
    case "chain": return `/chains/${encodeURIComponent(route.chain)}`;
    case "inbox": return "/inbox";
  }
}
