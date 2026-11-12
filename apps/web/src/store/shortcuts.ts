// The keyboard: the shortcuts the window announces (the "?" popover lists them), which key opens
// that list, and how the arrow keys move between the tabs of a tab list.

import type { State } from "./state";

/// What the person did.
export type ShortcutIntent = { type: "toggleShortcuts" } | { type: "closeShortcuts" };

/// One line of the "?" popover: the keys, and what they do.
export interface Shortcut {
  keys: string[];
  what: string;
}

/// Every key the window answers to. A key that is added to the window is added here, in the same change.
export const SHORTCUTS: Shortcut[] = [
  { keys: ["N"], what: "New story on the diagram" },
  { keys: ["S"], what: "Start the story you are on" },
  { keys: ["Enter"], what: "Open the story you are on" },
  { keys: ["Delete"], what: "Remove the dependency line you picked" },
  { keys: ["Esc"], what: "Close a menu or a dialog, or let go of what you picked" },
  { keys: ["Cmd/Ctrl", ","], what: "Open Settings" },
  { keys: ["Cmd/Ctrl", "."], what: "Show or hide the assistant beside the roadmap, an epic or a story" },
  { keys: ["Cmd/Ctrl", "K"], what: "Switch project: open the project list, type to filter" },
  { keys: ["?"], what: "Show this list" },
];

/// The reducer of the shortcuts popover.
export function reduceShortcuts(s: State, msg: ShortcutIntent): State {
  return { ...s, shortcutsOpen: msg.type === "toggleShortcuts" ? !s.shortcutsOpen : false };
}

/// What a key press looks like to the rules below.
export interface KeyPress {
  key: string;
  ctrlKey: boolean;
  metaKey: boolean;
  altKey: boolean;
}

/// Whether the press asks for the shortcuts list: "?", with no Ctrl, Cmd or Alt, and not typed
/// into a text field.
export function asksForShortcuts(e: KeyPress, inTextField: boolean): boolean {
  return e.key === "?" && !e.ctrlKey && !e.metaKey && !e.altKey && !inTextField;
}

/// The tab an arrow key moves to from `current`: Left and Right step and wrap round, Home and End
/// go to the ends; any other key leaves the tab where it is (`undefined`).
export function tabAfterKey<T>(tabs: readonly T[], current: T, key: string): T | undefined {
  const at = tabs.indexOf(current);
  if (at < 0 || tabs.length === 0) return undefined;
  switch (key) {
    case "ArrowRight": return tabs[(at + 1) % tabs.length];
    case "ArrowLeft": return tabs[(at - 1 + tabs.length) % tabs.length];
    case "Home": return tabs[0];
    case "End": return tabs[tabs.length - 1];
    default: return undefined;
  }
}
