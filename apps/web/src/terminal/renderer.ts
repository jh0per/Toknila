// Which renderer draws the terminal. The canvas (WebGL) renderer repaints a full-screen TUI's
// redraws without the DOM churn that makes the DOM renderer flicker; the DOM renderer stays the
// fallback (no WebGL, or the context is lost) and the one automated browsers use, because their
// tests read the rows' text.

export type Renderer = "webgl" | "dom";

/// The renderer to start with. `stored` is the viewer's override ("webgl" or "dom"); without one,
/// an automated browser (`webdriver`) gets the DOM and everyone else the canvas.
export function chooseRenderer(webdriver: boolean, stored: string | null): Renderer {
  if (stored === "webgl" || stored === "dom") return stored;
  return webdriver ? "dom" : "webgl";
}

export const RENDERER_KEY = "toknila.terminal.renderer";

/// The renderer for this browser, reading the override defensively (storage can be blocked).
export function currentRenderer(): Renderer {
  let stored: string | null = null;
  try {
    stored = localStorage.getItem(RENDERER_KEY);
  } catch {
    /* blocked storage: no override */
  }
  return chooseRenderer(typeof navigator !== "undefined" && navigator.webdriver === true, stored);
}
