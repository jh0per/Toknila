// Where "Back to work" goes: the page the person left for the settings. Settings pages move among
// themselves without changing it.

import type { Route } from "./route";

/// The page to go back to after a move from `prev` to `next`, given the one remembered so far.
export function nextSettingsFrom(prev: Route, next: Route, from: Route | undefined): Route | undefined {
  if (next.name !== "settings") return from;
  return prev.name === "settings" ? from : prev;
}

/// The page "Back to work" opens: the remembered one, else the home page.
export function backRoute(from: Route | undefined): Route {
  return from && from.name !== "settings" ? from : { name: "chains" };
}
