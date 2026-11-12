// Remembers the assistant panel's open state, width and filter in this browser. The storage may be
// missing or refuse (a private window, blocked site data): the panel still works for this visit.

import { type AssistantSlice, initialAssistant, parseAssistant } from "./assistant";

export const ASSISTANT_KEY = "toknila.assistant";

/// The remembered panel state, or the default when none is stored or it cannot be read.
export function readAssistant(): AssistantSlice {
  try {
    return parseAssistant(window.localStorage.getItem(ASSISTANT_KEY));
  } catch {
    return initialAssistant;
  }
}

/// Remembers what the viewer chose.
export function saveAssistant(a: { open: boolean; width: number; only: boolean }): void {
  try {
    window.localStorage.setItem(ASSISTANT_KEY, JSON.stringify(a));
  } catch {
    // Not remembered; it still applies now.
  }
}
