// The three sounds as plain data: every note is a frequency (Hz), a start and a length (seconds,
// from the moment the sound begins). Made in code; no audio file and nothing from another product.

export type SoundKind = "story_done" | "epic_done" | "problem";

export interface Note {
  freq: number;
  start: number;
  length: number;
}

/// The loudest a note may get (the system volume is the person's control).
export const PEAK_GAIN = 0.15;

/// The shape of each sound's oscillator.
export const WAVE: Record<SoundKind, OscillatorType> = {
  story_done: "sine",
  epic_done: "sine",
  problem: "triangle",
};

export const NOTES: Record<SoundKind, Note[]> = {
  // Two short rising notes, a quarter of a second.
  story_done: [
    { freq: 660, start: 0, length: 0.11 },
    { freq: 880, start: 0.12, length: 0.13 },
  ],
  // Three rising notes, 0.6 seconds: a major triad.
  epic_done: [
    { freq: 523.25, start: 0, length: 0.18 },
    { freq: 659.25, start: 0.2, length: 0.18 },
    { freq: 783.99, start: 0.4, length: 0.2 },
  ],
  // Two falling low notes, 0.4 seconds.
  problem: [
    { freq: 196, start: 0, length: 0.19 },
    { freq: 130.81, start: 0.2, length: 0.2 },
  ],
};

/// When the last note of a sound ends.
export const duration = (kind: SoundKind): number => Math.max(...NOTES[kind].map((n) => n.start + n.length));
