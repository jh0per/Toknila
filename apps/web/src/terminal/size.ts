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
