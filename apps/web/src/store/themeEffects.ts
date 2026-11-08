// Runs the theme's effects: the choice goes on <html> (`tokens.css` reads `data-theme`) and is
// remembered in this browser. Storage may be missing or refuse (a private window, blocked site
// data): the theme still applies for this visit.

import { type Theme, parseTheme } from "./theme";

export const THEME_KEY = "toknila.theme";

/// The remembered theme, or `system` when none is stored or it cannot be read.
export function readTheme(): Theme {
  try {
    return parseTheme(window.localStorage.getItem(THEME_KEY));
  } catch {
    return "system";
  }
}

/// Puts the theme on the document: `system` leaves the choice to the browser's colour scheme.
export function applyTheme(theme: Theme): void {
  const root = document.documentElement;
  if (theme === "system") root.removeAttribute("data-theme");
  else root.setAttribute("data-theme", theme);
}

/// Applies the theme and remembers it for this viewer.
export function saveTheme(theme: Theme): void {
  applyTheme(theme);
  try {
    window.localStorage.setItem(THEME_KEY, theme);
  } catch {
    // Not remembered; it still applies now.
  }
}
