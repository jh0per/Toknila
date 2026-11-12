// Which key presses in the terminal are the person's copy, and which belong to the agent. Pure:
// the view passes what it knows, the answer says what to do.

export interface KeyLike {
  key: string;
  ctrlKey: boolean;
  metaKey: boolean;
  shiftKey: boolean;
  altKey: boolean;
}

/// What a key press does: `copy` copies the selection and is not sent to the agent; `pass`
/// leaves the key to the terminal (the agent gets it).
export type KeyAction = "copy" | "pass";

/// Whether this is a Mac (Cmd is the copy key there, Ctrl+C is the agent's interrupt).
export const isMac = (platform: string): boolean => /mac|iphone|ipad/i.test(platform);

/// Cmd+C on a Mac and Ctrl+Shift+C elsewhere always copy (with no selection there is nothing to
/// copy and nothing is sent either). Ctrl+C on Windows and Linux copies only while something is
/// selected; without a selection it is the interrupt and goes to the agent.
export function keyAction(e: KeyLike, mac: boolean, hasSelection: boolean): KeyAction {
  if (e.altKey || e.key.toLowerCase() !== "c") return "pass";
  if (mac) return e.metaKey && !e.ctrlKey && !e.shiftKey ? "copy" : "pass";
  if (e.metaKey) return "pass";
  if (e.ctrlKey && e.shiftKey) return "copy";
  if (e.ctrlKey && !e.shiftKey && hasSelection) return "copy";
  return "pass";
}
