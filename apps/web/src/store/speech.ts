// Speaking to the assistant: the pure rules. The recognizer itself is the system's (the page's
// Web Speech API); this keeps how its words join what the person already typed, when a pause
// sends, and which sentence a failure gets. `views/useSpeech.ts` runs it.

/// How long a silence ends a turn when "send when I stop speaking" is on.
export const PAUSE_MS = 2000;

/// What the box holds while listening: the words typed before the microphone opened, the words the
/// recognizer is sure of, and the tail it may still change.
export interface Heard {
  base: string;
  final: string;
  interim: string;
}

/// One recognizer result: its words and whether they are settled.
export interface Piece {
  text: string;
  final: boolean;
}

/// Joins two stretches of text with one space (none when either is empty or a space already is there).
const join = (a: string, b: string): string => {
  const left = a.replace(/\s+$/, "");
  const right = b.replace(/^\s+/, "");
  if (!right) return a;
  if (!left) return right;
  return `${a.endsWith("\n") ? a : left} ${right}`.replace(/\n /, "\n");
};

/// The start of a listening turn: what is in the box now is kept as it is.
export const startHeard = (base: string): Heard => ({ base, final: "", interim: "" });

/// Folds the recognizer's results (the whole list it holds, in order) into what was heard.
export function hear(h: Heard, pieces: Piece[]): Heard {
  const final = pieces.filter((p) => p.final).map((p) => p.text.trim()).filter(Boolean).join(" ");
  const interim = pieces.filter((p) => !p.final).map((p) => p.text.trim()).filter(Boolean).join(" ");
  return { ...h, final, interim };
}

/// The text of the field: typed words, the final words, then the interim ones.
export const fieldText = (h: Heard): string => join(join(h.base, h.final), h.interim);

/// The text of the field that is settled (what to keep when the interim tail is dropped).
export const settledText = (h: Heard): string => join(h.base, h.final);

/// Whether a pause sends: the setting is on, the box has words, and the person stopped for `PAUSE_MS`.
export const shouldAutoSend = (on: boolean, text: string, lastHeardAt: number, now: number): boolean =>
  on && text.trim().length > 0 && now - lastHeardAt >= PAUSE_MS;

/// The language to listen in: the window's, then the system's, then English.
export const speechLang = (nav: { language?: string; languages?: readonly string[] }): string =>
  nav.language || nav.languages?.[0] || "en-US";

/// What the microphone button can do now.
export type Mic = "idle" | "listening" | "denied" | "none" | "error";

/// The recognizer's error code as a state and a sentence for the person.
export function micFailure(code: string): { mic: Mic; message: string } {
  switch (code) {
    case "not-allowed":
    case "service-not-allowed":
      return { mic: "denied", message: DENIED };
    case "no-speech":
      return { mic: "idle", message: "" };
    case "aborted":
      return { mic: "idle", message: "" };
    case "language-not-supported":
      return { mic: "error", message: `Could not listen: this language is not supported. ${DICTATE}` };
    default:
      return { mic: "error", message: `Could not listen (${code}). Try again, or ${DICTATE.toLowerCase()}` };
  }
}

/// The hint when the window has no recognizer.
export const DICTATE = "Press Fn twice to dictate with the system's dictation.";

/// What to allow, and where, when the microphone or the speech recognition is refused.
export const DENIED =
  "Allow Toknila to use the microphone and speech recognition: System Settings → Privacy & Security → Microphone and Speech Recognition.";
