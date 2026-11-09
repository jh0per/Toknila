// The pure rules of zooming the diagram, and the one remembered number per chain. The hook
// (`useFit`) applies them; the storage may be missing (a private window, blocked site data) and
// everything works without it.

/// The diagram is never drawn smaller than this, so cards stay readable.
export const MIN_ZOOM = 0.4;
/// ...nor larger than this.
export const MAX_ZOOM = 2;
/// The sizes the keys and the buttons step through.
export const ZOOM_STEPS = [0.4, 0.5, 0.67, 0.8, 0.9, 1, 1.1, 1.25, 1.5, 1.75, 2];

/// `z` kept between the bounds; anything that is not a number is 100%.
export function clampZoom(z: number): number {
  if (!Number.isFinite(z)) return 1;
  return Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, z));
}

/// The next size up (`dir` 1) or down (-1) from `z`; `z` itself when there is none.
export function stepZoom(z: number, dir: 1 | -1): number {
  const eps = 0.005;
  if (dir === 1) return ZOOM_STEPS.find((s) => s > z + eps) ?? clampZoom(z);
  return [...ZOOM_STEPS].reverse().find((s) => s < z - eps) ?? clampZoom(z);
}

/// The zoom after a pinch of `deltaY` (a ctrl+wheel event; pinching out gives a negative delta
/// that grows the diagram). Exponential, so equal pinches give equal ratios.
export function pinchZoom(z: number, deltaY: number): number {
  return clampZoom(z * Math.exp(-deltaY * 0.01));
}

export type Scroll = { left: number; top: number };

/// The scroll offsets that keep the diagram point under the pointer where it is while the zoom
/// goes from `from` to `to`. `pointer` is measured from the scroller's top left corner. The offsets
/// are not clamped: the browser clamps them to the size the scroller has after the change.
export function zoomAbout(scroll: Scroll, pointer: { x: number; y: number }, from: number, to: number): Scroll {
  const ratio = to / from;
  return { left: (scroll.left + pointer.x) * ratio - pointer.x, top: (scroll.top + pointer.y) * ratio - pointer.y };
}

const KEY = "toknila.zoom.";

/// The zoom remembered for a chain, or `null` when none was stored or it cannot be read.
export function readStoredZoom(chain: string): number | null {
  try {
    const n = Number.parseFloat(window.localStorage.getItem(KEY + chain) ?? "");
    return Number.isFinite(n) ? clampZoom(n) : null;
  } catch {
    return null;
  }
}

/// Remembers the zoom of a chain for this viewer; silently does nothing without storage.
export function storeZoom(chain: string, z: number): void {
  try {
    window.localStorage.setItem(KEY + chain, String(Math.round(clampZoom(z) * 1000) / 1000));
  } catch {
    // The zoom still applies for this visit.
  }
}
