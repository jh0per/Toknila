import { describe, expect, it } from "vitest";
import { type TermCursor, applyFrame, exitText, parseFrame } from "./wire";

const RUN1 = "01JBQ7ZJ0000000000000000R1";
const RUN2 = "01JBQ7ZJ0000000000000000R2";

function frame(run: string, offset: number, text: string): ArrayBuffer {
  const body = new TextEncoder().encode(text);
  const out = new Uint8Array(34 + body.length);
  out.set(new TextEncoder().encode(run), 0);
  new DataView(out.buffer).setBigUint64(26, BigInt(offset));
  out.set(body, 34);
  return out.buffer;
}

const text = (b: Uint8Array | null) => (b ? new TextDecoder().decode(b) : null);
const start: TermCursor = { run: null, end: 0 };
const parsed = (run: string, offset: number, t: string) => {
  const f = parseFrame(frame(run, offset, t));
  if (!f) throw new Error("not a frame");
  return f;
};

describe("parseFrame", () => {
  it("reads the run id, the 8-byte big-endian offset and the raw bytes", () => {
    const f = parsed(RUN1, 258, "hi\x1b[0m");
    expect(f.run).toBe(RUN1);
    expect(f.offset).toBe(258);
    expect(text(f.bytes)).toBe("hi\x1b[0m");
  });

  it("refuses a message too short to be a frame", () => {
    expect(parseFrame(new TextEncoder().encode("short").buffer as ArrayBuffer)).toBeNull();
  });
});

describe("applyFrame", () => {
  it("writes the first frame and moves the cursor to the byte after it", () => {
    const r = applyFrame(start, parsed(RUN1, 0, "abc"));
    expect(r.reset).toBe(false);
    expect(text(r.write)).toBe("abc");
    expect(r.cursor).toEqual({ run: RUN1, end: 3 });
  });

  it("drops bytes it already has (the scrollback and the stream overlap)", () => {
    const have: TermCursor = { run: RUN1, end: 5 };
    expect(applyFrame(have, parsed(RUN1, 0, "abc")).write).toBeNull();
    const r = applyFrame(have, parsed(RUN1, 3, "cdefg"));
    expect(text(r.write)).toBe("efg");
    expect(r.cursor.end).toBe(8);
  });

  it("a new run id clears the view and starts over", () => {
    const r = applyFrame({ run: RUN1, end: 99 }, parsed(RUN2, 0, "new"));
    expect(r.reset).toBe(true);
    expect(text(r.write)).toBe("new");
    expect(r.cursor).toEqual({ run: RUN2, end: 3 });
  });

  it("the first frame of a run is no reset, there is nothing to clear", () => {
    expect(applyFrame(start, parsed(RUN2, 0, "x")).reset).toBe(false);
  });
});

describe("exitText", () => {
  it("names a code, a signal and an unknown end", () => {
    expect(exitText({ code: 0, signal: null })).toBe("exit 0");
    expect(exitText({ code: null, signal: "SIGKILL" })).toBe("killed by signal SIGKILL");
    expect(exitText(undefined)).toBe("ended");
  });
});
