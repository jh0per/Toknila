// The assistant's pins on designs: which pins a mockup shows, and what an answer may be. The
// daemon decides what a pin is; nothing here decides what waits on a person.

import type { DesignPin, EventRecord } from "../api/types";

export type Pin = DesignPin;

/// Most characters of an answer (the daemon's bound).
export const MAX_ANSWER = 500;

/// Whether the person has not answered the pin yet.
export function isOpen(p: Pin): boolean {
  return !p.answer;
}

/// The pins on one design; with a `card`, only the ones about that card. Oldest first, as the
/// daemon lists them.
export function pinsFor(pins: Pin[], design: string, card?: string): Pin[] {
  return pins.filter((p) => p.design === design && (card === undefined || p.card === card));
}

/// The `seq` of the newest pin event of the journal, 0 when there is none: a page that shows pins
/// reads them again when it changes.
export function pinVersion(events: EventRecord[]): number {
  for (let i = events.length - 1; i >= 0; i--) if (events[i].entity === "design_pin") return events[i].seq;
  return 0;
}

/// What is wrong with an answer, or nothing.
export function answerProblem(text: string): string | undefined {
  if (!text.trim()) return "Write an answer first.";
  if (text.trim().length > MAX_ANSWER) return `An answer has at most ${MAX_ANSWER} characters.`;
  return undefined;
}

/// A pin's place in words, as the card's prompt names it.
export function pinPlace(p: Pin): string {
  const r = p.region;
  return `x ${r.x}%, y ${r.y}%, ${r.w}% wide, ${r.h}% tall`;
}
