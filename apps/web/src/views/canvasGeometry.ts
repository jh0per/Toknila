// Where things sit on the chain canvas, in pixels of the canvas (not of the window).

export const W = 196;
export const H = 84;
export const GAP_X = 44;
export const GAP_Y = 20;
export const PAD = 16;

export const x = (col: number): number => PAD + col * (W + GAP_X);
export const y = (lane: number): number => PAD + lane * (H + GAP_Y);

export interface Pt {
  x: number;
  y: number;
}

interface Placed {
  col: number;
  lane: number;
}

/// Where a line leaves a node (the middle of its right edge): the node's connector handle.
export const outPoint = (n: Placed): Pt => ({ x: x(n.col) + W, y: y(n.lane) + H / 2 });

/// Where a line enters a node (the middle of its left edge).
export const inPoint = (n: Placed): Pt => ({ x: x(n.col), y: y(n.lane) + H / 2 });

/// The curve from `a` to `b`.
export function curve(a: Pt, b: Pt): string {
  const mid = (a.x + b.x) / 2;
  return `M${a.x},${a.y} C${mid},${a.y} ${mid},${b.y} ${b.x},${b.y}`;
}

export function canvasSize(columns: number, lanes: number): { width: number; height: number } {
  return {
    width: PAD * 2 + columns * W + Math.max(0, columns - 1) * GAP_X,
    height: PAD * 2 + lanes * H + Math.max(0, lanes - 1) * GAP_Y,
  };
}
