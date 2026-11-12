import type { Face, Measure } from "./cardSize";

// The fonts of a card's text: the same sizes and weights as chain.css (`.node .title`, `.st`,
// `.key`), in the page's own font family.
const SIZE: Record<Face, string> = { title: "600 13px", pill: "600 11px", key: "400 11px" };

let ctx: CanvasRenderingContext2D | null | undefined;
let family = "";
const seen = new Map<string, number>();

function context(): CanvasRenderingContext2D | null {
  if (ctx !== undefined) return ctx;
  try {
    ctx = document.createElement("canvas").getContext("2d");
    family = getComputedStyle(document.body).fontFamily || "system-ui, sans-serif";
  } catch {
    ctx = null;
  }
  return ctx;
}

/// The width of `text` in a card's font, in pixels, from a canvas 2D context (cached). Without a
/// canvas (a test environment) it is a rough guess of 7 px a character.
export const measureText: Measure = (text, face) => {
  const key = `${face}|${text}`;
  const known = seen.get(key);
  if (known !== undefined) return known;
  const c = context();
  let w = text.length * 7;
  if (c) {
    c.font = `${SIZE[face]} ${family}`;
    w = c.measureText(text).width;
  }
  seen.set(key, w);
  return w;
};
