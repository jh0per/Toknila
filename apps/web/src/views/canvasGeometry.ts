// Where things sit on the chain canvas, in pixels of the canvas (not of the window).

export const W = 196;
export const H = 92;
export const GAP_X = 44;
export const GAP_Y = 20;
export const PAD = 16;

export const x = (col: number): number => PAD + col * (W + GAP_X);
export const y = (lane: number): number => PAD + lane * (H + GAP_Y);

export interface Pt {
  x: number;
  y: number;
}

/// The grid a dragged card snaps to, in canvas pixels.
export const GRID = 8;

const snapTo = (v: number): number => Math.max(PAD, Math.round(v / GRID) * GRID);

/// A card's top-left corner as dropped: on the grid, never outside the canvas's padding.
export const snapPos = (p: Pt): Pt => ({ x: snapTo(p.x), y: snapTo(p.y) });

/// Where a node sits: the top-left corner of its box, in canvas pixels.
export type Box = Pt;

/// Where a line leaves a node (the middle of its right edge): the node's connector handle.
export const outPoint = (n: Box): Pt => ({ x: n.x + W, y: n.y + H / 2 });

/// Where a line enters a node (the middle of its left edge).
export const inPoint = (n: Box): Pt => ({ x: n.x, y: n.y + H / 2 });

/// The curve from `a` to `b`, turning at `mid` (the middle by default).
export function curve(a: Pt, b: Pt, mid = (a.x + b.x) / 2): string {
  return `M${a.x},${a.y} C${mid},${a.y} ${mid},${b.y} ${b.x},${b.y}`;
}

/// The size of the canvas that holds every box, with the padding on every side.
export function canvasSize(boxes: Iterable<Box>): { width: number; height: number } {
  let width = PAD;
  let height = PAD;
  for (const b of boxes) {
    width = Math.max(width, b.x + W);
    height = Math.max(height, b.y + H);
  }
  return { width: width + PAD, height: height + PAD };
}

/// How one line runs: where it leaves, where it enters and where its curve turns.
export interface EdgeGeo {
  from: Pt;
  to: Pt;
  turn: number;
}

/// The key of a line in the map `edgeGeometry` returns.
export const edgeId = (e: { from: string; to: string }): string => `${e.from}>${e.to}`;

/// Space between neighbouring lines on one side of a card, and between their turns.
const SPREAD_Y = 9;
const SPREAD_X = 7;
const MARGIN_Y = 14;
/// A curve turns at least this far from either card.
const TURN_MARGIN = 6;

/// Offset of line `i` of `n` from the middle, evenly spread, inside the card's height.
const fan = (i: number, n: number, step: number, room: number): number =>
  n < 2 ? 0 : (i - (n - 1) / 2) * Math.min(step, room / (n - 1));

/// The path of every line, so many lines at one card do not share one trunk: those leaving a card
/// leave at separate points of its right edge, in the order of their targets from top to bottom,
/// those entering a card enter at separate points of its left edge, and the lines of one card turn
/// at slightly different places. A card with one line on a side keeps it at the middle.
export function edgeGeometry(
  edges: { from: string; to: string }[],
  at: Map<string, Box>,
): Map<string, EdgeGeo> {
  const byY = (a: string, b: string) => (at.get(a)?.y ?? 0) - (at.get(b)?.y ?? 0) || (at.get(a)?.x ?? 0) - (at.get(b)?.x ?? 0);
  const out = new Map<string, string[]>();
  const into = new Map<string, string[]>();
  for (const e of edges) {
    out.set(e.from, [...(out.get(e.from) ?? []), e.to]);
    into.set(e.to, [...(into.get(e.to) ?? []), e.from]);
  }
  for (const list of [...out.values(), ...into.values()]) list.sort(byY);
  const geo = new Map<string, EdgeGeo>();
  const room = H - 2 * MARGIN_Y;
  for (const e of edges) {
    const a = at.get(e.from);
    const b = at.get(e.to);
    if (!a || !b) continue;
    const outs = out.get(e.from) ?? [];
    const ins = into.get(e.to) ?? [];
    const o = outPoint(a);
    const i = inPoint(b);
    const from = { x: o.x, y: o.y + fan(outs.indexOf(e.to), outs.length, SPREAD_Y, room) };
    const to = { x: i.x, y: i.y + fan(ins.indexOf(e.from), ins.length, SPREAD_Y, room) };
    const middle = (from.x + to.x) / 2 + fan(outs.indexOf(e.to), outs.length, SPREAD_X, Math.max(0, to.x - from.x - 2 * TURN_MARGIN));
    geo.set(edgeId(e), { from, to, turn: Math.max(from.x + TURN_MARGIN, Math.min(to.x - TURN_MARGIN, middle)) });
  }
  return geo;
}
