// One ticket's terminal connection: the scrollback first, then the live stream from where the
// scrollback ended, reconnecting by itself. Output goes out through callbacks; typing comes in
// through `send`. No DOM here, so the view can be any terminal emulator.

import { getBytes, post, postBytes, runnerStreamUrl } from "../api/http";
import type { Size } from "./size";
import { type Exit, type Frame, type TermCursor, applyFrame, parseFrame } from "./wire";

export type TermStatus =
  | { kind: "connecting" }
  /// No run has written anything yet.
  | { kind: "none" }
  | { kind: "live" }
  | { kind: "ended"; exit?: Exit }
  | { kind: "offline" };

export interface TerminalSink {
  /// Bytes for the emulator; `reset` clears it first (a new run).
  output(bytes: Uint8Array | null, reset: boolean): void;
  status(s: TermStatus): void;
  /// A refused keystroke (no live runner).
  refused(message: string): void;
}

const FIRST_RETRY_MS = 500;
const MAX_RETRY_MS = 10_000;

export interface TerminalConnection {
  send(bytes: Uint8Array): void;
  /// The size of the panel showing the terminal. A ticket without a live runner refuses; that is
  /// not worth showing, the size is sent again when the stream goes live.
  resize(size: Size): void;
  close(): void;
}

export function openTerminal(key: string, sink: TerminalSink): TerminalConnection {
  const base = `/tickets/${encodeURIComponent(key)}/runner`;
  let cursor: TermCursor = { run: null, end: 0 };
  let ws: WebSocket | null = null;
  let closed = false;
  let retryMs = FIRST_RETRY_MS;
  let timer: number | undefined;
  let typing: Promise<void> = Promise.resolve();

  const take = (frame: Frame) => {
    const first = cursor.run === null;
    const next = applyFrame(cursor, frame);
    cursor = next.cursor;
    if (next.reset || next.write) sink.output(next.write, next.reset);
    if (next.reset || first) sink.status({ kind: "live" });
  };

  const connect = () => {
    if (closed) return;
    const from = cursor.run === null ? null : { run: cursor.run, from: cursor.end };
    ws = new WebSocket(runnerStreamUrl(key, from));
    ws.binaryType = "arraybuffer";
    ws.onopen = () => {
      retryMs = FIRST_RETRY_MS;
      // Until a run has written something there is nothing live to show.
      sink.status(cursor.run === null ? { kind: "none" } : { kind: "live" });
    };
    ws.onmessage = (m) => {
      if (m.data instanceof ArrayBuffer) {
        const frame = parseFrame(m.data);
        if (frame) take(frame);
        return;
      }
      try {
        const control = JSON.parse(String(m.data)) as { type?: string; exit?: Exit };
        if (control.type === "ended") sink.status({ kind: "ended", exit: control.exit });
      } catch {
        /* not a control frame */
      }
    };
    ws.onclose = () => {
      if (closed) return;
      sink.status({ kind: "offline" });
      timer = window.setTimeout(connect, retryMs);
      retryMs = Math.min(retryMs * 2, MAX_RETRY_MS);
    };
  };

  const start = async () => {
    try {
      const scrollback = await getBytes(`${base}/scrollback`);
      if (closed) return;
      const run = scrollback?.headers.get("x-toknila-run");
      const offset = Number(scrollback?.headers.get("x-toknila-offset") ?? 0);
      if (scrollback && run) {
        take({ run, offset, bytes: new Uint8Array(scrollback.bytes) });
      } else {
        sink.status({ kind: "none" });
      }
    } catch {
      /* the stream below shows what is reachable */
    }
    connect();
  };
  void start();

  return {
    send(bytes) {
      // One request at a time, so keystrokes reach the agent in the order they were typed.
      typing = typing
        .then(() => postBytes(`${base}/input`, bytes))
        .catch((e: unknown) => sink.refused(e instanceof Error ? e.message : String(e)));
    },
    resize(size) {
      void post(`${base}/size`, size).catch(() => undefined);
    },
    close() {
      closed = true;
      window.clearTimeout(timer);
      ws?.close();
    },
  };
}
