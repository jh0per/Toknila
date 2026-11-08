// The height of the bottom panel (the card's tabs, the planner's terminal), which the person
// drags. Pure rules plus the one remembered number; the view applies it. The storage may be
// missing (a private window, blocked site data): everything works without it.

/// The divider's own height.
export const DIVIDER_PX = 8;
/// Neither the panel nor the diagram goes below this.
export const MIN_PANEL_PX = 160;
export const MIN_DIAGRAM_PX = 160;
/// How far the arrow keys move the divider.
export const KEY_STEP_PX = 24;
export const STORAGE_KEY = "toknila.panelHeight";

/// The panel height to use: `px` kept between the minimums, given the height `available` for the
/// diagram, the divider and the panel together. A window too small for both minimums keeps the
/// panel's.
export function clampPanel(px: number, available: number): number {
  const hi = Math.max(MIN_PANEL_PX, available - DIVIDER_PX - MIN_DIAGRAM_PX);
  return Math.round(Math.min(Math.max(px, MIN_PANEL_PX), hi));
}

/// The remembered height, or `null` when none was stored or it cannot be read.
export function readStoredHeight(): number | null {
  try {
    const n = Number.parseFloat(window.localStorage.getItem(STORAGE_KEY) ?? "");
    return Number.isFinite(n) && n > 0 ? n : null;
  } catch {
    return null;
  }
}

/// Remembers the height for this viewer; silently does nothing when storage is unavailable.
export function storeHeight(px: number): void {
  try {
    window.localStorage.setItem(STORAGE_KEY, String(Math.round(px)));
  } catch {
    // The height still applies for this visit.
  }
}
