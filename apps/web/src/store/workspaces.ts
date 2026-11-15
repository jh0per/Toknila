import type { Workspace, Workspaces } from "../api/types";

/// The line above the workspace list: how many there are and how many belong to finished stories.
/// The assistant's reading copy is not counted.
export function workspacesLine(w: Workspaces): string {
  const of = w.finished === 1 ? "1 belongs to a finished story" : `${w.finished} belong to finished stories`;
  return `Workspaces: ${w.count}, of which ${of}`;
}

/// One workspace as a line of the list.
export function workspaceText(w: Workspace): string {
  if (w.kind === "planner") return `The assistant's reading copy (not counted): ${w.path}`;
  const state = [w.status ?? "", w.finished ? "finished" : ""].filter(Boolean).join(", ");
  return `${w.story ?? "?"}${state ? ` (${state})` : ""}: ${w.path}`;
}

/// What Clean up says when it started.
export function retiredText(n: number): string {
  if (n === 0) return "Nothing to clean up";
  return n === 1 ? "1 workspace is being cleaned up" : `${n} workspaces are being cleaned up`;
}
