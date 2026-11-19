import { describe, expect, it } from "vitest";
import { NOTES, PEAK_GAIN, type SoundKind, duration } from "./notes";

describe("the notes of the three sounds", () => {
  it("pins every note: frequency, start and length", () => {
    expect(NOTES).toEqual({
      story_done: [
        { freq: 660, start: 0, length: 0.11 },
        { freq: 880, start: 0.12, length: 0.13 },
      ],
      epic_done: [
        { freq: 523.25, start: 0, length: 0.18 },
        { freq: 659.25, start: 0.2, length: 0.18 },
        { freq: 783.99, start: 0.4, length: 0.2 },
      ],
      problem: [
        { freq: 196, start: 0, length: 0.19 },
        { freq: 130.81, start: 0.2, length: 0.2 },
      ],
    });
  });

  it("a story is two rising notes in about a quarter of a second", () => {
    const n = NOTES.story_done;
    expect(n).toHaveLength(2);
    expect(n[1].freq).toBeGreaterThan(n[0].freq);
    expect(duration("story_done")).toBeCloseTo(0.25, 1);
  });

  it("an epic is three rising notes in about 0.6 seconds", () => {
    const n = NOTES.epic_done;
    expect(n).toHaveLength(3);
    expect(n[0].freq).toBeLessThan(n[1].freq);
    expect(n[1].freq).toBeLessThan(n[2].freq);
    expect(duration("epic_done")).toBeCloseTo(0.6, 1);
  });

  it("a problem is two falling low notes in about 0.4 seconds", () => {
    const n = NOTES.problem;
    expect(n).toHaveLength(2);
    expect(n[1].freq).toBeLessThan(n[0].freq);
    expect(Math.max(...n.map((x) => x.freq))).toBeLessThan(250);
    expect(duration("problem")).toBeCloseTo(0.4, 1);
  });

  it("every note starts after the one before ends and the sound stays modest", () => {
    for (const kind of Object.keys(NOTES) as SoundKind[]) {
      NOTES[kind].forEach((note, i) => {
        if (i > 0) expect(note.start).toBeGreaterThanOrEqual(NOTES[kind][i - 1].start + NOTES[kind][i - 1].length);
      });
    }
    expect(PEAK_GAIN).toBeLessThanOrEqual(0.2);
  });
});
