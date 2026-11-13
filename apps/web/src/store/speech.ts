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

/// The hint when the window has no recognizer, and the tail of the failure sentences.
export const DICTATE = "Press Fn twice to dictate with the system's dictation (System Settings → Keyboard → Dictation turns it on).";

const lowerFirst = (s: string): string => s.charAt(0).toLowerCase() + s.slice(1);

/// The recognizer's error code as a state and a sentence for the person.
/// `detail` is the system's own reason (the error event's `message`), when it gave one.
export function micFailure(code: string, detail?: string): { mic: Mic; message: string } {
  const why = detail?.trim().replace(/\.+$/, "") ?? "";
  switch (code) {
    case "not-allowed":
    case "service-not-allowed":
      return { mic: "denied", message: DENIED };
    case "no-speech":
      return { mic: "idle", message: "" };
    case "aborted":
      // An abort nobody explained is the person's own stop or a replaced request.
      if (!why) return { mic: "idle", message: "" };
      if (/dictation/i.test(why)) return { mic: "error", message: DICTATION_OFF };
      return { mic: "error", message: failed(why) };
    case "language-not-supported":
      return { mic: "error", message: `Could not listen: this language is not supported. ${DICTATE}` };
    default:
      return { mic: "error", message: failed(why || `code ${code}`, code) };
  }
}

/// The sentence of a failure: the system's reason when there is one, else the error code.
const failed = (why: string, code?: string): string =>
  code && why === `code ${code}`
    ? `Could not listen (${code}). Try again, or ${lowerFirst(DICTATE)}`
    : `Could not listen: ${why}. Try again, or ${lowerFirst(DICTATE)}`;

/// The recognizer ended within `FAST_END_MS` of starting with no words and no error.
export const FAST_END_MS = 1500;

/// What the person reads when the recognizer ended at once without a word of explanation.
export const ENDED_AT_ONCE = `Listening ended at once, without words. Press again; if it repeats, ${lowerFirst(DICTATE)}`;

/// macOS refused because Siri and Dictation are both off.
export const DICTATION_OFF =
  "macOS refused to listen: Siri and Dictation are off. Turn on Dictation in System Settings → Keyboard → Dictation (or turn on Siri), then press the button again.";

/// What to allow, and where, when the microphone or the speech recognition is refused.
export const DENIED =
  "Allow Toknila to use the microphone and speech recognition: System Settings → Privacy & Security → Microphone and Speech Recognition.";
