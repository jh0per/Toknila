// Plays a sound with Web Audio: one oscillator per note under a gain envelope.
//
// A browser lets a page make sound only after the person has clicked or typed in it once, so the
// audio context is made and resumed on the first click or key. A sound that is due while audio is
// not running is dropped, never played late: a late sound says the wrong thing.

import { NOTES, PEAK_GAIN, type SoundKind, WAVE } from "./notes";

let ctx: AudioContext | undefined;

function context(): AudioContext | undefined {
  if (!ctx) {
    try {
      const Ctor = window.AudioContext ?? (window as unknown as { webkitAudioContext?: typeof AudioContext }).webkitAudioContext;
      if (Ctor) ctx = new Ctor();
    } catch {
      /* no audio here */
    }
  }
  return ctx;
}

/// Makes or resumes the audio context on the person's first click or key. Returns the way to stop.
export function armAudio(target: Window = window): () => void {
  const wake = () => {
    void context()?.resume().catch(() => undefined);
  };
  const events = ["pointerdown", "keydown"] as const;
  for (const e of events) target.addEventListener(e, wake, { capture: true });
  return () => {
    for (const e of events) target.removeEventListener(e, wake, { capture: true });
  };
}

/// Schedules the notes of `kind` on `c`, from now.
function schedule(c: AudioContext, kind: SoundKind) {
  const t0 = c.currentTime + 0.02;
  for (const note of NOTES[kind]) {
    const osc = c.createOscillator();
    const gain = c.createGain();
    osc.type = WAVE[kind];
    osc.frequency.value = note.freq;
    const start = t0 + note.start;
    const end = start + note.length;
    gain.gain.setValueAtTime(0, start);
    gain.gain.linearRampToValueAtTime(PEAK_GAIN, start + 0.01);
    gain.gain.linearRampToValueAtTime(0, end);
    osc.connect(gain);
    gain.connect(c.destination);
    osc.start(start);
    osc.stop(end + 0.02);
  }
}

/// Plays `kind` now, only when audio is already allowed and running; otherwise nothing happens
/// (never later). Returns whether it played.
export function playSound(kind: SoundKind): boolean {
  if (!ctx || ctx.state !== "running") return false;
  schedule(ctx, kind);
  return true;
}

/// The Play button of Settings: the click itself is the permission, so this wakes the audio and
/// plays whether or not the kind's switch is on.
export function previewSound(kind: SoundKind): void {
  const c = context();
  if (!c) return;
  void c.resume().then(() => schedule(c, kind), () => undefined);
}
