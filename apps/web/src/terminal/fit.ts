// Fits an xterm.js grid to the element that hosts it. The grid is counted from the host's content
// box (padding off) and the size of one cell as drawn, rounding down, so the last row and the last
// column are never cut by the host's edge. (The fit addon reads the host's border-box height, which
// includes its padding, and offers one row too many.)

import type { Terminal } from "@xterm/xterm";
import { type Size, fitGrid } from "./size";

const px = (v: string): number => Number.parseFloat(v) || 0;

/// The grid that fits `host` for `term` now, or `null` while it cannot be measured (hidden, or
/// the first frame is not drawn yet).
export function measureGrid(host: HTMLElement, term: Terminal): Size | null {
  const screen = host.querySelector<HTMLElement>(".xterm-screen");
  if (!screen || term.cols < 1 || term.rows < 1) return null;
  const style = getComputedStyle(host);
  const box = {
    width: host.clientWidth - px(style.paddingLeft) - px(style.paddingRight),
    height: host.clientHeight - px(style.paddingTop) - px(style.paddingBottom),
  };
  const drawn = screen.getBoundingClientRect();
  const cell = { width: drawn.width / term.cols, height: drawn.height / term.rows };
  const viewport = host.querySelector<HTMLElement>(".xterm-viewport");
  const scrollbar = viewport ? viewport.offsetWidth - viewport.clientWidth : 0;
  return fitGrid(box, cell, scrollbar);
}

/// Resizes `term` to the grid that fits `host`; returns whether it changed.
export function fitTerminal(host: HTMLElement, term: Terminal): boolean {
  const grid = measureGrid(host, term);
  if (!grid || (grid.cols === term.cols && grid.rows === term.rows)) return false;
  term.resize(grid.cols, grid.rows);
  return true;
}
