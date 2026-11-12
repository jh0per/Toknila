// A selection that survives a resize of the grid when it can. xterm clears the selection when the
// grid changes; the lines keep their absolute index when only the rows change, so the same span
// can be selected again. A change of the columns re-wraps the lines, so then the selection goes.

import type { Terminal } from "@xterm/xterm";

interface Pos { x: number; y: number }
export interface Span { column: number; row: number; length: number }

/// The arguments of `Terminal.select` for a selection from `start` to `end` (absolute rows) on a
/// grid `cols` wide.
export function spanOf(start: Pos, end: Pos, cols: number): Span {
  return { column: start.x, row: start.y, length: (end.y - start.y) * cols + end.x - start.x };
}

/// Runs `resize`, and selects the same span again when the columns did not change and the
/// selection is gone. Returns whatever `resize` returns.
export function keepingSelection<T>(term: Terminal, resize: () => T): T {
  const start = term.hasSelection() ? term.getSelectionPosition() : undefined;
  const cols = term.cols;
  const out = resize();
  if (start && term.cols === cols && !term.hasSelection()) {
    const s = spanOf({ x: start.start.x, y: start.start.y }, { x: start.end.x, y: start.end.y }, cols);
    if (s.length > 0) term.select(s.column, s.row, s.length);
  }
  return out;
}
