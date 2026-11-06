// The runner stream's wire format and the rule that keeps the view free of duplicates.
// Pure: the formats are the daemon's (`toknila_api_types::runner`), nothing here touches the DOM.

/// Characters of a run id in a frame header, then the 8-byte big-endian offset of the payload.
const ID_LEN = 26;
const HEADER = ID_LEN + 8;

/// An output frame, decoded.
export interface Frame {
  run: string;
  /// Offset of the first payload byte in the run's output.
  offset: number;
  bytes: Uint8Array;
}

/// What the view already shows: the run, and the offset after its last byte.
export interface TermCursor {
  run: string | null;
  end: number;
}

/// How a run's process ended, as the `ended` control frame carries it.
export interface Exit {
  code?: number | null;
  signal?: string | null;
}

/// Reads a binary message; `null` if it is not an output frame.
export function parseFrame(data: ArrayBuffer): Frame | null {
  if (data.byteLength < HEADER) return null;
  const run = new TextDecoder().decode(new Uint8Array(data, 0, ID_LEN));
  const offset = Number(new DataView(data).getBigUint64(ID_LEN));
  return { run, offset, bytes: new Uint8Array(data, HEADER) };
}

/// What to do with a frame: clear the view first (a new run), write these bytes (none if the view
/// already has them all), and the cursor after it.
export interface Applied {
  reset: boolean;
  write: Uint8Array | null;
  cursor: TermCursor;
}

/// Deduplicates by offset: the scrollback and the stream overlap, and a reconnect resends.
export function applyFrame(have: TermCursor, f: Frame): Applied {
  const stop = f.offset + f.bytes.length;
  if (have.run !== null && have.run !== f.run) {
    return { reset: true, write: f.bytes, cursor: { run: f.run, end: stop } };
  }
  const run = have.run ?? f.run;
  if (stop <= have.end) return { reset: false, write: null, cursor: have };
  // A frame that starts beyond what we have (the log was trimmed) is written whole.
  const skip = f.offset < have.end ? have.end - f.offset : 0;
  return { reset: false, write: f.bytes.subarray(skip), cursor: { run, end: stop } };
}

/// How the exit reads in the terminal's status line.
export function exitText(exit: Exit | undefined): string {
  if (exit?.code !== undefined && exit.code !== null) return `exit ${exit.code}`;
  if (exit?.signal) return `killed by signal ${exit.signal}`;
  return "ended";
}
