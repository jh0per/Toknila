// Tells the daemon the size of the panel that shows a terminal, so the agent (a full-screen
// terminal program) draws for the same grid the person sees. Pure: no DOM, no network.

export interface Size {
  cols: number;
  rows: number;
}

/// The size is one a terminal can have; a panel that is hidden or still laying out reports 0.
export function usable(s: Size): boolean {
  return Number.isInteger(s.cols) && Number.isInteger(s.rows) && s.cols >= 2 && s.rows >= 1;
}

export interface Px {
  width: number;
  height: number;
}

/// The grid that fits entirely in `box` (the host's content box, padding already taken off) when
/// a cell is `cell` pixels and a scrollbar takes `scrollbar` pixels of width. Always rounds down,
/// so no row or column is ever cut by the edge. `null` while the box or the cell is not measured
/// yet (hidden, laying out, font not loaded). A box smaller than one cell still gets the smallest
/// grid a terminal can have.
export function fitGrid(box: Px, cell: Px, scrollbar = 0): Size | null {
  const ok = (n: number) => Number.isFinite(n) && n > 0;
  if (!ok(box.width) || !ok(box.height) || !ok(cell.width) || !ok(cell.height)) return null;
  const cols = Math.floor((box.width - Math.max(0, scrollbar)) / cell.width);
  const rows = Math.floor(box.height / cell.height);
  return { cols: Math.max(2, cols), rows: Math.max(1, rows) };
}

export interface SizeReporter {
  /// The panel is now this size. Sent after the panel has been still for the delay, and only when
  /// it differs from what the daemon was last told.
  report(s: Size): void;
  /// The daemon may have lost the size (a new run, a reconnect): send the latest again, now.
  resend(): void;
  dispose(): void;
}

export function sizeReporter(send: (s: Size) => void, delayMs = 150): SizeReporter {
  let wanted: Size | null = null;
  let sent: Size | null = null;
  let timer: ReturnType<typeof setTimeout> | undefined;

  const flush = (force: boolean) => {
    clearTimeout(timer);
    timer = undefined;
    if (!wanted) return;
    if (!force && sent && sent.cols === wanted.cols && sent.rows === wanted.rows) return;
    sent = wanted;
    send(wanted);
  };

  return {
    report(s) {
      if (!usable(s)) return;
      wanted = { cols: s.cols, rows: s.rows };
      clearTimeout(timer);
      timer = setTimeout(() => flush(false), delayMs);
    },
    resend() {
      flush(true);
    },
    dispose() {
      clearTimeout(timer);
      timer = undefined;
      wanted = null;
    },
  };
}
